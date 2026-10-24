use std::sync::Arc;
use std::time::Instant;

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioEncoder, AudioKey, Document, DocumentId, DocumentReader, ErrorName, Sentence, SentenceId, SpeechEngine,
    SpeechRequest, VoiceId,
};
use tracing::{debug, info, warn};

use super::read_ahead::{ListenerPosition, ReadAhead};
use super::worker::{SpeechJob, SpeechPriority, SpeechTicket, SpeechWorker, audio_seconds, elapsed_ms};
use crate::{RequestContext, UseCase};

/// A sentence with nothing to say (e.g. only a URL) gets this much silence, so the player just moves on.
const SILENCE_SECONDS: f64 = 0.25;

#[derive(Debug, Clone)]
pub struct SpeakSentenceArgs {
    pub document_id: DocumentId,
    pub sentence_id: SentenceId,
    pub voice_id: VoiceId,
    /// The listener's speed, a positive number. Audio is always made at 1x; it sizes read-ahead and the cache window.
    pub rate: f32,
    /// Whether read-ahead includes code, table and caption sentences instead of their cues.
    pub read_skipped: bool,
    /// `false` makes only this sentence: no read-ahead plan, no cancelled jobs, no cache trim. A player that keeps
    /// loading while paused uses it, so it can't restart a read-ahead the shell stopped.
    pub plan_ahead: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenSentence {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    /// Estimated 1x audio of the cached sentences right after this one, in playback order (a player's buffered part).
    pub ready_ahead_ms: u32,
}

/// One sentence's audio: from the cache, or made by the speech worker at the highest priority, then cached.
/// Every request also moves the read-ahead and the cache window to this sentence.
pub struct SpeakSentence {
    engine: Arc<dyn SpeechEngine>,
    worker: Arc<SpeechWorker>,
    reader: Arc<dyn DocumentReader>,
    cache: Arc<dyn AudioCache>,
    encoder: Arc<dyn AudioEncoder>,
    read_ahead: Arc<ReadAhead>,
}

pub struct SpeakSentenceDeps {
    pub engine: Arc<dyn SpeechEngine>,
    pub worker: Arc<SpeechWorker>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub encoder: Arc<dyn AudioEncoder>,
    pub read_ahead: Arc<ReadAhead>,
}

impl SpeakSentence {
    pub fn new(deps: SpeakSentenceDeps) -> Self {
        let SpeakSentenceDeps { engine, worker, reader, cache, encoder, read_ahead } = deps;
        Self { engine, worker, reader, cache, encoder, read_ahead }
    }

    fn check_voice(&self, voice_id: &VoiceId) -> AppResult<()> {
        if self.engine.voices().iter().any(|voice| voice.id == *voice_id) {
            return Ok(());
        }
        Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")))
    }

    fn load_document(&self, id: &DocumentId) -> AppResult<Document> {
        let document = self.reader.find(id)?;
        debug!(service = Self::NAME, doc_id = %id, found = document.is_some(), cleaner_version = document.as_ref().map(|document| document.cleaner_version), "document loaded");
        document.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))
    }

    fn silence(&self, key: &AudioKey) -> AppResult<Vec<u8>> {
        let sample_rate = self.engine.sample_rate();
        let samples = (SILENCE_SECONDS * f64::from(sample_rate)) as usize;
        let bytes = self.encoder.encode(&Audio { samples: vec![0.0; samples], sample_rate })?;
        info!(service = Self::NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, bytes = bytes.len(), "sentence has no speech text, sent silence");
        Ok(bytes)
    }

    fn follow(&self, position: ListenerPosition<'_>, plan_ahead: bool) {
        if plan_ahead {
            self.read_ahead.follow(position);
        }
    }

    fn submit_now(&self, key: &AudioKey, text: &str) -> SpeechTicket {
        let request = SpeechRequest { text: text.to_string(), voice_id: key.voice_id.clone() };
        self.worker.submit(SpeechJob { key: key.clone(), request, priority: SpeechPriority::Now })
    }

    /// Waits for the worker unless the caller gives up first; then the job is withdrawn so it doesn't
    /// hold up the requests that come after it (review M1).
    fn receive(&self, ctx: &RequestContext, key: &AudioKey, ticket: SpeechTicket, started: Instant) -> AppResult<Vec<u8>> {
        let audio = ticket.wait_unless_cancelled(&ctx.cancellation).inspect_err(|error| {
            if error.is(ErrorName::RequestCancelled) {
                info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %key.document_id, sentence_id = key.sentence_id.0, wait_ms = elapsed_ms(started), "request cancelled, speech job withdrawn");
            }
        })?;
        let wait_ms = elapsed_ms(started);
        info!(service = Self::NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, voice_id = %key.voice_id, audio_seconds = audio_seconds(&audio), wait_ms, "speech received from worker");

        let bytes = self.encoder.encode(&audio)?;
        debug!(service = Self::NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, bytes = bytes.len(), "audio encoded");
        // The audio is already made, so a failed cache write only costs a later re-synthesis.
        match self.cache.put(key, &bytes) {
            Ok(()) => {
                debug!(service = Self::NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, voice_id = %key.voice_id, bytes = bytes.len(), "audio cached")
            }
            Err(error) => warn!(service = Self::NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "audio cache write failed"),
        }
        Ok(bytes)
    }

    fn spoken(&self, bytes: Vec<u8>, position: ListenerPosition<'_>) -> SpokenSentence {
        SpokenSentence { bytes, mime_type: self.encoder.mime_type().to_string(), ready_ahead_ms: self.read_ahead.ready_ahead_ms(position) }
    }
}

impl UseCase for SpeakSentence {
    const NAME: &'static str = "SpeakSentence";
    type Args = SpeakSentenceArgs;
    type Output = SpokenSentence;

    fn run(&self, ctx: &RequestContext, args: SpeakSentenceArgs) -> AppResult<SpokenSentence> {
        let SpeakSentenceArgs { document_id, sentence_id, voice_id, rate, read_skipped, plan_ahead } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, sentence_id = sentence_id.0, voice_id = %voice_id, rate, read_skipped, plan_ahead, "speaking sentence");

        check_rate(rate)?;
        self.check_voice(&voice_id)?;
        let document = self.load_document(&document_id)?;
        let Some(sentence) = find_sentence(&document, sentence_id) else {
            return Err(AppError::new(ErrorName::SentenceNotFound).with_detail(format!("sentence {sentence_id} of {document_id}")));
        };
        let key = AudioKey { document_id, cleaner_version: document.cleaner_version, engine_id: self.engine.id().to_string(), voice_id, sentence_id };

        let position = ListenerPosition { document: &document, key: &key, rate, read_skipped };

        let text = sentence.spoken_text();
        if text.trim().is_empty() {
            self.follow(position, plan_ahead);
            let bytes = self.silence(&key)?;
            return Ok(self.spoken(bytes, position));
        }

        let cached = self.cache.get(&key)?;
        debug!(service = Self::NAME, doc_id = %key.document_id, sentence_id = sentence_id.0, voice_id = %key.voice_id, cache_hit = cached.is_some(), "audio cache looked up");
        if let Some(bytes) = cached {
            self.follow(position, plan_ahead);
            return Ok(self.spoken(bytes, position));
        }

        // Queued before the read-ahead, so this sentence is made first even if the worker is idle.
        let started = Instant::now();
        let ticket = self.submit_now(&key, text);
        self.follow(position, plan_ahead);
        let bytes = self.receive(ctx, &key, ticket, started)?;
        Ok(self.spoken(bytes, position))
    }
}

/// The route also limits the range; here only what would break the read-ahead and the cache window is refused
/// (a NaN rate would plan the whole book and empty the window).
fn check_rate(rate: f32) -> AppResult<()> {
    if rate.is_finite() && rate > 0.0 {
        return Ok(());
    }
    Err(AppError::new(ErrorName::RequestInvalid).with_detail(format!("rate {rate} is not a positive number")))
}

/// Sentence ids run across blocks; a skipped block's cue is a sentence too.
fn find_sentence(document: &Document, id: SentenceId) -> Option<&Sentence> {
    document.blocks.iter().flat_map(|block| block.sentences.iter().chain(block.cue.iter())).find(|sentence| sentence.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::fakes::{InMemoryStore, Library, id_for};
    use crate::speech::fakes::{CountingEncoder, ENGINE_ID, GatedEngine, MemoryCache, SAMPLE_RATE};
    use crate::speech::read_ahead::{ReadAheadDeps, ReadAheadSettings};
    use lectrice_domain::{Block, BlockKind, Clock, DocumentMeta, DocumentWriter, PageLabel};
    use std::time::Duration;

    const KEEP_MINUTES: u32 = 60;

    struct Setup {
        library: Library,
        engine: Arc<GatedEngine>,
        cache: Arc<MemoryCache>,
        service: SpeakSentence,
    }

    /// Read-ahead plans nothing, so these tests see only the requested sentence.
    fn setup() -> Setup {
        setup_with_read_ahead(0)
    }

    fn setup_with_read_ahead(read_ahead_minutes: u32) -> Setup {
        setup_with(GatedEngine::open(), read_ahead_minutes)
    }

    fn setup_with(engine: GatedEngine, read_ahead_minutes: u32) -> Setup {
        let library = Library::new(1);
        let engine = Arc::new(engine);
        let cache = Arc::new(MemoryCache::default());
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
        let encoder = Arc::new(CountingEncoder);
        let settings = ReadAheadSettings { read_ahead_minutes, keep_minutes_per_document: KEEP_MINUTES };
        let read_ahead_deps =
            ReadAheadDeps { worker: worker.clone(), reader: library.reader(), cache: cache.clone(), retention: cache.clone(), encoder: encoder.clone() };
        let read_ahead = Arc::new(ReadAhead::new(read_ahead_deps, settings).unwrap());
        let deps = SpeakSentenceDeps { engine: engine.clone(), worker, reader: library.reader(), cache: cache.clone(), encoder, read_ahead };
        Setup { library, engine, cache, service: SpeakSentence::new(deps) }
    }

    fn speak(setup: &Setup, document_id: DocumentId, sentence: u32, voice: &str) -> AppResult<SpokenSentence> {
        speak_with(setup, &RequestContext::new("test"), document_id, sentence, voice)
    }

    fn args(document_id: DocumentId, sentence: u32, voice: &str) -> SpeakSentenceArgs {
        SpeakSentenceArgs { document_id, sentence_id: SentenceId(sentence), voice_id: VoiceId::new(voice), rate: 1.0, read_skipped: false, plan_ahead: true }
    }

    fn speak_with(setup: &Setup, ctx: &RequestContext, document_id: DocumentId, sentence: u32, voice: &str) -> AppResult<SpokenSentence> {
        setup.service.run(ctx, args(document_id, sentence, voice))
    }

    /// C13: without planning, only the sentence itself is made and the cache window stays put.
    #[test]
    fn plan_ahead_off_makes_only_the_sentence_and_a_later_request_plans_again() {
        let setup = setup_with_read_ahead(10);
        let id = setup.library.add_book();

        setup.service.run(&RequestContext::new("test"), SpeakSentenceArgs { plan_ahead: false, ..args(id.clone(), 1, "af_heart") }).unwrap();

        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(setup.engine.spoken(), ["One."]);
        assert!(setup.cache.retained().is_empty());
        speak(&setup, id, 1, "af_heart").unwrap();
        assert_eq!(setup.cache.retained().len(), 1);
    }

    /// C25: the sentences cached right after this one are reported as ready.
    #[test]
    fn the_answer_says_how_much_is_ready_ahead() {
        let setup = setup();
        let id = setup.library.add_book();
        speak(&setup, id.clone(), 2, "af_heart").unwrap();

        let spoken = speak(&setup, id, 1, "af_heart").unwrap();

        assert_eq!(spoken.ready_ahead_ms, (f64::from("Two.".len() as u32) / 18.0 * 1000.0).round() as u32);
    }

    /// Review M1: a request the client gave up on must not keep its `Now` job ahead of later requests.
    #[test]
    fn a_cancelled_request_withdraws_its_speech_job() {
        let setup = Arc::new(setup_with(GatedEngine::new("One."), 0));
        let id = setup.library.add_book();
        let busy = {
            let (setup, id) = (setup.clone(), id.clone());
            std::thread::spawn(move || speak(&setup, id, 1, "af_heart"))
        };
        setup.engine.wait_until_blocked();
        let ctx = RequestContext::new("abandoned");
        let abandoned = {
            let (setup, id, ctx) = (setup.clone(), id.clone(), ctx.clone());
            std::thread::spawn(move || speak_with(&setup, &ctx, id, 2, "af_heart"))
        };

        ctx.cancellation.cancel();
        assert_eq!(abandoned.join().unwrap().unwrap_err().primary(), ErrorName::RequestCancelled);
        setup.engine.release();
        busy.join().unwrap().unwrap();
        speak(&setup, id, 4, "af_heart").unwrap();

        assert_eq!(setup.engine.spoken(), ["One.", "Code example."]);
        assert_eq!(setup.cache.puts(), 2);
    }

    #[test]
    fn a_request_queues_the_read_ahead_after_itself_and_moves_the_cache_window() {
        let setup = setup_with_read_ahead(10);
        let id = setup.library.add_book();
        let args = SpeakSentenceArgs { rate: 1.5, ..args(id, 1, "af_heart") };

        setup.service.run(&RequestContext::new("test"), args).unwrap();

        let started = std::time::Instant::now();
        while setup.cache.puts() < 3 {
            assert!(started.elapsed() < Duration::from_secs(5), "read-ahead never finished");
            std::thread::yield_now();
        }
        assert_eq!(setup.engine.spoken(), ["One.", "Two.", "Code example."]);
        let (position, keep_seconds) = setup.cache.retained()[0].clone();
        assert_eq!(position.sentence_id, SentenceId(1));
        assert!((keep_seconds - f64::from(KEEP_MINUTES) * 60.0 * 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_miss_synthesizes_encodes_and_caches() {
        let setup = setup();
        let id = setup.library.add_book();

        let spoken = speak(&setup, id.clone(), 1, "af_heart").unwrap();

        assert_eq!(spoken.mime_type, "audio/test");
        assert_eq!(String::from_utf8(spoken.bytes).unwrap(), format!("{SAMPLE_RATE}:{}", "One.".len() * 100));
        assert_eq!(setup.engine.spoken(), ["One."]);
        let key = &setup.cache.keys()[0];
        assert_eq!((key.engine_id.as_str(), key.cleaner_version, key.sentence_id), (ENGINE_ID, 1, SentenceId(1)));
    }

    #[test]
    fn a_hit_does_not_call_the_engine() {
        let setup = setup();
        let id = setup.library.add_book();
        let first = speak(&setup, id.clone(), 1, "af_heart").unwrap();

        let second = speak(&setup, id, 1, "af_heart").unwrap();

        assert_eq!(first, second);
        assert_eq!(setup.engine.spoken().len(), 1);
        assert_eq!(setup.cache.puts(), 1);
    }

    #[test]
    fn the_cue_of_a_skipped_block_is_spoken() {
        let setup = setup();
        let id = setup.library.add_book();

        speak(&setup, id, 4, "af_heart").unwrap();

        assert_eq!(setup.engine.spoken(), ["Code example."]);
    }

    #[test]
    fn speech_text_is_used_and_empty_speech_text_is_silence() {
        let setup = setup();
        let store: &InMemoryStore = &setup.library.store;
        let id = id_for(2);
        let with_speech = Sentence { id: SentenceId(0), text: "See [12].".into(), page_index: 1, speech_text: Some("See.".into()) };
        let only_url = Sentence { id: SentenceId(1), text: "https://example.com".into(), page_index: 1, speech_text: Some(String::new()) };
        let block = Block {
            id: 0,
            kind: BlockKind::Paragraph,
            level: None,
            page_index: 1,
            spoken: true,
            text: None,
            sentences: vec![with_speech, only_url],
            cue: None,
        };
        let document = Document {
            id: id.clone(),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: vec![PageLabel { index: 1, label: "1".into() }],
            toc: Vec::new(),
            blocks: vec![block],
        };
        let now = setup.library.clock.now();
        store.save(&document, b"%PDF-", &DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None }).unwrap();

        speak(&setup, id.clone(), 0, "af_heart").unwrap();
        let silence = speak(&setup, id, 1, "af_heart").unwrap();

        assert_eq!(setup.engine.spoken(), ["See."]);
        assert_eq!(String::from_utf8(silence.bytes).unwrap(), format!("{SAMPLE_RATE}:6000"));
        assert_eq!(setup.cache.puts(), 1);
    }

    #[test]
    fn unknown_voice_document_and_sentence_are_rejected() {
        let setup = setup();
        let id = setup.library.add_book();

        assert_eq!(speak(&setup, id.clone(), 1, "zz_nobody").unwrap_err().primary(), ErrorName::VoiceUnknown);
        assert_eq!(speak(&setup, id_for(9), 1, "af_heart").unwrap_err().primary(), ErrorName::DocumentNotFound);
        assert_eq!(speak(&setup, id, 5, "af_heart").unwrap_err().primary(), ErrorName::SentenceNotFound);
        assert!(setup.engine.spoken().is_empty());
    }

    /// Review L4: the service checks the rate itself, not only the route.
    #[test]
    fn a_rate_that_is_not_a_positive_number_is_rejected() {
        let setup = setup_with_read_ahead(10);
        let id = setup.library.add_book();
        for rate in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            let args = SpeakSentenceArgs { rate, ..args(id.clone(), 1, "af_heart") };
            assert_eq!(setup.service.run(&RequestContext::new("test"), args).unwrap_err().primary(), ErrorName::RequestInvalid, "{rate}");
        }
        assert!(setup.engine.spoken().is_empty());
        assert!(setup.cache.retained().is_empty());
    }
}
