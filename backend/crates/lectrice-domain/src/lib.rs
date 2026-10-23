//! Lectrice core domain: entities, ports, the error registry and the pure cleaning rules.
//! Depends on no other Lectrice crate and on no I/O library.

pub mod audio_retention;
pub mod cleaned;
pub mod cleaning;
pub mod clock;
pub mod document;
pub mod error;
pub mod export;
pub mod ids;
pub mod paging;
pub mod pdf;
pub mod ports;
pub mod speech;

pub use audio_retention::{AudioEviction, AudioRetention};
pub use cleaned::{CleanedBlock, CleanedDocument, TextSpan};
pub use clock::{Clock, Timestamp};
pub use document::{Block, BlockKind, Document, DocumentMeta, DocumentSummary, PageLabel, ReadingPosition, Sentence, TocEntry};
pub use error::{AppError, AppResult, ErrorName, ErrorSpec};
pub use export::{AudioExport, ChapterMark, ExportFormat, ExportPart, ExportRecord, ExportStatus, ExportTarget, ExportedFile};
pub use ids::{BlockId, DocumentId, ExportId, SentenceId, VoiceId};
pub use paging::{ListOptions, Page};
pub use pdf::{BoundingBox, OutlineItem, PdfContent, PdfPage, TextRun};
pub use ports::{
    AudioCache, AudioDecoder, AudioEncoder, AudioExporter, ContentHasher, DocumentCleaner, DocumentReader, DocumentWriter, ExportRecordStore, ExportWriter,
    PdfTextSource, SentenceSplitter, SpeechEngine,
};
pub use speech::{Audio, AudioKey, SpeechRequest, Voice, VoiceGender, VoiceLanguage};
