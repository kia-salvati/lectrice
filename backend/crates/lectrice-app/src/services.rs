//! The one place that wires adapters into services. Every shell (the HTTP server, the mobile bindings) picks its
//! adapters and settings, then calls [`assemble`], so the shells can't drift apart.

use std::sync::Arc;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioEncoder, AudioExporter, AudioRetention, Clock, ContentHasher, DocumentCleaner, DocumentReader,
    DocumentWriter, ErrorName, ExportRecordStore, PdfTextSource, SentenceSplitter, SpeechEngine, VoiceId,
};

use crate::documents::{DocumentPipeline, GetDocument, ImportPdf, ListRecentDocuments, SaveReadingPosition};
use crate::exports::{
    CancelAudioExport, ExportDeps, ExportJobs, ExportSettings, GetAudioExport, GetAudioExportFile, ListAudioExports, PauseAudioExport, ResumeAudioExports,
    StartAudioExport,
};
use crate::speech::{ListVoices, ReadAhead, ReadAheadSettings, SpeakSentence, SpeakSentenceDeps, SpeechWorker};

/// Every adapter the services need, so tests can assemble the app with fakes.
pub struct Adapters {
    pub hasher: Arc<dyn ContentHasher>,
    pub pdf_source: Arc<dyn PdfTextSource>,
    pub cleaner: Arc<dyn DocumentCleaner>,
    pub splitter: Arc<dyn SentenceSplitter>,
    pub reader: Arc<dyn DocumentReader>,
    pub writer: Arc<dyn DocumentWriter>,
    pub clock: Arc<dyn Clock>,
    /// `None` when the engine failed to load (or isn't installed yet).
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
    /// The largest PDF that is read.
    pub import_max_bytes: usize,
    pub default_voice: VoiceId,
    pub read_ahead: ReadAheadSettings,
    pub export: ExportSettings,
}

/// Every use case, built once. Speech and exports are `None` without a speech engine.
#[derive(Clone)]
pub struct Services {
    pub documents: Arc<DocumentServices>,
    pub speech: Option<Arc<SpeechServices>>,
    /// `None` with the speech engine: exports need it.
    pub exports: Option<Arc<ExportServices>>,
}

impl Services {
    pub fn speech(&self) -> AppResult<Arc<SpeechServices>> {
        self.speech.clone().ok_or_else(engine_unavailable)
    }

    pub fn exports(&self) -> AppResult<Arc<ExportServices>> {
        self.exports.clone().ok_or_else(engine_unavailable)
    }
}

fn engine_unavailable() -> AppError {
    AppError::new(ErrorName::SpeechEngineUnavailable).with_detail("the speech engine failed to start")
}

pub struct DocumentServices {
    pub import_pdf: ImportPdf,
    pub get_document: GetDocument,
    pub list_recent: ListRecentDocuments,
    pub save_position: SaveReadingPosition,
}

pub struct SpeechServices {
    pub list_voices: ListVoices,
    pub speak_sentence: SpeakSentence,
}

pub struct ExportServices {
    pub start: StartAudioExport,
    pub get: GetAudioExport,
    pub list: ListAudioExports,
    pub pause: PauseAudioExport,
    pub resume: ResumeAudioExports,
    pub cancel: CancelAudioExport,
    pub file: GetAudioExportFile,
}

/// Paused exports stay paused: the shell decides when they continue (`ExportServices::resume`).
pub fn assemble(adapters: Adapters, settings: Settings) -> AppResult<Services> {
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
    let documents = Arc::new(DocumentServices {
        import_pdf: ImportPdf::new(hasher, pipeline.clone(), reader.clone(), writer.clone(), clock.clone(), settings.import_max_bytes),
        get_document: GetDocument::new(pipeline, reader.clone(), writer.clone(), clock.clone()),
        list_recent: ListRecentDocuments::new(reader.clone()),
        save_position: SaveReadingPosition::new(reader.clone(), writer),
    });
    let Some(engine) = speech_engine else {
        return Ok(Services { documents, speech: None, exports: None });
    };

    check_default_voice(engine.as_ref(), &settings.default_voice)?;
    let worker = Arc::new(SpeechWorker::start(engine.clone())?);
    let audio = AudioAdapters { reader: reader.clone(), cache: audio_cache.clone(), retention: audio_retention, encoder: audio_encoder };
    let speech = speech_services(engine.clone(), worker.clone(), audio, settings.default_voice, settings.read_ahead)?;
    let export_deps = ExportDeps { worker, engine, reader, cache: audio_cache, decoder: audio_decoder, exporter, store: export_records, clock };
    let exports = export_services(export_deps, settings.export)?;
    Ok(Services { documents, speech: Some(Arc::new(speech)), exports: Some(Arc::new(exports)) })
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
    let detail = format!("default voice {default_voice} is not a voice of engine {}", engine.id());
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
        pause: PauseAudioExport::new(jobs.clone()),
        resume: ResumeAudioExports::new(jobs.clone()),
        cancel: CancelAudioExport::new(jobs.clone()),
        file: GetAudioExportFile::new(jobs, reader, engine),
    })
}
