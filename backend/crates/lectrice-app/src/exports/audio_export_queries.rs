use std::path::PathBuf;
use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, AudioExport, DocumentId, DocumentReader, ErrorName, ExportFormat, ExportId, ExportStatus, SpeechEngine};
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
    type Args = ExportId;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, id: ExportId) -> AppResult<AudioExport> {
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, "pausing audio export");
        let export = self.jobs.pause(&id)?;
        info!(service = Self::NAME, export_id = %id, status = ?export.status, parts_done = export.parts_done, "audio export pause handled");
        Ok(export)
    }
}

/// Queues every paused export again (after a restart, or when the app may work again) and returns the active ones.
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
        info!(service = Self::NAME, request_id = %ctx.request_id, export_id = %id, status = ?record.export.status, has_file = record.file.is_some(), "audio export found");
        let path = match (record.export.status, record.file) {
            (ExportStatus::Done, Some(path)) => path,
            (status, _) => return Err(AppError::new(ErrorName::ExportNotReady).with_detail(format!("export {id} is {status:?}"))),
        };
        let summary = self.reader.find_summary(&record.export.document_id)?;
        info!(service = Self::NAME, doc_id = %record.export.document_id, found = summary.is_some(), "document summary loaded");
        let summary = summary.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        Ok(ExportDownload { path, format: record.export.format, title: summary.title, voice_name: voice_name(self.engine.as_ref(), &record.export.voice_id) })
    }
}
