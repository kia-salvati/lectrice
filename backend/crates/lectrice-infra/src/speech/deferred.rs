//! The deferred engine: knows its id, revision, voices and sample rate from the bundle without loading
//! the model, and loads it only when it first has to speak. While a helper computer makes the audio, the phone's own
//! model then never takes memory or start-up time; it is ready (`preload`) as soon as the link is lost.

use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::Instant;

use crate::sync::lock;
use lectrice_domain::{AppError, AppResult, Audio, ErrorName, SpeechEngine, SpeechRequest, Voice};
use tracing::{info, warn};

const SERVICE: &str = "DeferredSpeechEngine";

/// What the engine says about itself before it is loaded. The loaded engine must agree, or it is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredEngineInfo {
    pub engine_id: String,
    pub revision: String,
    pub sample_rate: u32,
    pub voices: Vec<Voice>,
}

type Loader = Box<dyn Fn() -> AppResult<Arc<dyn SpeechEngine>> + Send + Sync>;

pub struct DeferredSpeechEngine {
    info: DeferredEngineInfo,
    loader: Loader,
    /// Held while loading, so two callers never load twice.
    loaded: Mutex<Option<Arc<dyn SpeechEngine>>>,
}

impl DeferredSpeechEngine {
    pub fn new(info: DeferredEngineInfo, loader: impl Fn() -> AppResult<Arc<dyn SpeechEngine>> + Send + Sync + 'static) -> Self {
        Self { info, loader: Box::new(loader), loaded: Mutex::new(None) }
    }

    /// Loads the model now (e.g. when the helper link is lost), so the first local sentence doesn't wait for it.
    pub fn preload(&self) -> AppResult<()> {
        self.engine().map(|_| ())
    }

    /// Never waits for a load in progress (that counts as not loaded yet).
    pub fn is_loaded(&self) -> bool {
        match self.loaded.try_lock() {
            Ok(loaded) => loaded.is_some(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().is_some(),
            Err(TryLockError::WouldBlock) => false,
        }
    }

    /// Lets the model go (its memory is freed once no synthesis holds it); the next sentence loads it again.
    pub fn unload(&self) -> bool {
        let unloaded = self.lock().take().is_some();
        if unloaded {
            info!(service = SERVICE, engine_id = %self.info.engine_id, "Speech engine unloaded");
        }
        unloaded
    }

    fn lock(&self) -> MutexGuard<'_, Option<Arc<dyn SpeechEngine>>> {
        lock(&self.loaded)
    }

    fn engine(&self) -> AppResult<Arc<dyn SpeechEngine>> {
        let mut loaded = self.lock();
        if let Some(engine) = loaded.as_ref() {
            return Ok(engine.clone());
        }
        let started = Instant::now();
        let engine =
            (self.loader)().inspect_err(|error| warn!(service = SERVICE, engine_id = %self.info.engine_id, error = %error, "Speech engine load failed"))?;
        self.check(engine.as_ref())?;
        let load_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        info!(service = SERVICE, engine_id = %self.info.engine_id, load_ms, "Speech engine loaded on first use");
        *loaded = Some(engine.clone());
        Ok(engine)
    }

    /// Clips are cached under the promised id; a model that turns out different must never fill that cache.
    fn check(&self, engine: &dyn SpeechEngine) -> AppResult<()> {
        let info = &self.info;
        if engine.id() == info.engine_id && engine.revision() == info.revision && engine.sample_rate() == info.sample_rate {
            return Ok(());
        }
        let detail = format!(
            "the loaded engine is {} / {} at {} Hz, expected {} / {} at {} Hz",
            engine.id(),
            engine.revision(),
            engine.sample_rate(),
            info.engine_id,
            info.revision,
            info.sample_rate
        );
        Err(AppError::new(ErrorName::SpeechEngineUnavailable).with_detail(detail))
    }
}

impl SpeechEngine for DeferredSpeechEngine {
    fn id(&self) -> &str {
        &self.info.engine_id
    }

    fn voices(&self) -> Vec<Voice> {
        self.info.voices.clone()
    }

    fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }

    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
        self.engine()?.synthesize(request)
    }

    fn revision(&self) -> &str {
        &self.info.revision
    }

    fn is_loaded(&self) -> bool {
        Self::is_loaded(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use lectrice_domain::{VoiceGender, VoiceId, VoiceLanguage};

    use super::*;

    const SAMPLE_RATE: u32 = 24_000;

    struct Model {
        engine_id: &'static str,
    }

    impl SpeechEngine for Model {
        fn id(&self) -> &str {
            self.engine_id
        }

        fn voices(&self) -> Vec<Voice> {
            Vec::new()
        }

        fn sample_rate(&self) -> u32 {
            SAMPLE_RATE
        }

        fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
            Ok(Audio { samples: vec![0.0; request.text.len()], sample_rate: SAMPLE_RATE })
        }

        fn revision(&self) -> &str {
            "rev-1"
        }
    }

    fn info() -> DeferredEngineInfo {
        let voice = Voice { id: VoiceId::new("af_heart"), name: "Heart".into(), language: VoiceLanguage::EnUs, gender: VoiceGender::Female };
        DeferredEngineInfo { engine_id: "bundle".into(), revision: "rev-1".into(), sample_rate: SAMPLE_RATE, voices: vec![voice] }
    }

    fn deferred(loads: &Arc<AtomicUsize>, engine_id: &'static str) -> DeferredSpeechEngine {
        let loads = loads.clone();
        DeferredSpeechEngine::new(info(), move || {
            loads.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(Model { engine_id }) as Arc<dyn SpeechEngine>)
        })
    }

    fn speak(engine: &DeferredSpeechEngine) -> AppResult<Audio> {
        engine.synthesize(SpeechRequest { text: "Hello.".into(), voice_id: VoiceId::new("af_heart") })
    }

    #[test]
    fn answers_everything_but_speech_without_loading() {
        let loads = Arc::new(AtomicUsize::new(0));
        let engine = deferred(&loads, "bundle");

        assert_eq!((engine.id(), engine.revision(), engine.sample_rate(), engine.voices().len()), ("bundle", "rev-1", SAMPLE_RATE, 1));
        assert!(!engine.is_loaded());
        assert_eq!(loads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn loads_once_on_first_speech_or_preload() {
        let loads = Arc::new(AtomicUsize::new(0));
        let engine = Arc::new(deferred(&loads, "bundle"));

        let speakers: Vec<_> = (0..4)
            .map(|_| {
                let engine = engine.clone();
                std::thread::spawn(move || speak(&engine).unwrap())
            })
            .collect();
        speakers.into_iter().for_each(|speaker| assert_eq!(speaker.join().unwrap().samples.len(), 6));
        engine.preload().unwrap();

        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(engine.unload());
        engine.preload().unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_model_that_isnt_the_promised_one_is_refused() {
        let loads = Arc::new(AtomicUsize::new(0));
        let engine = deferred(&loads, "another-bundle");

        assert_eq!(speak(&engine).unwrap_err().primary(), ErrorName::SpeechEngineUnavailable);
        assert!(!engine.is_loaded());
    }

    #[test]
    fn a_failed_load_is_tried_again_next_time() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let engine = DeferredSpeechEngine::new(info(), move || {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(AppError::new(ErrorName::SpeechEngineUnavailable));
            }
            Ok(Arc::new(Model { engine_id: "bundle" }) as Arc<dyn SpeechEngine>)
        });

        assert!(speak(&engine).is_err());
        assert!(speak(&engine).is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
