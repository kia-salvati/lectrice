//! Fakes for the speech tests: an engine that can be held on one text, an in-memory cache and a tiny encoder.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioEncoder, AudioEviction, AudioKey, AudioRetention, DocumentId, ErrorName, SentenceId, SpeechEngine,
    SpeechRequest, Voice, VoiceGender, VoiceId, VoiceLanguage,
};

pub(crate) const ENGINE_ID: &str = "fake-engine";
pub(crate) const SAMPLE_RATE: u32 = 24_000;
const SAMPLES_PER_CHAR: usize = 100;

pub(crate) fn key_for(sentence: u32) -> AudioKey {
    AudioKey {
        document_id: DocumentId::parse(&format!("{:064x}", 1)).unwrap(),
        cleaner_version: 1,
        engine_id: ENGINE_ID.to_string(),
        voice_id: VoiceId::new("af_heart"),
        sentence_id: SentenceId(sentence),
    }
}

#[derive(Default)]
struct Gate {
    blocked: bool,
    released: bool,
}

/// Records every text it speaks. With a gate text, it blocks on that text until [`GatedEngine::release`].
/// Texts `s900` and `s901` fail and panic.
pub(crate) struct GatedEngine {
    gate_text: Option<String>,
    gate: Mutex<Gate>,
    changed: Condvar,
    spoken: Mutex<Vec<String>>,
}

impl GatedEngine {
    pub const FAILING: u32 = 900;
    pub const PANICKING: u32 = 901;

    pub fn new(gate_text: &str) -> Self {
        Self { gate_text: Some(gate_text.to_string()), ..Self::open() }
    }

    pub fn open() -> Self {
        Self { gate_text: None, gate: Mutex::default(), changed: Condvar::new(), spoken: Mutex::default() }
    }

    pub fn wait_until_blocked(&self) {
        let mut gate = self.gate.lock().unwrap();
        while !gate.blocked {
            gate = self.changed.wait(gate).unwrap();
        }
    }

    pub fn release(&self) {
        self.gate.lock().unwrap().released = true;
        self.changed.notify_all();
    }

    pub fn spoken(&self) -> Vec<String> {
        self.spoken.lock().unwrap().clone()
    }

    fn hold_if_gated(&self, text: &str) {
        if self.gate_text.as_deref() != Some(text) {
            return;
        }
        let mut gate = self.gate.lock().unwrap();
        gate.blocked = true;
        self.changed.notify_all();
        while !gate.released {
            gate = self.changed.wait(gate).unwrap();
        }
    }
}

impl SpeechEngine for GatedEngine {
    fn id(&self) -> &str {
        ENGINE_ID
    }

    fn voices(&self) -> Vec<Voice> {
        vec![
            Voice { id: VoiceId::new("af_heart"), name: "Heart".into(), language: VoiceLanguage::EnUs, gender: VoiceGender::Female },
            Voice { id: VoiceId::new("bm_lewis"), name: "Lewis".into(), language: VoiceLanguage::EnGb, gender: VoiceGender::Male },
        ]
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
        self.hold_if_gated(&request.text);
        if request.text == format!("s{}", Self::PANICKING) {
            panic!("fake engine panic");
        }
        if request.text == format!("s{}", Self::FAILING) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail("fake failure"));
        }
        self.spoken.lock().unwrap().push(request.text.clone());
        Ok(Audio { samples: vec![0.1; request.text.len() * SAMPLES_PER_CHAR], sample_rate: SAMPLE_RATE })
    }
}

/// Also records every `retain_around` call (it never evicts).
#[derive(Default)]
pub(crate) struct MemoryCache {
    clips: Mutex<HashMap<AudioKey, Vec<u8>>>,
    puts: AtomicUsize,
    retained: Mutex<Vec<(AudioKey, f64)>>,
}

impl MemoryCache {
    pub fn puts(&self) -> usize {
        self.puts.load(Ordering::SeqCst)
    }

    pub fn keys(&self) -> Vec<AudioKey> {
        self.clips.lock().unwrap().keys().cloned().collect()
    }

    pub fn retained(&self) -> Vec<(AudioKey, f64)> {
        self.retained.lock().unwrap().clone()
    }
}

impl AudioRetention for MemoryCache {
    fn contains(&self, key: &AudioKey) -> AppResult<bool> {
        Ok(self.clips.lock().unwrap().contains_key(key))
    }

    fn retain_around(&self, position: &AudioKey, keep_seconds: f64) -> AppResult<AudioEviction> {
        self.retained.lock().unwrap().push((position.clone(), keep_seconds));
        Ok(AudioEviction::default())
    }
}

impl AudioCache for MemoryCache {
    fn get(&self, key: &AudioKey) -> AppResult<Option<Vec<u8>>> {
        Ok(self.clips.lock().unwrap().get(key).cloned())
    }

    fn put(&self, key: &AudioKey, bytes: &[u8]) -> AppResult<()> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.clips.lock().unwrap().insert(key.clone(), bytes.to_vec());
        Ok(())
    }
}

/// Encodes audio as `"<sample rate>:<sample count>"`.
pub(crate) struct CountingEncoder;

impl AudioEncoder for CountingEncoder {
    fn mime_type(&self) -> &str {
        "audio/test"
    }

    fn encode(&self, audio: &Audio) -> AppResult<Vec<u8>> {
        Ok(format!("{}:{}", audio.sample_rate, audio.samples.len()).into_bytes())
    }
}
