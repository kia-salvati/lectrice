//! The contracts the outer layers implement (spec §3, §4a). Adapters live in `lectrice-infra`,
//! and services receive them as `Arc<dyn Trait>`.

use std::ops::Range;

use crate::cleaned::CleanedDocument;
use crate::clock::Timestamp;
use crate::document::{Document, DocumentMeta, DocumentSummary, ReadingPosition};
use crate::error::AppResult;
use crate::export::{ExportRecord, ExportTarget, ExportedFile};
use crate::ids::{DocumentId, ExportId};
use crate::paging::{ListOptions, Page};
use crate::pdf::PdfContent;
use crate::speech::{Audio, AudioKey, SpeechRequest, Voice};

/// Names a PDF by its content, so the same file renamed is the same document (spec: the SHA-256 of the bytes).
pub trait ContentHasher: Send + Sync {
    fn document_id(&self, pdf: &[u8]) -> AppResult<DocumentId>;
}

pub trait PdfTextSource: Send + Sync {
    fn read_runs(&self, pdf: &[u8]) -> AppResult<PdfContent>;
}

pub trait DocumentCleaner: Send + Sync {
    /// Stored as `cleanerVersion`; raising it re-cleans saved documents from their original PDF.
    fn version(&self) -> u32;
    fn clean(&self, content: PdfContent) -> AppResult<CleanedDocument>;
}

pub trait SentenceSplitter: Send + Sync {
    /// Byte ranges into `text`, one per sentence, in order, without surrounding whitespace.
    fn split(&self, text: &str) -> Vec<Range<usize>>;
}

/// The voice plug-in point. Nothing outside the adapter knows which model is behind it.
pub trait SpeechEngine: Send + Sync {
    fn id(&self) -> &str;
    fn voices(&self) -> Vec<Voice>;
    /// The rate of every `Audio` this engine returns, so silence can be made to match it.
    fn sample_rate(&self) -> u32;
    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio>;
}

pub trait AudioEncoder: Send + Sync {
    fn mime_type(&self) -> &str;
    fn encode(&self, audio: &Audio) -> AppResult<Vec<u8>>;
}

pub trait DocumentReader: Send + Sync {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>>;
    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>>;
    fn find_original_pdf(&self, id: &DocumentId) -> AppResult<Option<Vec<u8>>>;
    /// Newest `last_opened_at` first.
    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>>;
}

pub trait DocumentWriter: Send + Sync {
    /// When the document is already saved, its stored meta is merged in ([`DocumentMeta::merged_with_saved`]),
    /// atomically with position saves and opens.
    fn save(&self, document: &Document, original_pdf: &[u8], meta: &DocumentMeta) -> AppResult<()>;
    fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> AppResult<()>;
    fn mark_opened(&self, id: &DocumentId, opened_at: Timestamp) -> AppResult<()>;
}

/// Turns encoded clips (as cached) back into PCM, so an export can reuse them.
pub trait AudioDecoder: Send + Sync {
    fn decode(&self, bytes: &[u8]) -> AppResult<Audio>;
}

pub trait AudioCache: Send + Sync {
    fn get(&self, key: &AudioKey) -> AppResult<Option<Vec<u8>>>;
    fn put(&self, key: &AudioKey, bytes: &[u8]) -> AppResult<()>;
}

/// Encodes a whole book into one file. Unavailable tooling is reported as `ExporterUnavailable`.
pub trait AudioExporter: Send + Sync {
    /// Fails with `ExporterUnavailable` when the tooling is missing, so a job can be refused up front.
    fn check_available(&self) -> AppResult<()>;
    fn begin(&self, target: &ExportTarget, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>>;
}

/// One export in progress: audio is streamed in sentence by sentence, so a book never sits in memory.
/// Dropping it without `finish` abandons the export.
pub trait ExportWriter: Send {
    /// Marks a chapter starting at the current position (used for M4B chapters).
    fn start_chapter(&mut self, title: &str) -> AppResult<()>;
    fn append(&mut self, audio: &Audio) -> AppResult<()>;
    fn finish(self: Box<Self>) -> AppResult<ExportedFile>;
}

/// Export job metadata on disk.
pub trait ExportRecordStore: Send + Sync {
    fn load_all(&self) -> AppResult<Vec<ExportRecord>>;
    fn save(&self, record: &ExportRecord) -> AppResult<()>;
    fn delete(&self, id: &ExportId) -> AppResult<()>;
}
