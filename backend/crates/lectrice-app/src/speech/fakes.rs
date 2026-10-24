//! Fakes for the speech tests: an engine that can be held on one text, an in-memory cache and a tiny encoder.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioEncoder, AudioEviction, AudioKey, AudioRetention, CacheUsage, Document, DocumentBytes, DocumentCacheUsage,
    DocumentId, DocumentReader, DocumentSummary, ErrorName, ListOptions, Page, SentenceId, SpeechEngine, SpeechRequest, Timestamp, Voice, VoiceGender, VoiceId,
    VoiceLanguage,
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

const MEMORY_CACHE_CAP_BYTES: u64 = 1 << 20;

/// Also records every `retain_around` call (it never evicts) and counts refreshes.
#[derive(Default)]
pub(crate) struct MemoryCache {
    clips: Mutex<HashMap<AudioKey, Vec<u8>>>,
    puts: AtomicUsize,
    refreshes: AtomicUsize,
    retained: Mutex<Vec<(AudioKey, f64)>>,
}

impl MemoryCache {
    pub fn puts(&self) -> usize {
        self.puts.load(Ordering::SeqCst)
    }

    pub fn refreshes(&self) -> usize {
        self.refreshes.load(Ordering::SeqCst)
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

    fn usage(&self) -> AppResult<CacheUsage> {
        let clips = self.clips.lock().unwrap();
        let mut documents: HashMap<DocumentId, DocumentCacheUsage> = HashMap::new();
        for (key, bytes) in clips.iter() {
            let usage =
                documents.entry(key.document_id.clone()).or_insert_with(|| DocumentCacheUsage { document_id: key.document_id.clone(), bytes: 0, clips: 0 });
            usage.bytes += bytes.len() as u64;
            usage.clips += 1;
        }
        let bytes = clips.values().map(|clip| clip.len() as u64).sum();
        Ok(CacheUsage { bytes, clips: clips.len(), cap_bytes: MEMORY_CACHE_CAP_BYTES, documents: documents.into_values().collect() })
    }

    fn clear(&self) -> AppResult<AudioEviction> {
        let mut clips = self.clips.lock().unwrap();
        let eviction = AudioEviction { cap_clips: clips.len(), freed_bytes: clips.values().map(|clip| clip.len() as u64).sum(), ..AudioEviction::default() };
        clips.clear();
        Ok(eviction)
    }

    fn remove_document(&self, id: &DocumentId) -> AppResult<AudioEviction> {
        let mut clips = self.clips.lock().unwrap();
        let before = clips.len();
        clips.retain(|key, _| key.document_id != *id);
        Ok(AudioEviction { cap_clips: before - clips.len(), ..AudioEviction::default() })
    }

    fn refresh(&self) -> AppResult<()> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        Ok(())
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

/// Lists every document until it is removed; holds no documents itself (the read-ahead only asks for summaries).
#[derive(Default)]
pub(crate) struct ListedReader {
    removed: Mutex<HashSet<DocumentId>>,
}

impl ListedReader {
    pub fn remove(&self, id: &DocumentId) {
        self.removed.lock().unwrap().insert(id.clone());
    }
}

impl DocumentReader for ListedReader {
    fn find(&self, _id: &DocumentId) -> AppResult<Option<Document>> {
        Ok(None)
    }

    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
        if self.removed.lock().unwrap().contains(id) {
            return Ok(None);
        }
        let at = Timestamp::UNIX_EPOCH;
        Ok(Some(DocumentSummary {
            id: id.clone(),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            sentence_count: 0,
            cleaner_version: 1,
            added_at: at,
            last_opened_at: at,
            last_sentence_id: None,
        }))
    }

    fn find_original_pdf(&self, _id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
        Ok(Page { items: Vec::new(), total: 0, limit: options.limit_or_default(), offset: 0 })
    }

    fn list_removed(&self) -> AppResult<Vec<DocumentId>> {
        Ok(self.removed.lock().unwrap().iter().cloned().collect())
    }

    fn library_usage(&self) -> AppResult<Vec<DocumentBytes>> {
        Ok(Vec::new())
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
