use std::path::PathBuf;
use std::sync::Arc;

use lectrice_domain::{
    AppError, AppResult, AudioExport, DocumentId, DocumentReader, ErrorName, ExportFormat, ExportId, ExportLocation, ExportStatus, PauseReason, SpeechEngine,
};
use tracing::info;

use super::jobs::ExportJobs;
use super::run::voice_name;
use crate::{RequestContext, UseCase};

fn not_found(id: &ExportId) -> AppError {
    AppError::new(ErrorName::ExportNotFound).with_detail(format!("export {id}"))
}

/// Status, progress and time left of one export.
pub struct GetAudioExport {
    jobs: Arc<ExportJobs>,
}

impl GetAudioExport {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for GetAudioExport {
    const NAME: &'static str = "GetAudioExport";
    type Args = ExportId;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<AudioExport> {
        let export = self.jobs.get(&id).ok_or_else(|| not_found(&id))?;
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, status = ?export.status, sentences_done = export.sentences_done, sentences_total = export.sentences_total, "audio export found");
        Ok(export)
    }
}

/// A document's exports, newest first, so the reader can show a running or finished export after it's reopened.
pub struct ListAudioExports {
    jobs: Arc<ExportJobs>,
    reader: Arc<dyn DocumentReader>,
}

impl ListAudioExports {
    pub fn new(jobs: Arc<ExportJobs>, reader: Arc<dyn DocumentReader>) -> Self {
        Self { jobs, reader }
    }
}

impl UseCase for ListAudioExports {
    const NAME: &'static str = "ListAudioExports";
    type Args = DocumentId;
    type Output = Vec<AudioExport>;

    fn run(&self, ctx: &RequestContext, document_id: DocumentId) -> AppResult<Vec<AudioExport>> {
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, "listing audio exports");
        let summary = self.reader.find_summary(&document_id)?;
        info!(service = Self::NAME, doc_id = %document_id, found = summary.is_some(), "document summary loaded");
        if summary.is_none() {
            return Err(AppError::new(ErrorName::DocumentNotFound));
        }
        let exports = self.jobs.list(&document_id);
        info!(service = Self::NAME, doc_id = %document_id, exports = exports.len(), "audio exports listed");
        Ok(exports)
    }
}

/// Every export, newest first, also of books that were removed (each keeps the title it started with).
pub struct ListAllAudioExports {
    jobs: Arc<ExportJobs>,
}

impl ListAllAudioExports {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for ListAllAudioExports {
    const NAME: &'static str = "ListAllAudioExports";
    type Args = ();
    type Output = Vec<AudioExport>;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<Vec<AudioExport>> {
        let exports = self.jobs.list_all();
        info!(service = Self::NAME, request_id = %ctx.request_id, exports = exports.len(), "all audio exports listed");
        Ok(exports)
    }
}

#[derive(Debug, Clone)]
pub struct PauseAudioExportArgs {
    pub id: ExportId,
    pub reason: PauseReason,
}

/// Pauses a queued or running export, keeping its finished parts. Other exports are left as they are.
pub struct PauseAudioExport {
    jobs: Arc<ExportJobs>,
}

impl PauseAudioExport {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for PauseAudioExport {
    const NAME: &'static str = "PauseAudioExport";
    type Args = PauseAudioExportArgs;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, args: PauseAudioExportArgs) -> AppResult<AudioExport> {
        let PauseAudioExportArgs { id, reason } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, reason = ?reason, "pausing audio export");
        let export = self.jobs.pause(&id, reason)?;
        info!(service = Self::NAME, export_id = %id, status = ?export.status, parts_done = export.parts_done, "audio export pause handled");
        Ok(export)
    }
}

/// Pauses every queued and running export (the system stops the work, or the model bundle is about to change).
pub struct PauseAudioExports {
    jobs: Arc<ExportJobs>,
}

impl PauseAudioExports {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for PauseAudioExports {
    const NAME: &'static str = "PauseAudioExports";
    type Args = PauseReason;
    type Output = Vec<AudioExport>;

    fn run(&self, ctx: &RequestContext, reason: PauseReason) -> AppResult<Vec<AudioExport>> {
        info!(service = Self::NAME, request_id = %ctx.request_id, reason = ?reason, "pausing audio exports");
        let paused = self.jobs.pause_all(reason)?;
        info!(service = Self::NAME, paused = paused.len(), "audio exports paused");
        Ok(paused)
    }
}

/// Queues again every export the system or an engine switch paused (after a restart, or when the app may work again)
/// and returns the active ones. Exports the user or a full disk paused wait for [`ResumeAudioExport`].
pub struct ResumeAudioExports {
    jobs: Arc<ExportJobs>,
}

impl ResumeAudioExports {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for ResumeAudioExports {
    const NAME: &'static str = "ResumeAudioExports";
    type Args = ();
    type Output = Vec<AudioExport>;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<Vec<AudioExport>> {
        info!(service = Self::NAME, request_id = %ctx.request_id, "resuming audio exports");
        let active = self.jobs.resume_all()?;
        info!(service = Self::NAME, active = active.len(), "audio exports resumed");
        Ok(active)
    }
}

/// The user's "Continue": queues one paused export again, whatever paused it.
pub struct ResumeAudioExport {
    jobs: Arc<ExportJobs>,
}

impl ResumeAudioExport {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for ResumeAudioExport {
    const NAME: &'static str = "ResumeAudioExport";
    type Args = ExportId;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<AudioExport> {
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, "resuming audio export");
        let export = self.jobs.resume(&id)?;
        info!(service = Self::NAME, export_id = %id, status = ?export.status, "audio export resume handled");
        Ok(export)
    }
}

/// Stops an unfinished export and deletes its parts. Finished exports are left as they are.
pub struct CancelAudioExport {
    jobs: Arc<ExportJobs>,
}

impl CancelAudioExport {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for CancelAudioExport {
    const NAME: &'static str = "CancelAudioExport";
    type Args = ExportId;
    type Output = ExportStatus;

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<ExportStatus> {
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, "cancelling audio export");
        let status = self.jobs.cancel(&id)?;
        info!(service = Self::NAME, export_id = %id, status = ?status, "audio export cancel handled");
        Ok(status)
    }
}

/// Deletes a done, failed or cancelled export: its file when it is in app storage (a shared one is the shell's to
/// delete first), and its record. An unfinished export is refused with `RequestInvalid`.
pub struct DeleteAudioExport {
    jobs: Arc<ExportJobs>,
}

impl DeleteAudioExport {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for DeleteAudioExport {
    const NAME: &'static str = "DeleteAudioExport";
    type Args = ExportId;
    type Output = ();

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<()> {
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, "deleting audio export");
        self.jobs.delete(&id)
    }
}

#[derive(Debug, Clone)]
pub struct MarkExportSavedArgs {
    pub id: ExportId,
    /// The shell's handle for the new place (e.g. a content URI); the core never opens it.
    pub uri: String,
}

/// After the shell moved a done export's file to shared storage (and deleted the private copy).
pub struct MarkExportSaved {
    jobs: Arc<ExportJobs>,
}

impl MarkExportSaved {
    pub fn new(jobs: Arc<ExportJobs>) -> Self {
        Self { jobs }
    }
}

impl UseCase for MarkExportSaved {
    const NAME: &'static str = "MarkExportSaved";
    type Args = MarkExportSavedArgs;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, args: MarkExportSavedArgs) -> AppResult<AudioExport> {
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %args.id, "marking audio export saved");
        if args.uri.trim().is_empty() {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail("the saved location must not be empty"));
        }
        self.jobs.mark_saved(&args.id, args.uri)
    }
}

/// A finished export's file, with what the download name is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDownload {
    pub path: PathBuf,
    pub format: ExportFormat,
    pub title: String,
    pub voice_name: String,
}

pub struct GetAudioExportFile {
    jobs: Arc<ExportJobs>,
    reader: Arc<dyn DocumentReader>,
    engine: Arc<dyn SpeechEngine>,
}

impl GetAudioExportFile {
    pub fn new(jobs: Arc<ExportJobs>, reader: Arc<dyn DocumentReader>, engine: Arc<dyn SpeechEngine>) -> Self {
        Self { jobs, reader, engine }
    }
}

impl UseCase for GetAudioExportFile {
    const NAME: &'static str = "GetAudioExportFile";
    type Args = ExportId;
    type Output = ExportDownload;

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<ExportDownload> {
        let record = self.jobs.record(&id).ok_or_else(|| not_found(&id))?;
        let export = &record.export;
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, status = ?export.status, has_location = export.location.is_some(), "audio export found");
        let path = match (export.status, &export.location) {
            (ExportStatus::Done, Some(ExportLocation::AppStorage { path })) => path.clone(),
            (ExportStatus::Done, Some(ExportLocation::Shared { .. })) => {
                return Err(AppError::new(ErrorName::ExportNotFound).with_detail(format!("export {id} was moved to shared storage")));
            }
            (status, _) => return Err(AppError::new(ErrorName::ExportNotReady).with_detail(format!("export {id} is {status:?}"))),
        };
        let summary = self.reader.find_summary(&export.document_id)?;
        info!(service = Self::NAME, doc_id = %export.document_id, found = summary.is_some(), "document summary loaded");
        let title = match summary {
            Some(summary) => summary.title,
            None if !export.title.is_empty() => export.title.clone(),
            None => return Err(AppError::new(ErrorName::DocumentNotFound)),
        };
        Ok(ExportDownload { path, format: export.format, title, voice_name: voice_name(self.engine.as_ref(), &export.voice_id) })
    }
}
