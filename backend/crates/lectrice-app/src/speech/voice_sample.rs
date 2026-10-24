use std::sync::Arc;
use std::time::Instant;

use lectrice_domain::{
    AppError, AppResult, AudioEncoder, AudioKey, DocumentId, ErrorName, SentenceId, SpeechEngine, SpeechRequest, VoiceId, VoiceSampleCache, VoiceSampleKey,
};
use tracing::{info, warn};

use super::speak_sentence::SpokenSentence;
use super::worker::{SpeechJob, SpeechPriority, SpeechWorker, elapsed_ms};
use crate::{RequestContext, UseCase};

/// The one sample sentence for every voice and model (our own line, so no book text is involved).
pub const VOICE_SAMPLE_TEXT: &str = "This is how I sound when I read your books aloud.";
/// Raise it whenever [`VOICE_SAMPLE_TEXT`] changes, so samples of the old text aren't played.
pub const VOICE_SAMPLE_VERSION: u32 = 1;
/// Samples go through the speech worker like sentences, so they need a sentence key. No PDF hashes to this id.
const SAMPLE_DOCUMENT_ID: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A voice's sample: from disk when it was made before, else made now (at the highest priority) and kept on disk.
pub struct SpeakVoiceSample {
    engine: Arc<dyn SpeechEngine>,
    worker: Arc<SpeechWorker>,
    samples: Arc<dyn VoiceSampleCache>,
    encoder: Arc<dyn AudioEncoder>,
}

impl SpeakVoiceSample {
    pub fn new(engine: Arc<dyn SpeechEngine>, worker: Arc<SpeechWorker>, samples: Arc<dyn VoiceSampleCache>, encoder: Arc<dyn AudioEncoder>) -> Self {
        Self { engine, worker, samples, encoder }
    }

    fn spoken(&self, bytes: Vec<u8>) -> SpokenSentence {
        SpokenSentence { bytes, mime_type: self.encoder.mime_type().to_string(), ready_ahead_ms: 0 }
    }

    fn synthesize(&self, ctx: &RequestContext, voice_id: &VoiceId) -> AppResult<Vec<u8>> {
        let key = AudioKey {
            document_id: DocumentId::parse(SAMPLE_DOCUMENT_ID)?,
            cleaner_version: 0,
            engine_id: self.engine.id().to_string(),
            voice_id: voice_id.clone(),
            sentence_id: SentenceId(VOICE_SAMPLE_VERSION),
        };
        let started = Instant::now();
        let request = SpeechRequest { text: VOICE_SAMPLE_TEXT.to_string(), voice_id: voice_id.clone() };
        let audio = self.worker.submit(SpeechJob { key, request, priority: SpeechPriority::Now }).wait_unless_cancelled(&ctx.cancellation)?;
        let bytes = self.encoder.encode(&audio)?;
        info!(service = Self::NAME, voice_id = %voice_id, bytes = bytes.len(), wait_ms = elapsed_ms(started), "voice sample made");
        Ok(bytes)
    }
}

impl UseCase for SpeakVoiceSample {
    const NAME: &'static str = "SpeakVoiceSample";
    type Args = VoiceId;
    type Output = SpokenSentence;

    fn run(&self, ctx: &RequestContext, voice_id: VoiceId) -> AppResult<SpokenSentence> {
        info!(service = Self::NAME, request_id = %ctx.request_id, voice_id = %voice_id, "speaking voice sample");
        if !self.engine.voices().iter().any(|voice| voice.id == voice_id) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")));
        }
        let key = VoiceSampleKey { engine_id: self.engine.id().to_string(), voice_id: voice_id.clone(), version: VOICE_SAMPLE_VERSION };
        match self.samples.get(&key) {
            Ok(Some(bytes)) => {
                info!(service = Self::NAME, voice_id = %voice_id, bytes = bytes.len(), "voice sample found");
                return Ok(self.spoken(bytes));
            }
            Ok(None) => {}
            Err(error) => warn!(service = Self::NAME, voice_id = %voice_id, error = %error, "voice sample unreadable, making it again"),
        }

        let bytes = self.synthesize(ctx, &voice_id)?;
        // The sample still plays from memory when it can't be kept.
        match self.samples.put(&key, &bytes) {
            Ok(()) => info!(service = Self::NAME, voice_id = %voice_id, bytes = bytes.len(), "voice sample saved"),
            Err(error) => warn!(service = Self::NAME, voice_id = %voice_id, error = %error, "voice sample could not be saved"),
        }
        Ok(self.spoken(bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;
    use crate::speech::fakes::{CountingEncoder, GatedEngine, SAMPLE_RATE};

    #[derive(Default)]
    struct MemorySamples {
        stored: Mutex<HashMap<VoiceSampleKey, Vec<u8>>>,
        failing: bool,
    }

    impl VoiceSampleCache for MemorySamples {
        fn get(&self, key: &VoiceSampleKey) -> AppResult<Option<Vec<u8>>> {
            Ok(self.stored.lock().unwrap().get(key).cloned())
        }

        fn put(&self, key: &VoiceSampleKey, bytes: &[u8]) -> AppResult<()> {
            if self.failing {
                return Err(AppError::new(ErrorName::StorageFull));
            }
            self.stored.lock().unwrap().insert(key.clone(), bytes.to_vec());
            Ok(())
        }
    }

    fn service(samples: Arc<MemorySamples>) -> (Arc<GatedEngine>, SpeakVoiceSample) {
        let engine = Arc::new(GatedEngine::open());
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
        (engine.clone(), SpeakVoiceSample::new(engine, worker, samples, Arc::new(CountingEncoder)))
    }

    fn sample(service: &SpeakVoiceSample, voice: &str) -> AppResult<SpokenSentence> {
        service.run(&RequestContext::new("test"), VoiceId::new(voice))
    }

    #[test]
    fn the_first_sample_is_made_and_kept_and_the_next_one_is_read_back() {
        let samples = Arc::new(MemorySamples::default());
        let (engine, service) = service(samples.clone());

        let first = sample(&service, "af_heart").unwrap();
        let second = sample(&service, "af_heart").unwrap();

        assert_eq!(first, second);
        assert_eq!(String::from_utf8(first.bytes).unwrap(), format!("{SAMPLE_RATE}:{}", VOICE_SAMPLE_TEXT.len() * 100));
        assert_eq!(engine.spoken(), [VOICE_SAMPLE_TEXT]);
        let stored = samples.stored.lock().unwrap();
        let key = stored.keys().next().unwrap();
        assert_eq!((key.engine_id.as_str(), key.voice_id.as_str(), key.version), ("fake-engine", "af_heart", VOICE_SAMPLE_VERSION));
    }

    #[test]
    fn a_sample_that_cannot_be_saved_still_plays() {
        let (_, service) = service(Arc::new(MemorySamples { failing: true, ..MemorySamples::default() }));

        assert!(!sample(&service, "bm_lewis").unwrap().bytes.is_empty());
    }

    #[test]
    fn an_unknown_voice_is_refused() {
        let (engine, service) = service(Arc::default());

        assert_eq!(sample(&service, "zz_nobody").unwrap_err().primary(), ErrorName::VoiceUnknown);
        assert!(engine.spoken().is_empty());
    }
}
