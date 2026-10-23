//! The export job registry: every job's state in memory and on disk, and one runner thread that exports queued jobs
//! one at a time (the speech worker makes one sentence at a time anyway).
//!
//! States: `queued → running → done | failed | cancelled`, and `queued → cancelled`. Jobs found queued or running
//! when the registry starts were interrupted by a stop of the server, so they become `failed`.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioExport, AudioExporter, Clock, DocumentId, DocumentReader, ErrorName, ExportFormat, ExportId,
    ExportRecord, ExportRecordStore, ExportStatus, SpeechEngine, VoiceId,
};
use tracing::{error, info, warn};

use super::run::{ExportRunner, ExportRunnerDeps, Progress, RunOutcome};
use crate::speech::SpeechWorker;

const NAME: &str = "ExportJobs";
const THREAD_NAME: &str = "export-runner";

pub struct ExportDeps {
    pub worker: Arc<SpeechWorker>,
    pub engine: Arc<dyn SpeechEngine>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub decoder: Arc<dyn AudioDecoder>,
    pub exporter: Arc<dyn AudioExporter>,
    pub store: Arc<dyn ExportRecordStore>,
    pub clock: Arc<dyn Clock>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExportSettings {
    /// Exports only the first sentences when set (a short export for testing).
    pub max_sentences: Option<u32>,
}

/// A job to create. `sentences_total` comes from the export plan.
#[derive(Debug, Clone)]
pub struct NewExport {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub read_skipped: bool,
    pub sentences_total: u32,
}

pub struct ExportJobs {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    registry: Mutex<Registry>,
    wake: Condvar,
    store: Arc<dyn ExportRecordStore>,
    clock: Arc<dyn Clock>,
}

#[derive(Default)]
struct Registry {
    jobs: HashMap<ExportId, Job>,
    queue: VecDeque<ExportId>,
    shutdown: bool,
    next_number: u64,
}

struct Job {
    record: ExportRecord,
    cancel: Arc<AtomicBool>,
}

struct Claimed {
    export: AudioExport,
    read_skipped: bool,
    cancel: Arc<AtomicBool>,
}

impl ExportJobs {
    /// Loads the saved jobs (marking interrupted ones `failed`) and starts the runner thread.
    pub fn start(deps: ExportDeps, settings: ExportSettings) -> AppResult<Self> {
        let ExportDeps { worker, engine, reader, cache, decoder, exporter, store, clock } = deps;
        let runner = ExportRunner::new(ExportRunnerDeps { worker, engine, reader, cache, decoder, exporter }, settings.max_sentences);
        let records = store.load_all()?;
        let mut registry = Registry::default();
        let mut interrupted = 0;
        for mut record in records {
            if record.export.status.is_active() {
                record.export.status = ExportStatus::Failed;
                record.export.estimated_seconds_left = None;
                store.save(&record)?;
                interrupted += 1;
            }
            registry.jobs.insert(record.export.id.clone(), Job { record, cancel: Arc::default() });
        }
        info!(service = NAME, jobs = registry.jobs.len(), interrupted, "export jobs loaded");

        let shared = Arc::new(Shared { registry: Mutex::new(registry), wake: Condvar::new(), store, clock });
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || run_loop(&thread_shared, &runner))
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the export runner thread").with_source(error))?;
        Ok(Self { shared, thread: Some(thread) })
    }

    /// Queues a new job, or returns the queued or running job for the same document, voice and format (one file each).
    /// The flag is true when the job is new.
    pub fn enqueue(&self, new: NewExport) -> AppResult<(AudioExport, bool)> {
        let mut registry = self.shared.lock();
        let same_target = |job: &&Job| {
            let export = &job.record.export;
            export.status.is_active() && export.document_id == new.document_id && export.voice_id == new.voice_id && export.format == new.format
        };
        if let Some(existing) = registry.jobs.values().find(same_target) {
            return Ok((existing.record.export.clone(), false));
        }

        let created_at = self.shared.clock.now();
        let id = registry.new_id(created_at.timestamp());
        let export = AudioExport {
            id: id.clone(),
            document_id: new.document_id,
            voice_id: new.voice_id,
            format: new.format,
            status: ExportStatus::Queued,
            sentences_done: 0,
            sentences_total: new.sentences_total,
            estimated_seconds_left: None,
        };
        let record = ExportRecord { export: export.clone(), read_skipped: new.read_skipped, created_at, file: None };
        self.shared.store.save(&record)?;
        info!(service = NAME, export_id = %id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), sentences_total = export.sentences_total, "export job saved and queued");
        registry.jobs.insert(id.clone(), Job { record, cancel: Arc::default() });
        registry.queue.push_back(id);
        drop(registry);
        self.shared.wake.notify_all();
        Ok((export, true))
    }

    pub fn get(&self, id: &ExportId) -> Option<AudioExport> {
        self.shared.lock().jobs.get(id).map(|job| job.record.export.clone())
    }

    pub fn record(&self, id: &ExportId) -> Option<ExportRecord> {
        self.shared.lock().jobs.get(id).map(|job| job.record.clone())
    }

    /// Newest first.
    pub fn list(&self, document_id: &DocumentId) -> Vec<AudioExport> {
        let registry = self.shared.lock();
        let mut exports: Vec<AudioExport> =
            registry.jobs.values().map(|job| &job.record.export).filter(|export| export.document_id == *document_id).cloned().collect();
        exports.sort_by(|a, b| b.id.as_str().cmp(a.id.as_str()));
        exports
    }

    /// Cancels a queued or running job and returns its status afterwards (finished jobs are left as they are).
    /// A running job stops at its next sentence; its partial file is removed.
    pub fn cancel(&self, id: &ExportId) -> AppResult<ExportStatus> {
        let mut registry = self.shared.lock();
        let Some(job) = registry.jobs.get_mut(id) else {
            return Err(AppError::new(ErrorName::ExportNotFound).with_detail(format!("export {id}")));
        };
        let previous = job.record.export.status;
        if !previous.is_active() {
            return Ok(previous);
        }
        job.cancel.store(true, Ordering::SeqCst);
        job.record.export.status = ExportStatus::Cancelled;
        job.record.export.estimated_seconds_left = None;
        self.shared.store.save(&job.record)?;
        registry.queue.retain(|queued| queued != id);
        info!(service = NAME, export_id = %id, previous = ?previous, "export cancelled and saved");
        Ok(ExportStatus::Cancelled)
    }
}

impl Drop for ExportJobs {
    /// Stops the runner; a running export is abandoned and shows as `failed`.
    fn drop(&mut self) {
        let mut registry = self.shared.lock();
        registry.shutdown = true;
        for job in registry.jobs.values() {
            job.cancel.store(true, Ordering::SeqCst);
        }
        drop(registry);
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Registry {
    /// Sorts by creation: seconds, then a per-process counter; the random tail keeps ids unique across restarts.
    fn new_id(&mut self, created_seconds: i64) -> ExportId {
        self.next_number += 1;
        let random = RandomState::new().hash_one(self.next_number) as u32;
        ExportId::new(format!("{created_seconds:010x}{:06x}{random:08x}", self.next_number))
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for the next queued job and marks it running. `None` once the registry shuts down.
    fn claim_next(&self) -> Option<Claimed> {
        let mut registry = self.lock();
        loop {
            if registry.shutdown {
                return None;
            }
            let Some(id) = registry.queue.pop_front() else {
                registry = self.wake.wait(registry).unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let Some(job) = registry.jobs.get_mut(&id) else { continue };
            if job.record.export.status != ExportStatus::Queued {
                continue;
            }
            job.record.export.status = ExportStatus::Running;
            self.save(&job.record);
            return Some(Claimed { export: job.record.export.clone(), read_skipped: job.record.read_skipped, cancel: job.cancel.clone() });
        }
    }

    fn update_progress(&self, id: &ExportId, progress: Progress) {
        let mut registry = self.lock();
        let Some(job) = registry.jobs.get_mut(id) else { return };
        if job.record.export.status != ExportStatus::Running {
            return;
        }
        job.record.export.sentences_done = progress.sentences_done;
        job.record.export.estimated_seconds_left = progress.estimated_seconds_left;
        if progress.checkpoint {
            self.save(&job.record);
        }
    }

    fn complete(&self, id: &ExportId, outcome: RunOutcome) {
        let mut registry = self.lock();
        let shutdown = registry.shutdown;
        let Some(job) = registry.jobs.get_mut(id) else { return };
        let export = &mut job.record.export;
        export.estimated_seconds_left = None;
        match outcome {
            _ if shutdown => {
                export.status = ExportStatus::Failed;
                warn!(service = NAME, export_id = %id, "export interrupted by shutdown");
            }
            _ if export.status == ExportStatus::Cancelled => {
                info!(service = NAME, export_id = %id, sentences_done = export.sentences_done, "export stopped after cancel");
            }
            RunOutcome::Done(file) => {
                export.status = ExportStatus::Done;
                export.sentences_done = export.sentences_total;
                export.estimated_seconds_left = Some(0);
                job.record.file = Some(file.path);
                info!(service = NAME, export_id = %id, bytes = file.size_bytes, "export done");
            }
            RunOutcome::Cancelled => {
                export.status = ExportStatus::Cancelled;
                info!(service = NAME, export_id = %id, sentences_done = export.sentences_done, "export stopped");
            }
            RunOutcome::Failed(failure) => {
                export.status = ExportStatus::Failed;
                error!(service = NAME, export_id = %id, error = %failure, cause = ?std::error::Error::source(&failure), "export failed");
            }
        }
        let record = job.record.clone();
        self.save(&record);
        if record.export.status == ExportStatus::Done {
            self.drop_replaced(&mut registry, &record.export);
        }
    }

    /// An export for the same document, voice and format replaces the file, so older finished jobs for it go.
    fn drop_replaced(&self, registry: &mut Registry, newest: &AudioExport) {
        let replaced: Vec<ExportId> = registry
            .jobs
            .values()
            .map(|job| &job.record.export)
            .filter(|export| {
                export.id != newest.id
                    && export.status == ExportStatus::Done
                    && export.document_id == newest.document_id
                    && export.voice_id == newest.voice_id
                    && export.format == newest.format
            })
            .map(|export| export.id.clone())
            .collect();
        for id in replaced {
            registry.jobs.remove(&id);
            match self.store.delete(&id) {
                Ok(()) => info!(service = NAME, export_id = %id, replaced_by = %newest.id, "replaced export job deleted"),
                Err(failure) => warn!(service = NAME, export_id = %id, error = %failure, "replaced export job could not be deleted"),
            }
        }
    }

    /// Saves from the runner thread can't fail a request; a failed save is logged and retried at the next change.
    fn save(&self, record: &ExportRecord) {
        if let Err(failure) = self.store.save(record) {
            warn!(service = NAME, export_id = %record.export.id, status = ?record.export.status, error = %failure, "export job could not be saved");
        }
    }
}

fn run_loop(shared: &Shared, runner: &ExportRunner) {
    while let Some(claimed) = shared.claim_next() {
        let id = claimed.export.id.clone();
        let outcome = runner.run(&claimed.export, claimed.read_skipped, &claimed.cancel, &mut |progress| shared.update_progress(&id, progress));
        shared.complete(&id, outcome);
    }
}
