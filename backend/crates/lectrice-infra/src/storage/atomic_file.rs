use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

/// Writes to a temp file in the same directory, then renames it over `path`,
/// so readers see either the old file or the whole new one, never a partial write.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path.parent().ok_or_else(|| io::Error::other("target path has no parent directory"))?;
    let mut temp = NamedTempFile::new_in(directory)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Like [`write_atomically`], but never replaces an existing file: returns `false` (and writes nothing) when `path`
/// already exists. The check and the rename are one step, so of two writers racing for the same path exactly one wins.
pub(crate) fn write_new_atomically(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    let directory = path.parent().ok_or_else(|| io::Error::other("target path has no parent directory"))?;
    let mut temp = NamedTempFile::new_in(directory)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(path) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.error),
    }
}
