//! The library use cases: `ImportPdf`, `GetDocument`, `ListRecentDocuments`, `SaveReadingPosition`.

mod build;
#[cfg(test)]
pub(crate) mod fakes;
mod get_document;
mod import_pdf;
mod list_recent_documents;
mod pipeline;
mod save_reading_position;

pub use get_document::{GetDocument, GetDocumentArgs, OpenedDocument};
pub use import_pdf::{ImportPdf, ImportPdfArgs, ImportedPdf};
pub use list_recent_documents::ListRecentDocuments;
pub use pipeline::DocumentPipeline;
pub use save_reading_position::{SaveReadingPosition, SaveReadingPositionArgs};
