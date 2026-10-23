//! The library on disk (`FsDocumentRepository`, with `CachedLibrary` in front), the sentence audio cache
//! with its eviction (`FsAudioCache`) and the export job records (`FsExportRecordStore`).

mod atomic_file;
mod audio_cache;
mod cached_library;
mod document_repository;
mod export_records;
mod path_component;

pub use audio_cache::FsAudioCache;
pub use cached_library::CachedLibrary;
pub use document_repository::FsDocumentRepository;
pub use export_records::FsExportRecordStore;
#[cfg(feature = "ffmpeg-export")]
pub(crate) use path_component::safe_component;

use std::path::Path;

use lectrice_domain::{AppError, ErrorName};

/// The one way the file adapters report an I/O failure: the action and the path, never file contents.
pub(crate) fn storage_failed(action: &str, path: &Path, source: impl std::error::Error + Send + Sync + 'static) -> AppError {
    AppError::new(ErrorName::StorageFailed).with_detail(format!("{action} {}", path.display())).with_source(source)
}
