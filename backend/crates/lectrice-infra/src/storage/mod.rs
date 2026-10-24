//! The library on disk (`FsDocumentRepository`, with `CachedLibrary` in front), the sentence audio cache
//! with its eviction (`FsAudioCache`), the voice samples (`FsVoiceSampleCache`) and the export job records
//! (`FsExportRecordStore`).

mod atomic_file;
mod audio_cache;
mod cached_library;
mod document_repository;
mod export_records;
mod path_component;
mod voice_samples;

pub use audio_cache::FsAudioCache;
pub use cached_library::CachedLibrary;
pub use document_repository::FsDocumentRepository;
pub use export_records::FsExportRecordStore;
#[cfg(feature = "ffmpeg-export")]
pub(crate) use path_component::safe_component;
pub use voice_samples::FsVoiceSampleCache;

use std::error::Error;
use std::io;
use std::path::Path;

use lectrice_domain::{AppError, ErrorName};

/// `ENOSPC` on Linux and Android, for an error whose kind wasn't mapped.
const NO_SPACE_OS_ERROR: i32 = 28;

/// The one way the file adapters report an I/O failure: the action and the path, never file contents. A full disk is
/// raised as `StorageFull` first, so callers can pause instead of failing.
pub(crate) fn storage_failed(action: &str, path: &Path, source: impl Error + Send + Sync + 'static) -> AppError {
    let error =
        if is_out_of_space(&source) { AppError::new(ErrorName::StorageFull).and(ErrorName::StorageFailed) } else { AppError::new(ErrorName::StorageFailed) };
    error.with_detail(format!("{action} {}", path.display())).with_source(source)
}

/// Looks through the cause chain; hound only exposes its I/O error through its own variant.
fn is_out_of_space(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        let io_error = error.downcast_ref::<io::Error>().or_else(|| match error.downcast_ref::<hound::Error>() {
            Some(hound::Error::IoError(io_error)) => Some(io_error),
            _ => None,
        });
        if io_error.is_some_and(|io_error| io_error.kind() == io::ErrorKind::StorageFull || io_error.raw_os_error() == Some(NO_SPACE_OS_ERROR)) {
            return true;
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_disk_is_storage_full_first() {
        let path = Path::new("/data/x");
        let by_kind = storage_failed("writing", path, io::Error::from(io::ErrorKind::StorageFull));
        let by_code = storage_failed("writing", path, io::Error::from_raw_os_error(NO_SPACE_OS_ERROR));
        let through_hound = storage_failed("writing", path, hound::Error::IoError(io::Error::from(io::ErrorKind::StorageFull)));
        for error in [by_kind, by_code, through_hound] {
            assert_eq!(error.names(), [ErrorName::StorageFull, ErrorName::StorageFailed]);
        }
        assert_eq!(storage_failed("writing", path, io::Error::other("broken")).names(), [ErrorName::StorageFailed]);
    }
}
