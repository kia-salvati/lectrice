//! The contracts the outer layers implement (spec §3, §4a). Adapters live in `lectrice-infra`,
//! and services receive them as `Arc<dyn Trait>`.

use std::ops::Range;
use std::path::Path;

use crate::cleaned::CleanedDocument;
use crate::clock::Timestamp;
use crate::document::{Document, DocumentBytes, DocumentMeta, DocumentSummary, ReadingPosition};
use crate::error::AppResult;
use crate::export::{ChapterMark, ExportPart, ExportRecord, ExportSpeed, ExportTarget, ExportedFile};
use crate::ids::{DocumentId, ExportId};
use crate::paging::{ListOptions, Page};
use crate::pdf::PdfContent;
use crate::speech::{Audio, AudioKey, SpeechRequest, Voice, VoiceSampleKey};

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

/// A removed document (see [`DocumentWriter::set_removed`]) is invisible here: `find` and `find_summary` give `None`
/// and the lists skip it, until it is restored or deleted.
pub trait DocumentReader: Send + Sync {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>>;
    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>>;
    fn find_original_pdf(&self, id: &DocumentId) -> AppResult<Option<Vec<u8>>>;
    /// Newest `last_opened_at` first.
    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>>;
    /// Documents marked removed and not deleted yet.
    fn list_removed(&self) -> AppResult<Vec<DocumentId>>;
    /// The disk use of every document that isn't removed.
    fn library_usage(&self) -> AppResult<Vec<DocumentBytes>>;
}

pub trait DocumentWriter: Send + Sync {
    /// When the document is already saved, its stored meta is merged in ([`DocumentMeta::merged_with_saved`]),
    /// atomically with position saves and opens. A removed document stays removed.
    fn save(&self, document: &Document, original_pdf: &[u8], meta: &DocumentMeta) -> AppResult<()>;
    fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> AppResult<()>;
    fn mark_opened(&self, id: &DocumentId, opened_at: Timestamp) -> AppResult<()>;
    /// Marks a document removed (hidden, nothing deleted yet), or restores it with `None`.
    fn set_removed(&self, id: &DocumentId, removed_at: Option<Timestamp>) -> AppResult<()>;
    /// Deletes a document for good. Its bookkeeping goes first, so a crash halfway never leaves a half-listed book.
    fn delete(&self, id: &DocumentId) -> AppResult<()>;
}

/// Turns encoded clips (as cached) back into PCM, so an export can reuse them.
pub trait AudioDecoder: Send + Sync {
    fn decode(&self, bytes: &[u8]) -> AppResult<Audio>;
}

pub trait AudioCache: Send + Sync {
    fn get(&self, key: &AudioKey) -> AppResult<Option<Vec<u8>>>;
    fn put(&self, key: &AudioKey, bytes: &[u8]) -> AppResult<()>;
}

/// Encodes a whole book into one file, written as numbered parts and then joined, so a stopped export can resume.
/// Unavailable tooling is reported as `ExporterUnavailable`.
pub trait AudioExporter: Send + Sync {
    /// Fails with `ExporterUnavailable` when the tooling is missing, so a job can be refused up front.
    fn check_available(&self) -> AppResult<()>;
    /// Starts part `part_index` of the export. A part that was started before and not finished is replaced.
    fn begin_part(&self, target: &ExportTarget, part_index: u32, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>>;
    /// Joins the finished parts, in order, into the target file with the chapter marks (M4B only). Keeps the parts.
    fn assemble(&self, target: &ExportTarget, parts: &[ExportPart], chapters: &[ChapterMark]) -> AppResult<ExportedFile>;
    /// Deletes every part of an export. Parts that are already gone are fine.
    fn discard_parts(&self, export_id: &ExportId) -> AppResult<()>;
    /// The disk use of every export's parts.
    fn parts_bytes(&self) -> AppResult<u64>;
    /// Deletes a finished export's file. A file that is already gone is fine.
    fn delete_file(&self, path: &Path) -> AppResult<()>;
}

/// Changes the speed of mono PCM without changing its pitch. One stretcher per export part (it keeps state between
/// pushes).
pub trait TimeStretcher: Send {
    /// Feeds samples; appends whatever output is ready to `out`.
    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>);
    /// Ends the stream; appends the remaining output.
    fn flush(&mut self, out: &mut Vec<f32>);
}

pub trait TimeStretch: Send + Sync {
    fn stretcher(&self, sample_rate: u32, speed: ExportSpeed) -> Box<dyn TimeStretcher>;
}

/// Encoded voice samples, kept apart from the sentence cache (no window, no cap).
pub trait VoiceSampleCache: Send + Sync {
    fn get(&self, key: &VoiceSampleKey) -> AppResult<Option<Vec<u8>>>;
    fn put(&self, key: &VoiceSampleKey, bytes: &[u8]) -> AppResult<()>;
}

/// One part in progress: audio is streamed in sentence by sentence, so a book never sits in memory.
/// Dropping it without `finish` abandons the part.
pub trait ExportWriter: Send {
    fn append(&mut self, audio: &Audio) -> AppResult<()>;
    fn finish(self: Box<Self>) -> AppResult<ExportPart>;
}

/// Export job metadata on disk.
pub trait ExportRecordStore: Send + Sync {
    fn load_all(&self) -> AppResult<Vec<ExportRecord>>;
    fn save(&self, record: &ExportRecord) -> AppResult<()>;
    fn delete(&self, id: &ExportId) -> AppResult<()>;
}
