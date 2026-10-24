//! Removing books (soft remove, restore, purge) and the storage numbers: `RemoveDocument`, `RestoreDocument`,
//! `PurgeRemovedDocuments`, `ClearDocumentAudio`, `GetStorageUsage`, `ClearAudioCache`. They work without a speech
//! engine.

mod remove;
mod storage;
#[cfg(test)]
mod tests;

pub use remove::{ClearDocumentAudio, PurgeRemovedDocuments, PurgeRemovedDocumentsArgs, PurgeReport, RemoveDocument, RestoreDocument};
pub use storage::{ClearAudioCache, DocumentUsage, GetStorageUsage, StorageUsage};
