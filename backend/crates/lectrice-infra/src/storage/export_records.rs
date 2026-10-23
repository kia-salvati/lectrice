use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lectrice_domain::{AppResult, ExportId, ExportRecord, ExportRecordStore};

use super::atomic_file::write_atomically;
use super::path_component::safe_component;
use super::storage_failed;

const JOBS_DIR: &str = "exports/jobs";
const RECORD_EXTENSION: &str = "json";

/// One JSON file per export job: `<data_dir>/exports/jobs/<exportId>.json`, written atomically.
pub struct FsExportRecordStore {
    directory: PathBuf,
}

impl FsExportRecordStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { directory: data_dir.join(JOBS_DIR) }
    }

    fn record_path(&self, id: &ExportId) -> AppResult<PathBuf> {
        Ok(self.directory.join(format!("{}.{RECORD_EXTENSION}", safe_component(id.as_str())?)))
    }
}

impl ExportRecordStore for FsExportRecordStore {
    fn load_all(&self) -> AppResult<Vec<ExportRecord>> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(storage_failed("listing", &self.directory, error)),
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry.map_err(|error| storage_failed("listing", &self.directory, error))?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some(RECORD_EXTENSION) {
                continue;
            }
            let bytes = fs::read(&path).map_err(|error| storage_failed("reading", &path, error))?;
            records.push(serde_json::from_slice(&bytes).map_err(|error| storage_failed("parsing", &path, error))?);
        }
        Ok(records)
    }

    fn save(&self, record: &ExportRecord) -> AppResult<()> {
        let path = self.record_path(&record.export.id)?;
        fs::create_dir_all(&self.directory).map_err(|error| storage_failed("creating", &self.directory, error))?;
        let bytes = serde_json::to_vec(record).map_err(|error| storage_failed("serializing", &path, error))?;
        write_atomically(&path, &bytes).map_err(|error| storage_failed("writing", &path, error))
    }

    fn delete(&self, id: &ExportId) -> AppResult<()> {
        let path = self.record_path(id)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_failed("deleting", &path, error)),
        }
    }
}
