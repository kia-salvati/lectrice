use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lectrice_domain::{AppResult, VoiceSampleCache, VoiceSampleKey};

use super::atomic_file::write_atomically;
use super::path_component::safe_component;
use super::storage_failed;

const SAMPLES_DIR: &str = "samples";
const SAMPLE_EXTENSION: &str = "wav";

/// Voice samples at `<cache_dir>/samples/<engineId>/<voiceId>.v<version>.wav`, beside (not inside) the sentence cache.
/// An empty file (the system cleared it halfway) is a miss.
pub struct FsVoiceSampleCache {
    root: PathBuf,
}

impl FsVoiceSampleCache {
    pub fn new(cache_dir: &Path) -> Self {
        Self { root: cache_dir.join(SAMPLES_DIR) }
    }

    fn path(&self, key: &VoiceSampleKey) -> AppResult<PathBuf> {
        let file = format!("{}.v{}.{SAMPLE_EXTENSION}", safe_component(key.voice_id.as_str())?, key.version);
        Ok(self.root.join(safe_component(&key.engine_id)?).join(file))
    }
}

impl VoiceSampleCache for FsVoiceSampleCache {
    fn get(&self, key: &VoiceSampleKey) -> AppResult<Option<Vec<u8>>> {
        let path = self.path(key)?;
        match fs::read(&path) {
            Ok(bytes) => Ok((!bytes.is_empty()).then_some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(storage_failed("reading", &path, error)),
        }
    }

    fn put(&self, key: &VoiceSampleKey, bytes: &[u8]) -> AppResult<()> {
        let path = self.path(key)?;
        let directory = path.parent().unwrap_or(&self.root);
        fs::create_dir_all(directory).map_err(|error| storage_failed("creating", directory, error))?;
        write_atomically(&path, bytes).map_err(|error| storage_failed("writing", &path, error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lectrice_domain::VoiceId;

    fn key(version: u32) -> VoiceSampleKey {
        VoiceSampleKey { engine_id: "engine-a".into(), voice_id: VoiceId::new("af_heart"), version }
    }

    #[test]
    fn a_sample_is_kept_per_engine_voice_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let samples = FsVoiceSampleCache::new(dir.path());

        assert_eq!(samples.get(&key(1)).unwrap(), None);
        samples.put(&key(1), b"RIFF").unwrap();

        assert_eq!(samples.get(&key(1)).unwrap().as_deref(), Some(&b"RIFF"[..]));
        assert_eq!(samples.get(&key(2)).unwrap(), None);
        assert!(dir.path().join("samples/engine-a/af_heart.v1.wav").is_file());
    }

    #[test]
    fn an_empty_file_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let samples = FsVoiceSampleCache::new(dir.path());
        samples.put(&key(1), b"").unwrap();

        assert_eq!(samples.get(&key(1)).unwrap(), None);
    }
}
