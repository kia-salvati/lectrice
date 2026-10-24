use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, AudioEviction, AudioRetention, Clock, DocumentId, DocumentReader, DocumentSummary, DocumentWriter, ErrorName};
use tracing::{info, warn};

use crate::exports::ExportJobs;
use crate::speech::ReadAhead;
use crate::{RequestContext, UseCase};

/// Hides a book from the library at once; nothing is deleted until [`PurgeRemovedDocuments`]. Its queued read-ahead
/// stops. Its unfinished exports are the shell's to pause.
pub struct RemoveDocument {
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
    clock: Arc<dyn Clock>,
    /// `None` without a speech engine.
    read_ahead: Option<Arc<ReadAhead>>,
}

impl RemoveDocument {
    pub fn new(reader: Arc<dyn DocumentReader>, writer: Arc<dyn DocumentWriter>, clock: Arc<dyn Clock>, read_ahead: Option<Arc<ReadAhead>>) -> Self {
        Self { reader, writer, clock, read_ahead }
    }
}

impl UseCase for RemoveDocument {
    const NAME: &'static str = "RemoveDocument";
    type Args = DocumentId;
    type Output = ();

    fn run(&self, ctx: &RequestContext, id: DocumentId) -> AppResult<()> {
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %id, "removing document");
        if self.reader.find_summary(&id)?.is_none() {
            return Err(AppError::new(ErrorName::DocumentNotFound));
        }
        self.writer.set_removed(&id, Some(self.clock.now()))?;
        let cancelled = self.read_ahead.as_ref().map_or(0, |read_ahead| read_ahead.stop(&id));
        info!(service = Self::NAME, doc_id = %id, read_ahead_cancelled = cancelled, "document marked removed");
        Ok(())
    }
}

/// Undoes [`RemoveDocument`] (before the purge) and returns the book's summary; a book that isn't removed is
/// returned as it is.
pub struct RestoreDocument {
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
}

impl RestoreDocument {
    pub fn new(reader: Arc<dyn DocumentReader>, writer: Arc<dyn DocumentWriter>) -> Self {
        Self { reader, writer }
    }
}

impl UseCase for RestoreDocument {
    const NAME: &'static str = "RestoreDocument";
    type Args = DocumentId;
    type Output = DocumentSummary;

    fn run(&self, ctx: &RequestContext, id: DocumentId) -> AppResult<DocumentSummary> {
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %id, "restoring document");
        let removed = self.reader.list_removed()?.contains(&id);
        if removed {
            self.writer.set_removed(&id, None)?;
        }
        let summary = self.reader.find_summary(&id)?.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        info!(service = Self::NAME, doc_id = %id, was_removed = removed, "document restored");
        Ok(summary)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PurgeRemovedDocumentsArgs {
    /// Removed books whose finished exports go too; the others' exports stay listed under their title.
    pub delete_exports_for: Vec<DocumentId>,
}

/// What a purge did, as counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeReport {
    pub documents: usize,
    /// Documents that could not be deleted (they stay removed and are tried again by the next purge).
    pub failed: usize,
    pub exports_cancelled: usize,
    pub exports_deleted: usize,
    pub clips: usize,
}

/// Deletes every removed book for good, in an order that never leaves a runnable export or a listed half-book:
/// its unfinished exports (cancelled, parts deleted), its finished exports when asked, its audio, then its folder.
pub struct PurgeRemovedDocuments {
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
    retention: Arc<dyn AudioRetention>,
    jobs: Arc<ExportJobs>,
}

impl PurgeRemovedDocuments {
    pub fn new(reader: Arc<dyn DocumentReader>, writer: Arc<dyn DocumentWriter>, retention: Arc<dyn AudioRetention>, jobs: Arc<ExportJobs>) -> Self {
        Self { reader, writer, retention, jobs }
    }

    fn purge(&self, id: &DocumentId, delete_exports: bool, report: &mut PurgeReport) -> AppResult<()> {
        let exports = self.jobs.purge_document(id, delete_exports)?;
        report.exports_cancelled += exports.cancelled;
        report.exports_deleted += exports.deleted;
        report.clips += self.retention.remove_document(id)?.evicted_clips();
        self.writer.delete(id)
    }
}

impl UseCase for PurgeRemovedDocuments {
    const NAME: &'static str = "PurgeRemovedDocuments";
    type Args = PurgeRemovedDocumentsArgs;
    type Output = PurgeReport;

    fn run(&self, ctx: &RequestContext, args: PurgeRemovedDocumentsArgs) -> AppResult<PurgeReport> {
        let removed = self.reader.list_removed()?;
        info!(service = Self::NAME, request_id = %ctx.request_id, removed = removed.len(), delete_exports_for = args.delete_exports_for.len(), "purging removed documents");
        let mut report = PurgeReport::default();
        for id in removed {
            match self.purge(&id, args.delete_exports_for.contains(&id), &mut report) {
                Ok(()) => {
                    report.documents += 1;
                    info!(service = Self::NAME, doc_id = %id, "removed document deleted");
                }
                Err(error) => {
                    report.failed += 1;
                    warn!(service = Self::NAME, doc_id = %id, error = %error, "removed document could not be deleted; tried again at the next purge");
                }
            }
        }
        info!(
            service = Self::NAME,
            documents = report.documents,
            failed = report.failed,
            exports_cancelled = report.exports_cancelled,
            exports_deleted = report.exports_deleted,
            clips = report.clips,
            "removed documents purged"
        );
        Ok(report)
    }
}

/// Deletes one book's sentence audio, keeping the book ("Clear this book's audio").
pub struct ClearDocumentAudio {
    retention: Arc<dyn AudioRetention>,
}

impl ClearDocumentAudio {
    pub fn new(retention: Arc<dyn AudioRetention>) -> Self {
        Self { retention }
    }
}

impl UseCase for ClearDocumentAudio {
    const NAME: &'static str = "ClearDocumentAudio";
    type Args = DocumentId;
    type Output = AudioEviction;

    fn run(&self, ctx: &RequestContext, id: DocumentId) -> AppResult<AudioEviction> {
        let eviction = self.retention.remove_document(&id)?;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %id, clips = eviction.evicted_clips(), freed_bytes = eviction.freed_bytes, "document audio cleared");
        Ok(eviction)
    }
}
