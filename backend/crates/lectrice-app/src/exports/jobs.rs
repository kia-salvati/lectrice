//! The export job registry: every job's state in memory and on disk, and (with a speech engine) one runner thread
//! that exports queued jobs one at a time (the speech worker makes one sentence at a time anyway).
//!
//! States: `queued → running → done | failed | cancelled`, `queued | running → paused → queued` (pause, resume) and
//! `queued | paused → cancelled`. A paused job keeps its finished parts; resuming continues at the first unfinished
//! one. Jobs found queued or running when the registry starts were interrupted by a stop, so they become paused
//! (`System`). Running out of space at any step pauses (`StorageFull`) instead of failing, so no part is lost.
//! Only jobs for the loaded engine run; the others wait in the queue.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioExport, AudioExporter, Clock, DocumentId, DocumentReader, ErrorName, ExportFormat, ExportId,
    ExportLocation, ExportRange, ExportRecord, ExportRecordStore, ExportSpeed, ExportStage, ExportStatus, PauseReason, SpeechEngine, TimeStretch, VoiceId,
};
use tracing::{error, info, warn};

use super::run::{ExportRunner, ExportRunnerDeps, FinishedPart, Progress, RunOutcome, RunReport};
use crate::speech::SpeechWorker;

const NAME: &str = "ExportJobs";
const THREAD_NAME: &str = "export-runner";
/// Listening minutes per export part (Android spec §8.3): at most this much work is lost when an export stops.
const PART_MINUTES: u32 = 10;

/// What exports are made with. Without it, jobs are still kept, listed and deleted, and queued ones wait.
pub struct ExportSpeech {
    pub worker: Arc<SpeechWorker>,
    pub engine: Arc<dyn SpeechEngine>,
}

pub struct ExportDeps {
    pub speech: Option<ExportSpeech>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub decoder: Arc<dyn AudioDecoder>,
    pub exporter: Arc<dyn AudioExporter>,
    pub store: Arc<dyn ExportRecordStore>,
    pub clock: Arc<dyn Clock>,
    pub time_stretch: Option<Arc<dyn TimeStretch>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExportSettings {
    /// Exports only the first sentences when set (a short export for testing).
    pub max_sentences: Option<u32>,
    /// Listening minutes per part; 0 puts every sentence in its own part.
    pub part_minutes: u32,
    /// For a shell with a single engine: whole-book 1x exports keep 1.0's `<voiceId>.<ext>` file name.
    pub short_file_names: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { max_sentences: None, part_minutes: PART_MINUTES, short_file_names: false }
    }
}

/// A job to create. The totals come from the export plan.
#[derive(Debug, Clone)]
pub struct NewExport {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub engine_id: String,
    pub speed: ExportSpeed,
    pub range: Option<ExportRange>,
    pub range_label: Option<String>,
    pub title: String,
    pub read_skipped: bool,
    pub sentences_total: u32,
    pub parts_total: u32,
}

/// What makes two exports write the same file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetKey<'a> {
    document_id: &'a DocumentId,
    voice_id: &'a VoiceId,
    format: ExportFormat,
    engine_id: &'a str,
    speed: ExportSpeed,
    range: Option<ExportRange>,
}

impl<'a> TargetKey<'a> {
    fn of(export: &'a AudioExport) -> Self {
        Self {
            document_id: &export.document_id,
            voice_id: &export.voice_id,
            format: export.format,
            engine_id: &export.engine_id,
            speed: export.speed,
            range: export.range,
        }
    }

    fn of_new(new: &'a NewExport) -> Self {
        Self { document_id: &new.document_id, voice_id: &new.voice_id, format: new.format, engine_id: &new.engine_id, speed: new.speed, range: new.range }
    }
}

/// What purging a document did to its exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgedExports {
    pub cancelled: usize,
    pub deleted: usize,
    pub kept: usize,
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
    /// The loaded engine's id; `None` without one.
    engine_id: Option<String>,
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
    /// A record save ran out of space; the stopped run becomes a `StorageFull` pause.
    out_of_space: bool,
    /// Purged while running: dropped (with its parts) once the run stops.
    forget_when_stopped: bool,
}

impl Job {
    fn new(record: ExportRecord) -> Self {
        Self { record, stop: Arc::default(), out_of_space: false, forget_when_stopped: false }
    }
}

struct Claimed {
    record: ExportRecord,
    stop: Arc<AtomicBool>,
}

impl ExportJobs {
    /// Loads the saved jobs (marking interrupted ones paused) and, with a speech engine, starts the runner thread.
    /// Nothing runs until a job is queued: call [`ExportJobs::resume_all`] to continue the paused ones.
    pub fn start(deps: ExportDeps, settings: ExportSettings) -> AppResult<Self> {
        let ExportDeps { speech, reader, cache, decoder, exporter, store, clock, time_stretch } = deps;
        let engine_id = speech.as_ref().map(|speech| speech.engine.id().to_string());
        let mut registry = Registry::default();
        let mut interrupted = 0;
        for mut record in store.load_all()? {
            let was_active = record.export.status.is_active();
            if upgrade(&mut record, engine_id.as_deref()) {
                store.save(&record)?;
            }
            interrupted += usize::from(was_active);
            registry.jobs.insert(record.export.id.clone(), Job::new(record));
        }
        info!(service = NAME, jobs = registry.jobs.len(), interrupted, engine_id = engine_id.as_deref(), "export jobs loaded");

        let shared = Arc::new(Shared { registry: Mutex::new(registry), wake: Condvar::new(), store, exporter: exporter.clone(), clock, engine_id });
        let Some(ExportSpeech { worker, engine }) = speech else {
            return Ok(Self { shared, thread: None });
        };
        let runner = ExportRunner::new(ExportRunnerDeps { worker, engine, reader, cache, decoder, exporter, time_stretch }, settings);
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || run_loop(&thread_shared, &runner))
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the export runner thread").with_source(error))?;
        Ok(Self { shared, thread: Some(thread) })
    }

    /// Queues a new job, or returns the unfinished job for the same file (document, voice, format, engine, speed and
    /// range); a paused one is queued again. The flag is true when the job is new.
    pub fn enqueue(&self, new: NewExport) -> AppResult<(AudioExport, bool)> {
        let mut registry = self.shared.lock();
        let wanted = TargetKey::of_new(&new);
        let existing = registry.jobs.values().find(|job| job.record.export.status.is_unfinished() && TargetKey::of(&job.record.export) == wanted);
        if let Some(existing) = existing.map(|job| job.record.export.id.clone()) {
            let export = self.shared.requeue(&mut registry, &existing)?;
            drop(registry);
            self.shared.wake.notify_all();
            return Ok((self.shared.view(export), false));
        }

        let created_at = self.shared.clock.now();
        let id = registry.new_id(created_at.timestamp());
        let export = AudioExport {
            id: id.clone(),
            document_id: new.document_id,
            voice_id: new.voice_id,
            format: new.format,
            speed: new.speed,
            engine_id: new.engine_id,
            range: new.range,
            range_label: new.range_label,
            title: new.title,
            status: ExportStatus::Queued,
            paused_by: None,
            stage: ExportStage::Parts,
            error: None,
            sentences_done: 0,
            sentences_total: new.sentences_total,
            parts_done: 0,
            parts_total: new.parts_total,
            estimated_seconds_left: None,
            file_bytes: None,
            location: None,
            waiting_for_engine: false,
        };
        let record = ExportRecord {
            export: export.clone(),
            read_skipped: new.read_skipped,
            created_at,
            file: None,
            parts: Vec::new(),
            chapters: Vec::new(),
            source_samples: 0,
        };
        self.shared.store.save(&record)?;
        info!(service = NAME, export_id = %id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), engine_id = %export.engine_id, speed_percent = export.speed.percent(), ranged = export.range.is_some(), sentences_total = export.sentences_total, parts_total = export.parts_total, "export job saved and queued");
        registry.jobs.insert(id.clone(), Job::new(record));
        registry.queue.push_back(id);
        drop(registry);
        self.shared.wake.notify_all();
        Ok((self.shared.view(export), true))
    }

    pub fn get(&self, id: &ExportId) -> Option<AudioExport> {
        self.shared.lock().jobs.get(id).map(|job| self.shared.view(job.record.export.clone()))
    }

    pub fn record(&self, id: &ExportId) -> Option<ExportRecord> {
        self.shared.lock().jobs.get(id).map(|job| job.record.clone())
    }

    /// One document's exports, newest first.
    pub fn list(&self, document_id: &DocumentId) -> Vec<AudioExport> {
        self.list_where(|export| export.document_id == *document_id)
    }

    /// Every export, newest first, also those of documents that were removed.
    pub fn list_all(&self) -> Vec<AudioExport> {
        self.list_where(|_| true)
    }

    /// Every record, for the storage numbers.
    pub fn records(&self) -> Vec<ExportRecord> {
        self.shared.lock().jobs.values().map(|job| job.record.clone()).collect()
    }

    fn list_where(&self, keep: impl Fn(&AudioExport) -> bool) -> Vec<AudioExport> {
        let registry = self.shared.lock();
        let mut exports: Vec<AudioExport> =
            registry.jobs.values().map(|job| &job.record.export).filter(|export| keep(export)).map(|export| self.shared.view(export.clone())).collect();
        exports.sort_by(|a, b| b.id.as_str().cmp(a.id.as_str()));
        exports
    }

    /// Pauses a queued or running job and returns it. A running job stops at its next sentence; its finished parts are
    /// kept and the part it was writing is dropped. A paused job only changes when the user pauses it (their pause
    /// isn't resumed by the system). Finished jobs are returned as they are.
    pub fn pause(&self, id: &ExportId, reason: PauseReason) -> AppResult<AudioExport> {
        let mut registry = self.shared.lock();
        let job = registry.find_mut(id)?;
        let previous = job.record.export.status;
        let user_takes_over = previous == ExportStatus::Paused && reason == PauseReason::User && job.record.export.paused_by != Some(reason);
        if !previous.is_active() && !user_takes_over {
            return Ok(self.shared.view(job.record.export.clone()));
        }
        pause_job(job, reason);
        self.shared.store.save(&job.record)?;
        let export = job.record.export.clone();
        registry.queue.retain(|queued| queued != id);
        info!(service = NAME, export_id = %id, previous = ?previous, reason = ?reason, parts_done = export.parts_done, "export paused and saved");
        Ok(self.shared.view(export))
    }

    /// Pauses every queued and running job (the system stops the work, or the engine is about to change).
    pub fn pause_all(&self, reason: PauseReason) -> AppResult<Vec<AudioExport>> {
        let mut registry = self.shared.lock();
        let mut paused = Vec::new();
        for job in registry.jobs.values_mut().filter(|job| job.record.export.status.is_active()) {
            pause_job(job, reason);
            self.shared.store.save(&job.record)?;
            paused.push(self.shared.view(job.record.export.clone()));
        }
        registry.queue.clear();
        info!(service = NAME, reason = ?reason, paused = paused.len(), "exports paused and saved");
        Ok(paused)
    }

    /// Queues again, oldest first, every job paused by the system or an engine switch (not the user's or a full disk's
    /// pauses), and returns the jobs that are now queued or running.
    pub fn resume_all(&self) -> AppResult<Vec<AudioExport>> {
        let mut registry = self.shared.lock();
        let mut paused: Vec<ExportId> = registry
            .jobs
            .values()
            .filter(|job| job.record.export.status == ExportStatus::Paused && job.record.export.paused_by.is_none_or(PauseReason::resumes_by_itself))
            .map(|job| job.record.export.id.clone())
            .collect();
        paused.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for id in &paused {
            self.shared.requeue(&mut registry, id)?;
        }
        let mut active: Vec<AudioExport> = registry
            .jobs
            .values()
            .map(|job| &job.record.export)
            .filter(|export| export.status.is_active())
            .map(|export| self.shared.view(export.clone()))
            .collect();
        active.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        drop(registry);
        self.shared.wake.notify_all();
        info!(service = NAME, resumed = paused.len(), active = active.len(), "paused exports queued again");
        Ok(active)
    }

    /// Queues one paused job again, whatever paused it (the user's "Continue"). Other jobs are returned as they are.
    pub fn resume(&self, id: &ExportId) -> AppResult<AudioExport> {
        let mut registry = self.shared.lock();
        let export = self.shared.requeue(&mut registry, id)?;
        drop(registry);
        self.shared.wake.notify_all();
        Ok(self.shared.view(export))
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
        self.shared.cancel_job(job);
        self.shared.store.save(&job.record)?;
        registry.queue.retain(|queued| queued != id);
        info!(service = NAME, export_id = %id, previous = ?previous, "export cancelled and saved");
        Ok(ExportStatus::Cancelled)
    }

    /// Deletes a finished, failed or cancelled job's record and, when it is in app storage, its file.
    pub fn delete(&self, id: &ExportId) -> AppResult<()> {
        let mut registry = self.shared.lock();
        let job = registry.find_mut(id)?;
        if job.record.export.status.is_unfinished() {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail(format!("export {id} is unfinished; cancel it first")));
        }
        let file = app_storage_path(&job.record);
        if let Some(path) = &file {
            self.shared.exporter.delete_file(path)?;
        }
        self.shared.store.delete(id)?;
        registry.jobs.remove(id);
        info!(service = NAME, export_id = %id, file_deleted = file.is_some(), "export deleted");
        Ok(())
    }

    /// Records that the shell moved a done export's file to shared storage; the core keeps only the handle.
    pub fn mark_saved(&self, id: &ExportId, uri: String) -> AppResult<AudioExport> {
        let mut registry = self.shared.lock();
        let job = registry.find_mut(id)?;
        if job.record.export.status != ExportStatus::Done {
            return Err(AppError::new(ErrorName::ExportNotReady).with_detail(format!("export {id} is {:?}", job.record.export.status)));
        }
        job.record.export.location = Some(ExportLocation::Shared { uri });
        job.record.file = None;
        self.shared.store.save(&job.record)?;
        info!(service = NAME, export_id = %id, "export marked as moved to shared storage");
        Ok(self.shared.view(job.record.export.clone()))
    }

    /// For a document being deleted: cancels and drops its unfinished jobs (parts deleted), and deletes its finished
    /// ones (record and file) only when `delete_finished`; else they stay listed under their title.
    pub fn purge_document(&self, document_id: &DocumentId, delete_finished: bool) -> AppResult<PurgedExports> {
        let mut registry = self.shared.lock();
        let ids: Vec<ExportId> =
            registry.jobs.values().filter(|job| job.record.export.document_id == *document_id).map(|job| job.record.export.id.clone()).collect();
        let mut purged = PurgedExports::default();
        for id in ids {
            let job = registry.find_mut(&id)?;
            if job.record.export.status.is_unfinished() {
                let running = job.record.export.status == ExportStatus::Running;
                self.shared.cancel_job(job);
                purged.cancelled += 1;
                if running {
                    // Saved, so a crash before the run stops can't bring it back as resumable.
                    job.forget_when_stopped = true;
                    self.shared.store.save(&job.record)?;
                    continue;
                }
            } else if !delete_finished {
                purged.kept += 1;
                continue;
            } else {
                if let Some(path) = app_storage_path(&job.record) {
                    self.shared.exporter.delete_file(&path)?;
                }
                purged.deleted += 1;
            }
            self.shared.store.delete(&id)?;
            registry.jobs.remove(&id);
            registry.queue.retain(|queued| *queued != id);
        }
        info!(service = NAME, doc_id = %document_id, cancelled = purged.cancelled, deleted = purged.deleted, kept = purged.kept, "document exports purged");
        Ok(purged)
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

/// Brings a loaded record up to date; true when it changed. Records saved before v1.1 have no engine (the one engine
/// that made them is the loaded one) and no pause reason; a done 1.x record has its file in app storage.
fn upgrade(record: &mut ExportRecord, engine_id: Option<&str>) -> bool {
    let mut changed = false;
    let export = &mut record.export;
    if export.engine_id.is_empty()
        && let Some(engine_id) = engine_id
    {
        export.engine_id = engine_id.to_string();
        changed = true;
    }
    if export.status.is_active() {
        export.status = ExportStatus::Paused;
        export.paused_by = Some(PauseReason::System);
        export.estimated_seconds_left = None;
        changed = true;
    }
    if export.status == ExportStatus::Paused && export.paused_by.is_none() {
        export.paused_by = Some(PauseReason::System);
        changed = true;
    }
    if export.status == ExportStatus::Done
        && export.location.is_none()
        && let Some(path) = &record.file
    {
        export.location = Some(ExportLocation::AppStorage { path: path.clone() });
        changed = true;
    }
    changed
}

fn pause_job(job: &mut Job, reason: PauseReason) {
    job.stop.store(true, Ordering::SeqCst);
    job.record.export.status = ExportStatus::Paused;
    job.record.export.paused_by = Some(reason);
    job.record.export.estimated_seconds_left = None;
}

fn app_storage_path(record: &ExportRecord) -> Option<std::path::PathBuf> {
    match &record.export.location {
        Some(ExportLocation::AppStorage { path }) => Some(path.clone()),
        Some(ExportLocation::Shared { .. }) => None,
        None => record.file.clone(),
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

    /// The export as callers see it: whether it waits for another engine is worked out here.
    fn view(&self, mut export: AudioExport) -> AudioExport {
        export.waiting_for_engine = export.status == ExportStatus::Queued && self.engine_id.as_deref() != Some(export.engine_id.as_str());
        export
    }

    /// Queues a paused job again with a fresh stop flag; queued and running jobs are returned as they are.
    fn requeue(&self, registry: &mut Registry, id: &ExportId) -> AppResult<AudioExport> {
        let job = registry.find_mut(id)?;
        if job.record.export.status != ExportStatus::Paused {
            return Ok(job.record.export.clone());
        }
        job.record.export.status = ExportStatus::Queued;
        job.record.export.paused_by = None;
        job.record.export.error = None;
        job.stop = Arc::default();
        job.out_of_space = false;
        self.store.save(&job.record)?;
        let export = job.record.export.clone();
        registry.queue.push_back(id.clone());
        info!(service = NAME, export_id = %id, parts_done = export.parts_done, parts_total = export.parts_total, stage = ?export.stage, "export queued again and saved");
        Ok(export)
    }

    /// Marks an unfinished job cancelled; a running one deletes its parts when its run stops.
    fn cancel_job(&self, job: &mut Job) {
        let previous = job.record.export.status;
        job.stop.store(true, Ordering::SeqCst);
        job.record.export.status = ExportStatus::Cancelled;
        job.record.export.paused_by = None;
        job.record.export.estimated_seconds_left = None;
        if previous != ExportStatus::Running {
            self.discard_parts(&mut job.record);
        }
    }

    /// Waits for the oldest queued job of the loaded engine and marks it running. `None` once the registry shuts down.
    fn claim_next(&self) -> Option<Claimed> {
        let mut registry = self.lock();
        loop {
            if registry.shutdown {
                return None;
            }
            let Registry { jobs, queue, .. } = &mut *registry;
            queue.retain(|id| jobs.get(id).is_some_and(|job| job.record.export.status == ExportStatus::Queued));
            let runnable = queue.iter().position(|id| jobs.get(id).is_some_and(|job| self.engine_id.as_deref() == Some(job.record.export.engine_id.as_str())));
            let Some(position) = runnable else {
                registry = self.wake.wait(registry).unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let Some(id) = queue.remove(position) else { continue };
            let Some(job) = jobs.get_mut(&id) else { continue };
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
            self.save_or_stop(job);
        }
    }

    /// Kept even when the job was paused meanwhile: the part is complete, so a resume can skip it.
    fn record_part(&self, id: &ExportId, finished: FinishedPart) {
        let mut registry = self.lock();
        let Some(job) = registry.jobs.get_mut(id) else { return };
        job.record.parts.push(finished.part);
        job.record.chapters.extend(finished.chapters);
        job.record.source_samples = finished.source_samples;
        job.record.export.parts_done = u32::try_from(job.record.parts.len()).unwrap_or(u32::MAX);
        self.save_or_stop(job);
    }

    fn mark_assembling(&self, id: &ExportId) {
        let mut registry = self.lock();
        let Some(job) = registry.jobs.get_mut(id) else { return };
        job.record.export.stage = ExportStage::Assemble;
        self.save_or_stop(job);
    }

    /// A full disk while saving the record stops the run, which then pauses for space.
    fn save_or_stop(&self, job: &mut Job) {
        if self.save(&job.record) == Some(ErrorName::StorageFull) {
            job.out_of_space = true;
            job.stop.store(true, Ordering::SeqCst);
        }
    }

    fn complete(&self, id: &ExportId, outcome: RunOutcome) {
        let mut registry = self.lock();
        let shutdown = registry.shutdown;
        // With 1.0 file names an older finished export can own the same path.
        let file_owned_elsewhere = match &outcome {
            RunOutcome::Done(file) => {
                registry.jobs.values().any(|job| job.record.export.id != *id && app_storage_path(&job.record).as_ref() == Some(&file.path))
            }
            _ => false,
        };
        let Some(job) = registry.jobs.get_mut(id) else { return };
        let out_of_space = job.out_of_space;
        let forget = job.forget_when_stopped;
        let record = &mut job.record;
        record.export.estimated_seconds_left = None;
        match outcome {
            _ if record.export.status == ExportStatus::Cancelled => {
                if let RunOutcome::Done(file) = &outcome
                    && !file_owned_elsewhere
                {
                    self.delete_unwanted_file(id, &file.path);
                }
                self.discard_parts(record);
                info!(service = NAME, export_id = %id, sentences_done = record.export.sentences_done, "export stopped after cancel");
            }
            // The file is complete, so a pause or shutdown that came during the last step changes nothing.
            RunOutcome::Done(file) => {
                record.export.status = ExportStatus::Done;
                record.export.paused_by = None;
                record.export.sentences_done = record.export.sentences_total;
                record.export.estimated_seconds_left = Some(0);
                record.export.file_bytes = Some(file.size_bytes);
                record.export.location = Some(ExportLocation::AppStorage { path: file.path.clone() });
                record.file = Some(file.path);
                self.discard_parts(record);
                info!(service = NAME, export_id = %id, bytes = file.size_bytes, "export done");
            }
            _ if record.export.status != ExportStatus::Running => {
                info!(service = NAME, export_id = %id, status = ?record.export.status, parts_done = record.export.parts_done, "export stopped after pause");
            }
            RunOutcome::Failed(failure) if failure.primary() == ErrorName::StorageFull => {
                pause_for_space(record);
                warn!(service = NAME, export_id = %id, stage = ?record.export.stage, parts_done = record.export.parts_done, error = %failure, "export paused, out of space");
            }
            RunOutcome::Stopped if out_of_space => {
                pause_for_space(record);
                warn!(service = NAME, export_id = %id, parts_done = record.export.parts_done, "export paused, out of space saving its record");
            }
            _ if shutdown => {
                record.export.status = ExportStatus::Paused;
                record.export.paused_by = Some(PauseReason::System);
                info!(service = NAME, export_id = %id, parts_done = record.export.parts_done, "export paused by shutdown");
            }
            RunOutcome::Stopped => {
                record.export.status = ExportStatus::Paused;
                record.export.paused_by = Some(PauseReason::System);
                warn!(service = NAME, export_id = %id, "export stopped without a request; paused");
            }
            RunOutcome::Failed(failure) => {
                record.export.status = ExportStatus::Failed;
                record.export.error = Some(failure.primary());
                self.discard_parts(record);
                error!(service = NAME, export_id = %id, error = %failure, cause = ?std::error::Error::source(&failure), "export failed");
            }
        }
        if forget {
            registry.jobs.remove(id);
            match self.store.delete(id) {
                Ok(()) => info!(service = NAME, export_id = %id, "purged export dropped after its run stopped"),
                Err(failure) => warn!(service = NAME, export_id = %id, error = %failure, "purged export record could not be deleted"),
            }
            return;
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
        record.source_samples = 0;
        match self.exporter.discard_parts(&record.export.id) {
            Ok(()) => info!(service = NAME, export_id = %record.export.id, parts, "export parts deleted"),
            Err(failure) => warn!(service = NAME, export_id = %record.export.id, error = %failure, "export parts could not be deleted"),
        }
    }

    /// A finished file the job no longer owns (cancelled or purged during its last step).
    fn delete_unwanted_file(&self, id: &ExportId, path: &std::path::Path) {
        match self.exporter.delete_file(path) {
            Ok(()) => info!(service = NAME, export_id = %id, "export file of a cancelled job deleted"),
            Err(failure) => warn!(service = NAME, export_id = %id, error = %failure, "export file of a cancelled job could not be deleted"),
        }
    }

    /// An export of the same file replaces it, so older finished jobs for it go. One moved to shared storage is
    /// another file and stays.
    fn drop_replaced(&self, registry: &mut Registry, newest: &AudioExport) {
        let target = TargetKey::of(newest);
        let replaced: Vec<ExportId> = registry
            .jobs
            .values()
            .map(|job| &job.record.export)
            .filter(|export| export.id != newest.id && export.status == ExportStatus::Done && TargetKey::of(export) == target)
            .filter(|export| !matches!(export.location, Some(ExportLocation::Shared { .. })))
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

    /// Saves from the runner thread can't fail a request; a failed save is logged (and retried at the next change).
    /// Returns the failure's primary name.
    fn save(&self, record: &ExportRecord) -> Option<ErrorName> {
        let failure = self.store.save(record).err()?;
        warn!(service = NAME, export_id = %record.export.id, status = ?record.export.status, error = %failure, "export job could not be saved");
        Some(failure.primary())
    }
}

/// Keeps every finished part, so nothing is made again once there is room.
fn pause_for_space(record: &mut ExportRecord) {
    record.export.status = ExportStatus::Paused;
    record.export.paused_by = Some(PauseReason::StorageFull);
    record.export.error = Some(ErrorName::StorageFull);
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

    fn part_finished(&mut self, finished: FinishedPart) {
        self.shared.record_part(self.id, finished);
    }

    fn assembling(&mut self) {
        self.shared.mark_assembling(self.id);
    }
}

fn run_loop(shared: &Shared, runner: &ExportRunner) {
    while let Some(claimed) = shared.claim_next() {
        let id = claimed.record.export.id.clone();
        let outcome = runner.run(&claimed.record, &claimed.stop, &mut JobReport { shared, id: &id });
        shared.complete(&id, outcome);
    }
}
