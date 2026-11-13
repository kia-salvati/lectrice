//! Prepare (spec 19 C21): make a range of a book's audio ahead of time, into the sentence cache, for phones that make
//! speech slower than real time. One job per document; it runs on its own thread, one sentence at a time at export
//! priority (listening comes first), skips what is already cached, and pins the range so the listener's cache window
//! doesn't delete it. The shell decides when it runs (e.g. while charging) and lifts the pin.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioEncoder, AudioKey, AudioRetention, DocumentId, DocumentReader, ErrorName, SentenceId, SpeechEngine, SpeechRequest,
    VoiceId,
};
use tracing::{info, warn};

use super::read_ahead_plan::playable_in_order;
use super::worker::{SpeechJob, SpeechPriority, SpeechWorker};

const NAME: &str = "Prepare";
const THREAD_NAME: &str = "prepare";

#[derive(Debug, Clone)]
pub struct PrepareArgs {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub read_skipped: bool,
    /// Inclusive sentence ids; only the playable sentences between them are made.
    pub from: SentenceId,
    pub to: SentenceId,
}

/// Progress of a document's prepare job, counted in playable sentences of its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrepareStatus {
    pub ready: u32,
    pub total: u32,
    pub running: bool,
}

pub struct PrepareDeps {
    pub engine: Arc<dyn SpeechEngine>,
    pub worker: Arc<SpeechWorker>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub retention: Arc<dyn AudioRetention>,
    pub encoder: Arc<dyn AudioEncoder>,
}

struct Job {
    stop: Arc<AtomicBool>,
    ready: Arc<AtomicU32>,
    total: u32,
    running: Arc<AtomicBool>,
}

/// Starts, reports and stops prepare jobs.
pub struct PrepareJobs {
    deps: Arc<PrepareDeps>,
    jobs: Mutex<HashMap<DocumentId, Job>>,
}

impl PrepareJobs {
    pub fn new(deps: PrepareDeps) -> Self {
        Self { deps: Arc::new(deps), jobs: Mutex::default() }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<DocumentId, Job>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Replaces the document's job (and pin) with this range; already-cached sentences count as ready at once.
    pub fn start(&self, args: PrepareArgs) -> AppResult<PrepareStatus> {
        let PrepareArgs { document_id, voice_id, read_skipped, from, to } = args;
        if !self.deps.engine.voices().iter().any(|voice| voice.id == voice_id) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")));
        }
        let document = self.deps.reader.find(&document_id)?.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        let (low, high) = (from.0.min(to.0), from.0.max(to.0));
        let sentences: Vec<(SentenceId, String)> = playable_in_order(&document, read_skipped)
            .into_iter()
            .filter(|(_, sentence)| (low..=high).contains(&sentence.id.0) && !sentence.spoken_text().trim().is_empty())
            .map(|(_, sentence)| (sentence.id, sentence.spoken_text().to_string()))
            .collect();
        let scope = AudioKey {
            document_id: document_id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: self.deps.engine.id().to_string(),
            voice_id,
            sentence_id: SentenceId(low),
        };
        self.deps.retention.pin(&scope, SentenceId(low), SentenceId(high))?;
        self.stop(&document_id);

        let total = u32::try_from(sentences.len()).unwrap_or(u32::MAX);
        let job = Job { stop: Arc::default(), ready: Arc::default(), total, running: Arc::new(AtomicBool::new(true)) };
        let (stop, ready, running, deps) = (job.stop.clone(), job.ready.clone(), job.running.clone(), self.deps.clone());
        info!(service = NAME, doc_id = %document_id, sentences = total, "prepare started");
        std::thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || run(&deps, &scope, &sentences, &stop, &ready, &running))
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the prepare thread").with_source(error))?;
        let status = PrepareStatus { ready: 0, total, running: true };
        self.lock().insert(document_id, job);
        Ok(status)
    }

    /// Stops the document's job after the sentence being made; the pin and the made clips stay.
    pub fn stop(&self, document_id: &DocumentId) {
        if let Some(job) = self.lock().remove(document_id) {
            job.stop.store(true, Ordering::SeqCst);
        }
    }

    /// Stops the job and lifts the pin (the user turned Prepare off, or listening passed the range).
    pub fn cancel(&self, document_id: &DocumentId) -> AppResult<()> {
        self.stop(document_id);
        self.deps.retention.unpin(document_id)
    }

    pub fn status(&self, document_id: &DocumentId) -> PrepareStatus {
        self.lock().get(document_id).map_or_else(PrepareStatus::default, |job| PrepareStatus {
            ready: job.ready.load(Ordering::SeqCst),
            total: job.total,
            running: job.running.load(Ordering::SeqCst),
        })
    }
}

fn run(deps: &PrepareDeps, scope: &AudioKey, sentences: &[(SentenceId, String)], stop: &AtomicBool, ready: &AtomicU32, running: &AtomicBool) {
    for (sentence_id, text) in sentences {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let key = AudioKey { sentence_id: *sentence_id, ..scope.clone() };
        if !matches!(deps.retention.contains(&key), Ok(true)) {
            let job =
                SpeechJob { key: key.clone(), request: SpeechRequest { text: text.clone(), voice_id: key.voice_id.clone() }, priority: SpeechPriority::Export };
            let stored = deps.worker.submit(job).wait().and_then(|audio| deps.encoder.encode(&audio)).and_then(|bytes| deps.cache.put(&key, &bytes));
            if let Err(error) = stored {
                warn!(service = NAME, doc_id = %key.document_id, sentence_id = sentence_id.0, error = %error, "prepare stopped");
                break;
            }
        }
        ready.fetch_add(1, Ordering::SeqCst);
    }
    running.store(false, Ordering::SeqCst);
    info!(service = NAME, doc_id = %scope.document_id, ready = ready.load(Ordering::SeqCst), total = sentences.len(), "prepare finished");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::fakes::{CountingEncoder, GatedEngine, MemoryCache};
    use lectrice_domain::{Block, BlockKind, Document, DocumentBytes, DocumentSummary, ListOptions, Page, Sentence};
    use std::time::{Duration, Instant};

    /// Serves one invented document.
    struct OneBook(Document);

    impl DocumentReader for OneBook {
        fn find(&self, id: &DocumentId) -> AppResult<Option<Document>> {
            Ok((*id == self.0.id).then(|| self.0.clone()))
        }

        fn find_summary(&self, _id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
            Ok(None)
        }

        fn find_original_pdf(&self, _id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
            Ok(None)
        }

        fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
            Ok(Page { items: Vec::new(), total: 0, limit: options.limit_or_default(), offset: 0 })
        }

        fn list_removed(&self) -> AppResult<Vec<DocumentId>> {
            Ok(Vec::new())
        }

        fn library_usage(&self) -> AppResult<Vec<DocumentBytes>> {
            Ok(Vec::new())
        }
    }

    fn book(sentences: u32) -> Document {
        let blocks = (0..sentences)
            .map(|id| Block {
                id,
                kind: BlockKind::Paragraph,
                level: None,
                page_index: 1,
                spoken: true,
                text: None,
                sentences: vec![Sentence { id: SentenceId(id), text: format!("{id:04} words to say."), page_index: 1, speech_text: None }],
                cue: None,
            })
            .collect();
        Document {
            id: DocumentId::parse(&format!("{:064x}", 7)).unwrap(),
            title: "The Long Book".into(),
            source_file_name: "long.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: Vec::new(),
            toc: Vec::new(),
            blocks,
        }
    }

    #[test]
    fn a_range_is_made_into_the_cache_skipping_what_is_cached_and_stays_pinned() {
        let document = book(6);
        let engine = Arc::new(GatedEngine::open());
        let cache = Arc::new(MemoryCache::default());
        let jobs = PrepareJobs::new(PrepareDeps {
            engine: engine.clone(),
            worker: Arc::new(SpeechWorker::start(engine.clone()).unwrap()),
            reader: Arc::new(OneBook(document.clone())),
            cache: cache.clone(),
            retention: cache.clone(),
            encoder: Arc::new(CountingEncoder),
        });
        let cached = AudioKey {
            document_id: document.id.clone(),
            cleaner_version: 1,
            engine_id: engine.id().to_string(),
            voice_id: VoiceId::new("af_heart"),
            sentence_id: SentenceId(2),
        };
        cache.put(&cached, b"cached").unwrap();

        let args =
            PrepareArgs { document_id: document.id.clone(), voice_id: VoiceId::new("af_heart"), read_skipped: false, from: SentenceId(1), to: SentenceId(3) };
        assert_eq!(jobs.start(args).unwrap().total, 3);

        let started = Instant::now();
        while jobs.status(&document.id).running {
            assert!(started.elapsed() < Duration::from_secs(5), "prepare didn't finish");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(jobs.status(&document.id).ready, 3);
        let spoken: Vec<u32> = engine.spoken().iter().map(|text| text[..4].parse().unwrap()).collect();
        assert_eq!(spoken, [1, 3]);
        assert_eq!(cache.pins(), [(document.id.clone(), 1, 3)]);
        jobs.cancel(&document.id).unwrap();
        assert!(cache.pins().is_empty());
    }
}
