//! The server's composition root: picks the concrete adapters from the config, then wires them with
//! `lectrice_app::services::assemble` (shared with the mobile bindings).

use std::sync::Arc;
use std::time::Instant;

use lectrice_app::exports::ExportSettings;
use lectrice_app::services::{Adapters, Services, Settings, assemble};
use lectrice_app::speech::ReadAheadSettings;
use lectrice_app::{RequestContext, UseCase};
use lectrice_domain::cleaning::RuleBasedCleaner;
use lectrice_domain::{AppResult, AudioExporter, SpeechEngine, VoiceId};
use lectrice_infra::{
    CachedLibrary, FfmpegExporter, FsAudioCache, FsDocumentRepository, FsExportRecordStore, FsVoiceSampleCache, KokoroSettings, KokoroSpeechEngine,
    KokoroVariant, PdfiumTextSource, SentencexSplitter, Sha256ContentHasher, SonicTimeStretch, SystemClock, WavDecoder, WavEncoder,
};
use tracing::{info, warn};

use crate::config::{Config, ExportConfig, SpeechConfig, SpeechEngineKind};
use crate::state::AppState;

const SERVICE: &str = "composition";
/// The request id of the calls the server makes on its own while starting.
const STARTUP_REQUEST_ID: &str = "startup";
const BYTES_PER_MB: u64 = 1024 * 1024;
/// Parsed documents kept in memory (a book's `document.json` is a few hundred KB to a few MB).
const DOCUMENT_CACHE_CAPACITY: usize = 4;

/// Builds the real app from the config and continues exports a stop interrupted. A speech engine that fails to load
/// is logged and reported by `/api/health`; anything else that fails stops the start.
pub fn build_state(config: &Config) -> AppResult<AppState> {
    let library = Arc::new(CachedLibrary::new(Arc::new(FsDocumentRepository::new(&config.storage.data_dir)), DOCUMENT_CACHE_CAPACITY));
    let audio_cache = Arc::new(open_audio_cache(config)?);
    let (speech_engine, speech_engine_load_ms) = load_speech_engine(&config.speech);
    let adapters = Adapters {
        hasher: Arc::new(Sha256ContentHasher),
        pdf_source: Arc::new(PdfiumTextSource::new(&config.pdf.pdfium_library)?),
        cleaner: Arc::new(RuleBasedCleaner),
        splitter: Arc::new(SentencexSplitter),
        reader: library.clone(),
        writer: library,
        clock: Arc::new(SystemClock),
        speech_engine,
        speech_engine_load_ms,
        audio_cache: audio_cache.clone(),
        audio_retention: audio_cache,
        audio_encoder: Arc::new(WavEncoder),
        audio_decoder: Arc::new(WavDecoder),
        voice_samples: Arc::new(FsVoiceSampleCache::new(config.storage.cache_dir())),
        exporter: Arc::new(open_exporter(&config.export, &config.storage.data_dir)?),
        export_records: Arc::new(FsExportRecordStore::new(&config.storage.data_dir)),
        time_stretch: Some(Arc::new(SonicTimeStretch)),
    };
    let read_ahead =
        ReadAheadSettings { read_ahead_minutes: config.audio.read_ahead_minutes, keep_minutes_per_document: config.audio.keep_minutes_per_document };
    // One engine on the desktop, so 1x exports keep their 1.0 file names.
    let export = ExportSettings {
        max_sentences: (config.export.max_sentences > 0).then_some(config.export.max_sentences),
        short_file_names: true,
        ..ExportSettings::default()
    };
    let upload_max_bytes = megabytes_to_bytes(config.upload.max_mb);
    let settings = Settings { import_max_bytes: upload_max_bytes, default_voice: VoiceId::new(config.speech.default_voice.trim()), read_ahead, export };
    let services = assemble(adapters, settings)?;
    resume_exports(&services)?;
    Ok(AppState { services, upload_max_bytes })
}

/// The speech engine factory: `speech.engine` picks the adapter.
pub fn build_speech_engine(config: &SpeechConfig) -> AppResult<Arc<dyn SpeechEngine>> {
    match config.engine {
        SpeechEngineKind::Kokoro => {
            let settings = KokoroSettings {
                model_dir: config.model_dir.clone(),
                variant: KokoroVariant::Fp32,
                num_threads: config.num_threads,
                provider: config.provider.clone(),
            };
            Ok(Arc::new(KokoroSpeechEngine::load(&settings)?))
        }
    }
}

/// The engine and how long it took to load.
fn load_speech_engine(config: &SpeechConfig) -> (Option<Arc<dyn SpeechEngine>>, Option<u32>) {
    let started = Instant::now();
    match build_speech_engine(config) {
        Ok(engine) => {
            let load_seconds = started.elapsed().as_secs_f64();
            info!(
                service = SERVICE,
                engine_id = engine.id(),
                voices = engine.voices().len(),
                num_threads = config.num_threads,
                provider = config.provider.as_str(),
                load_seconds,
                "Speech engine loaded"
            );
            (Some(engine), u32::try_from(started.elapsed().as_millis()).ok())
        }
        Err(error) => {
            warn!(service = SERVICE, engine = ?config.engine, error = %error, "Speech engine failed to load; speech is unavailable");
            (None, None)
        }
    }
}

/// On the desktop an export a stop interrupted continues as soon as the server is up again.
fn resume_exports(services: &Services) -> AppResult<()> {
    let Some(exports) = &services.exports else { return Ok(()) };
    let active = exports.resume.run(&RequestContext::new(STARTUP_REQUEST_ID), ())?;
    info!(service = SERVICE, active = active.len(), "Interrupted exports resumed");
    Ok(())
}

/// Also removes partial files of exports a crash left behind. A missing ffmpeg only disables exports.
fn open_exporter(config: &ExportConfig, data_dir: &std::path::Path) -> AppResult<FfmpegExporter> {
    let exporter = FfmpegExporter::new(config.ffmpeg.trim(), data_dir);
    let removed = exporter.remove_partial_files()?;
    match exporter.check_available() {
        Ok(()) => info!(
            service = SERVICE,
            ffmpeg = config.ffmpeg.trim(),
            max_sentences = config.max_sentences,
            removed_partial_files = removed,
            "Audio exporter ready"
        ),
        Err(error) => {
            warn!(service = SERVICE, ffmpeg = config.ffmpeg.trim(), error = %error, "Audio exporter unavailable; exports will fail with ExporterUnavailable")
        }
    }
    Ok(exporter)
}

fn open_audio_cache(config: &Config) -> AppResult<FsAudioCache> {
    let started = Instant::now();
    let cache = FsAudioCache::open(config.storage.cache_dir(), config.audio.cache_max_mb.saturating_mul(BYTES_PER_MB))?;
    info!(service = SERVICE, cache_max_mb = config.audio.cache_max_mb, scan_ms = started.elapsed().as_millis() as u64, "Audio cache opened");
    Ok(cache)
}

fn megabytes_to_bytes(megabytes: u64) -> usize {
    usize::try_from(megabytes.saturating_mul(BYTES_PER_MB)).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_limit_is_converted_to_bytes() {
        assert_eq!(megabytes_to_bytes(200), 200 * 1024 * 1024);
        assert_eq!(megabytes_to_bytes(u64::MAX), usize::MAX);
    }
}
