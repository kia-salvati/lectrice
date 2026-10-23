//! `config.toml` (spec §11), then `LECTRICE_<SECTION>_<KEY>` env overrides, then path resolution and validation.

mod env;
mod paths;

use std::path::{Path, PathBuf};

use axum::http::HeaderValue;
use lectrice_app::speech::ReadAheadSettings;
use serde::{Deserialize, Deserializer};

pub use env::CONFIG_PATH_ENV;

pub const DEFAULT_CONFIG_PATH: &str = "config.toml";

const HTTP_SCHEMES: [&str; 2] = ["http://", "https://"];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub audio: AudioConfig,
    pub upload: UploadConfig,
    pub pdf: PdfConfig,
    pub export: ExportConfig,
    pub speech: SpeechConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportConfig {
    /// A program name looked up on `PATH`, or a path.
    pub ffmpeg: String,
    /// 0 exports the whole book; more exports only the first sentences (for quick tests).
    pub max_sentences: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub port: u16,
    #[serde(deserialize_with = "header_value")]
    pub allowed_origin: HeaderValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioConfig {
    pub read_ahead_minutes: u32,
    pub keep_minutes_per_document: u32,
    pub cache_max_mb: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadConfig {
    pub max_mb: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfConfig {
    pub pdfium_library: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechConfig {
    pub engine: SpeechEngineKind,
    pub model_dir: PathBuf,
    pub default_voice: String,
    pub num_threads: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeechEngineKind {
    Kokoro,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config file {path}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("config file {path} is not valid TOML")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("unknown config override {0}")]
    UnknownOverride(String),
    #[error("config override {0} has the wrong type")]
    BadOverride(String),
    #[error("config has missing, unknown or mistyped keys")]
    Shape(#[source] toml::de::Error),
    #[error("cannot expand ~ in {0}: no home directory")]
    NoHome(PathBuf),
    #[error("invalid config value {key}: {reason}")]
    Invalid { key: &'static str, reason: &'static str },
}

/// `env` is the process environment (injected for tests); `home` expands a leading `~`.
/// Relative paths are resolved against the config file's directory.
pub fn load(path: &Path, env: impl IntoIterator<Item = (String, String)>, home: Option<&Path>) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read { path: path.to_path_buf(), source })?;
    let mut table: toml::Table = text.parse().map_err(|source| ConfigError::Parse { path: path.to_path_buf(), source })?;
    env::apply_overrides(&mut table, env)?;
    let mut config: Config = toml::Value::Table(table).try_into().map_err(ConfigError::Shape)?;

    let base_dir = path.parent().unwrap_or(Path::new(""));
    config.storage.data_dir = paths::resolve(&config.storage.data_dir, base_dir, home)?;
    config.pdf.pdfium_library = paths::resolve(&config.pdf.pdfium_library, base_dir, home)?;
    config.speech.model_dir = paths::resolve(&config.speech.model_dir, base_dir, home)?;

    validate(&config)?;
    Ok(config)
}

fn header_value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HeaderValue, D::Error> {
    let text = String::deserialize(deserializer)?;
    HeaderValue::from_str(&text).map_err(serde::de::Error::custom)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    let origin = config.server.allowed_origin.to_str().unwrap_or_default();
    require(config.server.port != 0, "server.port", "must be between 1 and 65535")?;
    require(HTTP_SCHEMES.iter().any(|scheme| origin.starts_with(scheme)), "server.allowed_origin", "must be an http(s) origin")?;
    require(config.audio.read_ahead_minutes > 0, "audio.read_ahead_minutes", "must be at least 1")?;
    let read_ahead =
        ReadAheadSettings { read_ahead_minutes: config.audio.read_ahead_minutes, keep_minutes_per_document: config.audio.keep_minutes_per_document };
    require(read_ahead.keeps_enough(), "audio.keep_minutes_per_document", "must be at least 1.5 x audio.read_ahead_minutes")?;
    require(config.audio.cache_max_mb > 0, "audio.cache_max_mb", "must be at least 1")?;
    require(config.upload.max_mb > 0, "upload.max_mb", "must be at least 1")?;
    require(!config.export.ffmpeg.trim().is_empty(), "export.ffmpeg", "must not be empty")?;
    require(!config.speech.default_voice.trim().is_empty(), "speech.default_voice", "must not be empty")?;
    require(config.speech.num_threads > 0, "speech.num_threads", "must be at least 1")
}

fn require(holds: bool, key: &'static str, reason: &'static str) -> Result<(), ConfigError> {
    if holds { Ok(()) } else { Err(ConfigError::Invalid { key, reason }) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_config_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.toml")
    }

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect()
    }

    fn load_shipped(env: &[(&str, &str)]) -> Result<Config, ConfigError> {
        load(&shipped_config_path(), vars(env), Some(Path::new("/home/reader")))
    }

    #[test]
    fn shipped_config_loads_with_spec_defaults() {
        let config = load_shipped(&[]).unwrap();
        assert_eq!(config.server.port, 7878);
        assert_eq!(config.server.allowed_origin, "http://localhost:5173");
        assert_eq!(config.storage.data_dir, Path::new("/home/reader/.local/share/lectrice"));
        assert_eq!((config.audio.read_ahead_minutes, config.audio.keep_minutes_per_document, config.audio.cache_max_mb), (10, 60, 2048));
        assert_eq!(config.upload.max_mb, 200);
        assert!(config.pdf.pdfium_library.ends_with("vendor/pdfium/lib/libpdfium.so"));
        assert!(config.pdf.pdfium_library.is_absolute());
        assert_eq!(config.speech.engine, SpeechEngineKind::Kokoro);
        assert!(config.speech.model_dir.ends_with("../models/kokoro-multi-lang-v1_0"));
        assert_eq!(config.speech.default_voice, "af_heart");
        assert_eq!(config.speech.num_threads, 4);
        assert_eq!((config.export.ffmpeg.as_str(), config.export.max_sentences), ("ffmpeg", 0));
    }

    #[test]
    fn export_settings_can_be_overridden() {
        let config = load_shipped(&[("LECTRICE_EXPORT_MAX_SENTENCES", "40"), ("LECTRICE_EXPORT_FFMPEG", "/opt/ffmpeg/bin/ffmpeg")]).unwrap();
        assert_eq!((config.export.ffmpeg.as_str(), config.export.max_sentences), ("/opt/ffmpeg/bin/ffmpeg", 40));
        let error = load_shipped(&[("LECTRICE_EXPORT_FFMPEG", " ")]).unwrap_err();
        assert!(matches!(error, ConfigError::Invalid { key: "export.ffmpeg", .. }));
    }

    #[test]
    fn env_overrides_replace_file_values() {
        let config =
            load_shipped(&[("LECTRICE_SERVER_PORT", "9000"), ("LECTRICE_SPEECH_DEFAULT_VOICE", "af_bella"), ("LECTRICE_STORAGE_DATA_DIR", "/srv/books")])
                .unwrap();
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.speech.default_voice, "af_bella");
        assert_eq!(config.storage.data_dir, Path::new("/srv/books"));
    }

    #[test]
    fn other_env_vars_are_ignored() {
        assert!(load_shipped(&[("PATH", "/usr/bin"), (CONFIG_PATH_ENV, "elsewhere.toml")]).is_ok());
    }

    #[test]
    fn unknown_and_mistyped_overrides_are_rejected() {
        assert!(matches!(load_shipped(&[("LECTRICE_SERVER_PROT", "1")]), Err(ConfigError::UnknownOverride(_))));
        assert!(matches!(load_shipped(&[("LECTRICE_SERVER_PORT", "lots")]), Err(ConfigError::BadOverride(_))));
    }

    #[test]
    fn invalid_values_are_rejected() {
        let error = load_shipped(&[("LECTRICE_SERVER_PORT", "0")]).unwrap_err();
        assert!(matches!(error, ConfigError::Invalid { key: "server.port", .. }));
        let error = load_shipped(&[("LECTRICE_AUDIO_KEEP_MINUTES_PER_DOCUMENT", "5")]).unwrap_err();
        assert!(matches!(error, ConfigError::Invalid { key: "audio.keep_minutes_per_document", .. }));
        // Review L3: a window equal to the read-ahead made clips get deleted and made again.
        let equal = [("LECTRICE_AUDIO_READ_AHEAD_MINUTES", "10"), ("LECTRICE_AUDIO_KEEP_MINUTES_PER_DOCUMENT", "10")];
        assert!(matches!(load_shipped(&equal).unwrap_err(), ConfigError::Invalid { key: "audio.keep_minutes_per_document", .. }));
        assert!(load_shipped(&[("LECTRICE_AUDIO_READ_AHEAD_MINUTES", "10"), ("LECTRICE_AUDIO_KEEP_MINUTES_PER_DOCUMENT", "15")]).is_ok());
        assert!(matches!(load_shipped(&[("LECTRICE_SPEECH_ENGINE", "remote")]), Err(ConfigError::Shape(_))));
    }
}
