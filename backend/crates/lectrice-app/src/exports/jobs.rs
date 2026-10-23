//! The export job registry: every job's state in memory and on disk, and one runner thread that exports queued jobs
//! one at a time (the speech worker makes one sentence at a time anyway).
//!
//! States: `queued → running → done | failed | cancelled`, `queued | running → paused → queued` (pause, resume) and
//! `queued | paused → cancelled`. A paused job keeps its finished parts; resuming continues at the first unfinished
//! one. Jobs found queued or running when the registry starts were interrupted by a stop, so they become `paused`.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioExport, AudioExporter, ChapterMark, Clock, DocumentId, DocumentReader, ErrorName, ExportFormat,
    ExportId, ExportPart, ExportRecord, ExportRecordStore, ExportStatus, SpeechEngine, VoiceId,
};
use tracing::{error, info, warn};

use super::run::{ExportRunner, ExportRunnerDeps, Progress, RunOutcome, RunReport};
use crate::speech::SpeechWorker;

const NAME: &str = "ExportJobs";
const THREAD_NAME: &str = "export-runner";
/// Listening minutes per export part (Android spec §8.3): at most this much work is lost when an export stops.
const PART_MINUTES: u32 = 10;

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

#[derive(Debug, Clone, Copy)]
pub struct ExportSettings {
    /// Exports only the first sentences when set (a short export for testing).
    pub max_sentences: Option<u32>,
    /// Listening minutes per part; 0 puts every sentence in its own part.
    pub part_minutes: u32,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { max_sentences: None, part_minutes: PART_MINUTES }
    }
}

/// A job to create. The totals come from the export plan.
#[derive(Debug, Clone)]
pub struct NewExport {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub read_skipped: bool,
    pub sentences_total: u32,
    pub parts_total: u32,
}

pub struct ExportJobs {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    registry: Mutex<Registry>,
    wake: Condvar,
    store: Arc<dyn ExportRecordStore>,
    exporter: Arc<dyn AudioExporter>,
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
    /// Tells a running export to stop at its next sentence. Replaced when the job is queued again.
    stop: Arc<AtomicBool>,
}

struct Claimed {
    record: ExportRecord,
    stop: Arc<AtomicBool>,
}

impl ExportJobs {
    /// Loads the saved jobs (marking interrupted ones `paused`) and starts the runner thread. Nothing runs until a job
    /// is queued: call [`ExportJobs::resume_all`] to continue the paused ones.
    pub fn start(deps: ExportDeps, settings: ExportSettings) -> AppResult<Self> {
        let ExportDeps { worker, engine, reader, cache, decoder, exporter, store, clock } = deps;
        let runner = ExportRunner::new(ExportRunnerDeps { worker, engine, reader, cache, decoder, exporter: exporter.clone() }, settings);
        let records = store.load_all()?;
        let mut registry = Registry::default();
        let mut interrupted = 0;
        for mut record in records {
            if record.export.status.is_active() {
                record.export.status = ExportStatus::Paused;
                record.export.estimated_seconds_left = None;
                store.save(&record)?;
                interrupted += 1;
            }
            registry.jobs.insert(record.export.id.clone(), Job { record, stop: Arc::default() });
        }
        info!(service = NAME, jobs = registry.jobs.len(), interrupted, "export jobs loaded");

        let shared = Arc::new(Shared { registry: Mutex::new(registry), wake: Condvar::new(), store, exporter, clock });
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || run_loop(&thread_shared, &runner))
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the export runner thread").with_source(error))?;
        Ok(Self { shared, thread: Some(thread) })
    }

    /// Queues a new job, or returns the unfinished job for the same document, voice and format (one file each); a
    /// paused one is queued again. The flag is true when the job is new.
    pub fn enqueue(&self, new: NewExport) -> AppResult<(AudioExport, bool)> {
        let mut registry = self.shared.lock();
        let same_target = |job: &Job| {
            let export = &job.record.export;
            export.status.is_unfinished() && export.document_id == new.document_id && export.voice_id == new.voice_id && export.format == new.format
        };
        if let Some(existing) = registry.jobs.values().find(|job| same_target(job)).map(|job| job.record.export.id.clone()) {
            let export = self.shared.requeue(&mut registry, &existing)?;
            drop(registry);
            self.shared.wake.notify_all();
            return Ok((export, false));
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
            parts_done: 0,
            parts_total: new.parts_total,
            estimated_seconds_left: None,
        };
        let record = ExportRecord { export: export.clone(), read_skipped: new.read_skipped, created_at, file: None, parts: Vec::new(), chapters: Vec::new() };
        self.shared.store.save(&record)?;
        info!(service = NAME, export_id = %id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), sentences_total = export.sentences_total, parts_total = export.parts_total, "export job saved and queued");
        registry.jobs.insert(id.clone(), Job { record, stop: Arc::default() });
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

    /// Pauses a queued or running job and returns it (other jobs are left as they are). A running job stops at its
    /// next sentence; its finished parts are kept and the part it was writing is dropped.
    pub fn pause(&self, id: &ExportId) -> AppResult<AudioExport> {
        let mut registry = self.shared.lock();
        let job = registry.find_mut(id)?;
        let previous = job.record.export.status;
        if !previous.is_active() {
            return Ok(job.record.export.clone());
        }
        job.stop.store(true, Ordering::SeqCst);
        job.record.export.status = ExportStatus::Paused;
        job.record.export.estimated_seconds_left = None;
        self.shared.store.save(&job.record)?;
        let export = job.record.export.clone();
        registry.queue.retain(|queued| queued != id);
        info!(service = NAME, export_id = %id, previous = ?previous, parts_done = export.parts_done, "export paused and saved");
        Ok(export)
    }

    /// Queues every paused job again, oldest first, and returns the jobs that are now queued or running.
    pub fn resume_all(&self) -> AppResult<Vec<AudioExport>> {
        let mut registry = self.shared.lock();
        let mut paused: Vec<ExportId> =
            registry.jobs.values().filter(|job| job.record.export.status == ExportStatus::Paused).map(|job| job.record.export.id.clone()).collect();
        paused.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for id in &paused {
            self.shared.requeue(&mut registry, id)?;
        }
        let mut active: Vec<AudioExport> = registry.jobs.values().map(|job| &job.record.export).filter(|export| export.status.is_active()).cloned().collect();
        active.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        drop(registry);
        self.shared.wake.notify_all();
        info!(service = NAME, resumed = paused.len(), active = active.len(), "paused exports queued again");
        Ok(active)
    }

    /// Cancels an unfinished job and returns its status afterwards (finished jobs are left as they are).
    /// A running job stops at its next sentence. Its parts are deleted.
    pub fn cancel(&self, id: &ExportId) -> AppResult<ExportStatus> {
        let mut registry = self.shared.lock();
        let job = registry.find_mut(id)?;
        let previous = job.record.export.status;
        if !previous.is_unfinished() {
            return Ok(previous);
        }
        job.stop.store(true, Ordering::SeqCst);
        job.record.export.status = ExportStatus::Cancelled;
        job.record.export.estimated_seconds_left = None;
        if previous != ExportStatus::Running {
            self.shared.discard_parts(&mut job.record);
        }
        self.shared.store.save(&job.record)?;
        registry.queue.retain(|queued| queued != id);
        info!(service = NAME, export_id = %id, previous = ?previous, "export cancelled and saved");
        Ok(ExportStatus::Cancelled)
    }
}

impl Drop for ExportJobs {
    /// Stops the runner; a running export is paused, so it continues at its next unfinished part after a restart.
    fn drop(&mut self) {
        let mut registry = self.shared.lock();
        registry.shutdown = true;
        for job in registry.jobs.values() {
            job.stop.store(true, Ordering::SeqCst);
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

    fn find_mut(&mut self, id: &ExportId) -> AppResult<&mut Job> {
        self.jobs.get_mut(id).ok_or_else(|| AppError::new(ErrorName::ExportNotFound).with_detail(format!("export {id}")))
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues a paused job again with a fresh stop flag; queued and running jobs are returned as they are.
    fn requeue(&self, registry: &mut Registry, id: &ExportId) -> AppResult<AudioExport> {
        let job = registry.find_mut(id)?;
        if job.record.export.status != ExportStatus::Paused {
            return Ok(job.record.export.clone());
        }
        job.record.export.status = ExportStatus::Queued;
        job.stop = Arc::default();
        self.store.save(&job.record)?;
        let export = job.record.export.clone();
        registry.queue.push_back(id.clone());
        info!(service = NAME, export_id = %id, parts_done = export.parts_done, parts_total = export.parts_total, "export queued again and saved");
        Ok(export)
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
            return Some(Claimed { record: job.record.clone(), stop: job.stop.clone() });
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

    /// Kept even when the job was paused meanwhile: the part is complete, so a resume can skip it.
    fn record_part(&self, id: &ExportId, part: ExportPart, chapters: Vec<ChapterMark>) {
        let mut registry = self.lock();
        let Some(job) = registry.jobs.get_mut(id) else { return };
        job.record.parts.push(part);
        job.record.chapters.extend(chapters);
        job.record.export.parts_done = u32::try_from(job.record.parts.len()).unwrap_or(u32::MAX);
        self.save(&job.record);
    }

    fn complete(&self, id: &ExportId, outcome: RunOutcome) {
        let mut registry = self.lock();
        let shutdown = registry.shutdown;
        let Some(job) = registry.jobs.get_mut(id) else { return };
        let record = &mut job.record;
        record.export.estimated_seconds_left = None;
        match outcome {
            _ if record.export.status == ExportStatus::Cancelled => {
                self.discard_parts(record);
                info!(service = NAME, export_id = %id, sentences_done = record.export.sentences_done, "export stopped after cancel");
            }
            _ if record.export.status != ExportStatus::Running => {
                info!(service = NAME, export_id = %id, status = ?record.export.status, parts_done = record.export.parts_done, "export stopped after pause");
            }
            _ if shutdown => {
                record.export.status = ExportStatus::Paused;
                info!(service = NAME, export_id = %id, parts_done = record.export.parts_done, "export paused by shutdown");
            }
            RunOutcome::Done(file) => {
                record.export.status = ExportStatus::Done;
                record.export.sentences_done = record.export.sentences_total;
                record.export.estimated_seconds_left = Some(0);
                record.file = Some(file.path);
                self.discard_parts(record);
                info!(service = NAME, export_id = %id, bytes = file.size_bytes, "export done");
            }
            RunOutcome::Stopped => {
                record.export.status = ExportStatus::Paused;
                warn!(service = NAME, export_id = %id, "export stopped without a request; paused");
            }
            RunOutcome::Failed(failure) => {
                record.export.status = ExportStatus::Failed;
                self.discard_parts(record);
                error!(service = NAME, export_id = %id, error = %failure, cause = ?std::error::Error::source(&failure), "export failed");
            }
        }
        let record = record.clone();
        self.save(&record);
        if record.export.status == ExportStatus::Done {
            self.drop_replaced(&mut registry, &record.export);
        }
    }

    /// Parts are only needed to resume. A failed delete is logged; the files are found and removed by the next discard.
    fn discard_parts(&self, record: &mut ExportRecord) {
        let parts = record.parts.len();
        record.parts.clear();
        record.chapters.clear();
        match self.exporter.discard_parts(&record.export.id) {
            Ok(()) => info!(service = NAME, export_id = %record.export.id, parts, "export parts deleted"),
            Err(failure) => warn!(service = NAME, export_id = %record.export.id, error = %failure, "export parts could not be deleted"),
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

/// Forwards the runner's reports to the registry.
struct JobReport<'a> {
    shared: &'a Shared,
    id: &'a ExportId,
}

impl RunReport for JobReport<'_> {
    fn progress(&mut self, progress: Progress) {
        self.shared.update_progress(self.id, progress);
    }

    fn part_finished(&mut self, part: ExportPart, chapters: Vec<ChapterMark>) {
        self.shared.record_part(self.id, part, chapters);
    }
}

fn run_loop(shared: &Shared, runner: &ExportRunner) {
    while let Some(claimed) = shared.claim_next() {
        let id = claimed.record.export.id.clone();
        let outcome = runner.run(&claimed.record, &claimed.stop, &mut JobReport { shared, id: &id });
        shared.complete(&id, outcome);
    }
}
