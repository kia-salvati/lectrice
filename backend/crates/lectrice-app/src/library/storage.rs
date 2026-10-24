use std::collections::HashMap;
use std::sync::Arc;

use lectrice_domain::{AppResult, AudioEviction, AudioExporter, AudioRetention, DocumentId, DocumentReader, ExportLocation, ExportStatus};
use tracing::{info, warn};

use crate::exports::ExportJobs;
use crate::{RequestContext, UseCase};

/// What the app keeps on disk, for a storage screen. Removed books are left out of the per-book list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageUsage {
    pub library_documents: u32,
    pub library_bytes: u64,
    pub cache_bytes: u64,
    pub cache_cap_bytes: u64,
    /// Finished exports whose file is still in app storage.
    pub export_files: u32,
    pub export_bytes: u64,
    /// Parts of unfinished exports.
    pub export_parts_bytes: u64,
    /// Largest total first.
    pub documents: Vec<DocumentUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentUsage {
    pub document_id: DocumentId,
    /// The PDF copy, the cleaned text and the bookkeeping.
    pub library_bytes: u64,
    pub cache_bytes: u64,
}

pub struct GetStorageUsage {
    reader: Arc<dyn DocumentReader>,
    retention: Arc<dyn AudioRetention>,
    jobs: Arc<ExportJobs>,
    exporter: Arc<dyn AudioExporter>,
}

impl GetStorageUsage {
    pub fn new(reader: Arc<dyn DocumentReader>, retention: Arc<dyn AudioRetention>, jobs: Arc<ExportJobs>, exporter: Arc<dyn AudioExporter>) -> Self {
        Self { reader, retention, jobs, exporter }
    }
}

impl UseCase for GetStorageUsage {
    const NAME: &'static str = "GetStorageUsage";
    type Args = ();
    type Output = StorageUsage;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<StorageUsage> {
        // The system may have cleared clips; numbers must not count them.
        if let Err(error) = self.retention.refresh() {
            warn!(service = Self::NAME, error = %error, "audio cache could not be re-read");
        }
        let cache = self.retention.usage()?;
        let cache_by_document: HashMap<&DocumentId, u64> = cache.documents.iter().map(|usage| (&usage.document_id, usage.bytes)).collect();
        let library = self.reader.library_usage()?;
        let mut documents: Vec<DocumentUsage> = library
            .iter()
            .map(|book| DocumentUsage {
                document_id: book.document_id.clone(),
                library_bytes: book.bytes,
                cache_bytes: cache_by_document.get(&book.document_id).copied().unwrap_or(0),
            })
            .collect();
        documents.sort_by(|a, b| (b.library_bytes + b.cache_bytes).cmp(&(a.library_bytes + a.cache_bytes)).then_with(|| a.document_id.cmp(&b.document_id)));

        let in_app_storage: Vec<u64> = self
            .jobs
            .records()
            .iter()
            .filter(|record| record.export.status == ExportStatus::Done && matches!(record.export.location, Some(ExportLocation::AppStorage { .. })))
            .map(|record| record.export.file_bytes.unwrap_or(0))
            .collect();
        let usage = StorageUsage {
            library_documents: u32::try_from(library.len()).unwrap_or(u32::MAX),
            library_bytes: library.iter().map(|book| book.bytes).sum(),
            cache_bytes: cache.bytes,
            cache_cap_bytes: cache.cap_bytes,
            export_files: u32::try_from(in_app_storage.len()).unwrap_or(u32::MAX),
            export_bytes: in_app_storage.iter().sum(),
            export_parts_bytes: self.exporter.parts_bytes()?,
            documents,
        };
        info!(service = Self::NAME, request_id = %ctx.request_id, library_documents = usage.library_documents, library_bytes = usage.library_bytes, cache_bytes = usage.cache_bytes, export_files = usage.export_files, export_bytes = usage.export_bytes, export_parts_bytes = usage.export_parts_bytes, "storage usage read");
        Ok(usage)
    }
}

/// Deletes every sentence clip not in use. Books, positions and exports stay.
pub struct ClearAudioCache {
    retention: Arc<dyn AudioRetention>,
}

impl ClearAudioCache {
    pub fn new(retention: Arc<dyn AudioRetention>) -> Self {
        Self { retention }
    }
}

impl UseCase for ClearAudioCache {
    const NAME: &'static str = "ClearAudioCache";
    type Args = ();
    type Output = AudioEviction;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<AudioEviction> {
        if let Err(error) = self.retention.refresh() {
            warn!(service = Self::NAME, error = %error, "audio cache could not be re-read");
        }
        let eviction = self.retention.clear()?;
        info!(service = Self::NAME, request_id = %ctx.request_id, clips = eviction.evicted_clips(), freed_bytes = eviction.freed_bytes, cache_bytes = eviction.cache_bytes, "audio cache cleared");
        Ok(eviction)
    }
}
