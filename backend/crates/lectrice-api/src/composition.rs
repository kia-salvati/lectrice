//! The composition root: the only place that picks concrete adapters and wires them into services.

use std::sync::Arc;
use std::time::Instant;

use lectrice_app::documents::{DocumentPipeline, GetDocument, ImportPdf, ListRecentDocuments, SaveReadingPosition};
use lectrice_app::exports::{
    CancelAudioExport, ExportDeps, ExportJobs, ExportSettings, GetAudioExport, GetAudioExportFile, ListAudioExports, StartAudioExport,
};
use lectrice_app::speech::{ListVoices, ReadAhead, ReadAheadSettings, SpeakSentence, SpeakSentenceDeps, SpeechWorker};
use lectrice_domain::cleaning::RuleBasedCleaner;
use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioEncoder, AudioExporter, AudioRetention, Clock, ContentHasher, DocumentCleaner, DocumentReader,
    DocumentWriter, ErrorName, ExportRecordStore, PdfTextSource, SentenceSplitter, SpeechEngine, VoiceId,
};
use lectrice_infra::{
    CachedLibrary, FfmpegExporter, FsAudioCache, FsDocumentRepository, FsExportRecordStore, KokoroSettings, KokoroSpeechEngine, PdfiumTextSource,
    SentencexSplitter, Sha256ContentHasher, SystemClock, WavDecoder, WavEncoder,
};
use tracing::{info, warn};

use crate::config::{Config, ExportConfig, SpeechConfig, SpeechEngineKind};
use crate::state::{AppState, DocumentServices, ExportServices, SpeechServices};

const SERVICE: &str = "composition";
const BYTES_PER_MB: u64 = 1024 * 1024;
/// Parsed documents kept in memory (a book's `document.json` is a few hundred KB to a few MB).
const DOCUMENT_CACHE_CAPACITY: usize = 4;

/// Every adapter the services need, so tests can assemble the app with fakes.
pub struct Adapters {
    pub hasher: Arc<dyn ContentHasher>,
    pub pdf_source: Arc<dyn PdfTextSource>,
    pub cleaner: Arc<dyn DocumentCleaner>,
    pub splitter: Arc<dyn SentenceSplitter>,
    pub reader: Arc<dyn DocumentReader>,
    pub writer: Arc<dyn DocumentWriter>,
    pub clock: Arc<dyn Clock>,
    /// `None` when the engine failed to load.
    pub speech_engine: Option<Arc<dyn SpeechEngine>>,
    pub audio_cache: Arc<dyn AudioCache>,
    /// Usually the same object as `audio_cache`.
    pub audio_retention: Arc<dyn AudioRetention>,
    pub audio_encoder: Arc<dyn AudioEncoder>,
    /// Reads cached clips back for exports.
    pub audio_decoder: Arc<dyn AudioDecoder>,
    pub exporter: Arc<dyn AudioExporter>,
    pub export_records: Arc<dyn ExportRecordStore>,
}

pub struct Settings {
    pub upload_max_bytes: usize,
    pub default_voice: VoiceId,
    pub read_ahead: ReadAheadSettings,
    pub export: ExportSettings,
}

/// Builds the real app from the config. A speech engine that fails to load is logged and reported by
/// `/api/health`; anything else that fails stops the start.
pub fn build_state(config: &Config) -> AppResult<AppState> {
    let library = Arc::new(CachedLibrary::new(Arc::new(FsDocumentRepository::new(&config.storage.data_dir)), DOCUMENT_CACHE_CAPACITY));
    let audio_cache = Arc::new(open_audio_cache(config)?);
    let adapters = Adapters {
        hasher: Arc::new(Sha256ContentHasher),
        pdf_source: Arc::new(PdfiumTextSource::new(&config.pdf.pdfium_library)?),
        cleaner: Arc::new(RuleBasedCleaner),
        splitter: Arc::new(SentencexSplitter),
        reader: library.clone(),
        writer: library,
        clock: Arc::new(SystemClock),
        speech_engine: load_speech_engine(&config.speech),
        audio_cache: audio_cache.clone(),
        audio_retention: audio_cache,
        audio_encoder: Arc::new(WavEncoder),
        audio_decoder: Arc::new(WavDecoder),
        exporter: Arc::new(open_exporter(&config.export, &config.storage.data_dir)?),
        export_records: Arc::new(FsExportRecordStore::new(&config.storage.data_dir)),
    };
    let read_ahead =
        ReadAheadSettings { read_ahead_minutes: config.audio.read_ahead_minutes, keep_minutes_per_document: config.audio.keep_minutes_per_document };
    let export = ExportSettings { max_sentences: (config.export.max_sentences > 0).then_some(config.export.max_sentences) };
    let settings = Settings {
        upload_max_bytes: megabytes_to_bytes(config.upload.max_mb),
        default_voice: VoiceId::new(config.speech.default_voice.trim()),
        read_ahead,
        export,
    };
    assemble(adapters, settings)
}

pub fn assemble(adapters: Adapters, settings: Settings) -> AppResult<AppState> {
    let Adapters {
        hasher,
        pdf_source,
        cleaner,
        splitter,
        reader,
        writer,
        clock,
        speech_engine,
        audio_cache,
        audio_retention,
        audio_encoder,
        audio_decoder,
        exporter,
        export_records,
    } = adapters;
    let pipeline = Arc::new(DocumentPipeline::new(pdf_source, cleaner, splitter));
    let documents = DocumentServices {
        import_pdf: ImportPdf::new(hasher, pipeline.clone(), reader.clone(), writer.clone(), clock.clone(), settings.upload_max_bytes),
        get_document: GetDocument::new(pipeline, reader.clone(), writer.clone(), clock.clone()),
        list_recent: ListRecentDocuments::new(reader.clone()),
        save_position: SaveReadingPosition::new(reader.clone(), writer),
    };
    let Some(engine) = speech_engine else {
        return Ok(AppState { documents: Arc::new(documents), speech: None, exports: None, upload_max_bytes: settings.upload_max_bytes });
    };

    check_default_voice(engine.as_ref(), &settings.default_voice)?;
    let worker = Arc::new(SpeechWorker::start(engine.clone())?);
    let audio = AudioAdapters { reader: reader.clone(), cache: audio_cache.clone(), retention: audio_retention, encoder: audio_encoder };
    let speech = speech_services(engine.clone(), worker.clone(), audio, settings.default_voice, settings.read_ahead)?;
    let export_deps = ExportDeps { worker, engine, reader, cache: audio_cache, decoder: audio_decoder, exporter, store: export_records, clock };
    let exports = export_services(export_deps, settings.export)?;
    Ok(AppState {
        documents: Arc::new(documents),
        speech: Some(Arc::new(speech)),
        exports: Some(Arc::new(exports)),
        upload_max_bytes: settings.upload_max_bytes,
    })
}

/// The speech engine factory: `speech.engine` picks the adapter.
pub fn build_speech_engine(config: &SpeechConfig) -> AppResult<Arc<dyn SpeechEngine>> {
    match config.engine {
        SpeechEngineKind::Kokoro => {
            let settings = KokoroSettings { model_dir: config.model_dir.clone(), num_threads: config.num_threads };
            Ok(Arc::new(KokoroSpeechEngine::load(&settings)?))
        }
    }
}

fn load_speech_engine(config: &SpeechConfig) -> Option<Arc<dyn SpeechEngine>> {
    let started = Instant::now();
    match build_speech_engine(config) {
        Ok(engine) => {
            let load_seconds = started.elapsed().as_secs_f64();
            info!(
                service = SERVICE,
                engine_id = engine.id(),
                voices = engine.voices().len(),
                num_threads = config.num_threads,
                load_seconds,
                "Speech engine loaded"
            );
            Some(engine)
        }
        Err(error) => {
            warn!(service = SERVICE, engine = ?config.engine, error = %error, "Speech engine failed to load; speech is unavailable");
            None
        }
    }
}

struct AudioAdapters {
    reader: Arc<dyn DocumentReader>,
    cache: Arc<dyn AudioCache>,
    retention: Arc<dyn AudioRetention>,
    encoder: Arc<dyn AudioEncoder>,
}

fn check_default_voice(engine: &dyn SpeechEngine, default_voice: &VoiceId) -> AppResult<()> {
    if engine.voices().iter().any(|voice| voice.id == *default_voice) {
        return Ok(());
    }
    let detail = format!("speech.default_voice {default_voice} is not a voice of engine {}", engine.id());
    Err(AppError::new(ErrorName::VoiceUnknown).with_detail(detail))
}

fn speech_services(
    engine: Arc<dyn SpeechEngine>,
    worker: Arc<SpeechWorker>,
    audio: AudioAdapters,
    default_voice: VoiceId,
    read_ahead: ReadAheadSettings,
) -> AppResult<SpeechServices> {
    let AudioAdapters { reader, cache, retention, encoder } = audio;
    let read_ahead = ReadAhead::new(worker.clone(), cache.clone(), retention, encoder.clone(), read_ahead)?;
    Ok(SpeechServices {
        list_voices: ListVoices::new(engine.clone(), default_voice),
        speak_sentence: SpeakSentence::new(SpeakSentenceDeps { engine, worker, reader, cache, encoder, read_ahead }),
    })
}

/// Exports share the speech worker with playback (at the lowest priority).
fn export_services(deps: ExportDeps, settings: ExportSettings) -> AppResult<ExportServices> {
    let (engine, reader, exporter) = (deps.engine.clone(), deps.reader.clone(), deps.exporter.clone());
    let jobs = Arc::new(ExportJobs::start(deps, settings)?);
    Ok(ExportServices {
        start: StartAudioExport::new(jobs.clone(), engine.clone(), reader.clone(), exporter, settings),
        get: GetAudioExport::new(jobs.clone()),
        list: ListAudioExports::new(jobs.clone(), reader.clone()),
        cancel: CancelAudioExport::new(jobs.clone()),
        file: GetAudioExportFile::new(jobs, reader, engine),
    })
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
    let cache = FsAudioCache::open(&config.storage.data_dir, config.audio.cache_max_mb.saturating_mul(BYTES_PER_MB))?;
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
