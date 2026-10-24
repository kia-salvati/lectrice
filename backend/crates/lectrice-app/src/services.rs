//! The one place that wires adapters into services. Every shell (the HTTP server, the mobile bindings) picks its
//! adapters and settings, then calls [`assemble`], so the shells can't drift apart.

use std::sync::Arc;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioDecoder, AudioEncoder, AudioExporter, AudioRetention, Clock, ContentHasher, DocumentCleaner, DocumentReader,
    DocumentWriter, ErrorName, ExportRecordStore, PdfTextSource, SentenceSplitter, SpeechEngine, TimeStretch, VoiceId, VoiceSampleCache,
};
use tracing::info;

use crate::documents::{DocumentPipeline, GetDocument, ImportPdf, ListRecentDocuments, SaveReadingPosition};
use crate::exports::{
    CancelAudioExport, DeleteAudioExport, EstimateAudioExport, ExportDeps, ExportJobs, ExportSettings, ExportSpeech, GetAudioExport, GetAudioExportFile,
    ListAllAudioExports, ListAudioExports, MarkExportSaved, PauseAudioExport, PauseAudioExports, ResumeAudioExport, ResumeAudioExports, StartAudioExport,
    StartAudioExportDeps,
};
use crate::library::{ClearAudioCache, ClearDocumentAudio, GetStorageUsage, PurgeRemovedDocuments, PurgeRemovedDocumentsArgs, RemoveDocument, RestoreDocument};
use crate::speech::{
    GetSpeechStats, ListVoices, ReadAhead, ReadAheadDeps, ReadAheadSettings, SetReadAheadMinutes, SpeakSentence, SpeakSentenceDeps, SpeakVoiceSample,
    SpeechWorker, StopAllReadAhead, StopReadAhead,
};
use crate::{RequestContext, UseCase};

const NAME: &str = "assemble";
/// The request id of the calls the core makes on its own while starting.
const STARTUP_REQUEST_ID: &str = "startup";

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
    /// How long the shell took to load the engine, for the speech stats.
    pub speech_engine_load_ms: Option<u32>,
    pub audio_cache: Arc<dyn AudioCache>,
    /// Usually the same object as `audio_cache`.
    pub audio_retention: Arc<dyn AudioRetention>,
    pub audio_encoder: Arc<dyn AudioEncoder>,
    /// Reads cached clips back for exports.
    pub audio_decoder: Arc<dyn AudioDecoder>,
    pub voice_samples: Arc<dyn VoiceSampleCache>,
    pub exporter: Arc<dyn AudioExporter>,
    pub export_records: Arc<dyn ExportRecordStore>,
    /// Bakes a speed into exports; without it only 1x exports are accepted.
    pub time_stretch: Option<Arc<dyn TimeStretch>>,
}

pub struct Settings {
    /// The largest PDF that is read.
    pub import_max_bytes: usize,
    pub default_voice: VoiceId,
    pub read_ahead: ReadAheadSettings,
    pub export: ExportSettings,
}

/// Every use case, built once. Speech and exports are `None` without a speech engine; the library always works.
#[derive(Clone)]
pub struct Services {
    pub documents: Arc<DocumentServices>,
    pub library: Arc<LibraryServices>,
    pub speech: Option<Arc<SpeechServices>>,
    /// `None` without the speech engine: exports need it.
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
    cleaner_version: u32,
}

impl DocumentServices {
    /// A document summary with an older `cleaner_version` is cleaned again when it is opened.
    pub fn current_cleaner_version(&self) -> u32 {
        self.cleaner_version
    }
}

/// Removing books and the storage screens: none of it needs the speech engine.
pub struct LibraryServices {
    pub remove: RemoveDocument,
    pub restore: RestoreDocument,
    pub purge_removed: PurgeRemovedDocuments,
    pub clear_document_audio: ClearDocumentAudio,
    pub storage_usage: GetStorageUsage,
    pub clear_audio_cache: ClearAudioCache,
    pub list_all_exports: ListAllAudioExports,
    pub delete_export: DeleteAudioExport,
    pub mark_export_saved: MarkExportSaved,
}

pub struct SpeechServices {
    pub list_voices: ListVoices,
    pub speak_sentence: SpeakSentence,
    pub voice_sample: SpeakVoiceSample,
    pub stats: GetSpeechStats,
    pub stop_read_ahead: StopReadAhead,
    pub stop_all_read_ahead: StopAllReadAhead,
    pub set_read_ahead_minutes: SetReadAheadMinutes,
}

pub struct ExportServices {
    pub start: StartAudioExport,
    pub estimate: EstimateAudioExport,
    pub get: GetAudioExport,
    pub list: ListAudioExports,
    pub pause: PauseAudioExport,
    pub pause_all: PauseAudioExports,
    /// Continues what the system or an engine switch paused.
    pub resume: ResumeAudioExports,
    /// Continues one export, whatever paused it.
    pub resume_one: ResumeAudioExport,
    pub cancel: CancelAudioExport,
    pub file: GetAudioExportFile,
}

/// Paused exports stay paused: the shell decides when they continue (`ExportServices::resume`). Books removed and not
/// purged (the app stopped during an undo window) are purged here, keeping their exports.
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
        speech_engine_load_ms,
        audio_cache,
        audio_retention,
        audio_encoder,
        audio_decoder,
        voice_samples,
        exporter,
        export_records,
        time_stretch,
    } = adapters;
    let cleaner_version = cleaner.version();
    let pipeline = Arc::new(DocumentPipeline::new(pdf_source, cleaner, splitter));
    let documents = Arc::new(DocumentServices {
        import_pdf: ImportPdf::new(hasher, pipeline.clone(), reader.clone(), writer.clone(), clock.clone(), settings.import_max_bytes),
        get_document: GetDocument::new(pipeline, reader.clone(), writer.clone(), clock.clone()),
        list_recent: ListRecentDocuments::new(reader.clone()),
        save_position: SaveReadingPosition::new(reader.clone(), writer.clone()),
        cleaner_version,
    });

    let speech = match &speech_engine {
        Some(engine) => {
            check_default_voice(engine.as_ref(), &settings.default_voice)?;
            let worker = Arc::new(SpeechWorker::start(engine.clone())?);
            let deps = ReadAheadDeps {
                worker: worker.clone(),
                reader: reader.clone(),
                cache: audio_cache.clone(),
                retention: audio_retention.clone(),
                encoder: audio_encoder.clone(),
            };
            Some((engine.clone(), worker, Arc::new(ReadAhead::new(deps, settings.read_ahead)?)))
        }
        None => None,
    };

    let export_deps = ExportDeps {
        speech: speech.as_ref().map(|(engine, worker, _)| ExportSpeech { worker: worker.clone(), engine: engine.clone() }),
        reader: reader.clone(),
        cache: audio_cache.clone(),
        decoder: audio_decoder,
        exporter: exporter.clone(),
        store: export_records,
        clock: clock.clone(),
        time_stretch: time_stretch.clone(),
    };
    let jobs = Arc::new(ExportJobs::start(export_deps, settings.export)?);
    let library = Arc::new(LibraryServices {
        remove: RemoveDocument::new(reader.clone(), writer.clone(), clock, speech.as_ref().map(|(_, _, read_ahead)| read_ahead.clone())),
        restore: RestoreDocument::new(reader.clone(), writer.clone()),
        purge_removed: PurgeRemovedDocuments::new(reader.clone(), writer, audio_retention.clone(), jobs.clone()),
        clear_document_audio: ClearDocumentAudio::new(audio_retention.clone()),
        storage_usage: GetStorageUsage::new(reader.clone(), audio_retention.clone(), jobs.clone(), exporter.clone()),
        clear_audio_cache: ClearAudioCache::new(audio_retention.clone()),
        list_all_exports: ListAllAudioExports::new(jobs.clone()),
        delete_export: DeleteAudioExport::new(jobs.clone()),
        mark_export_saved: MarkExportSaved::new(jobs.clone()),
    });
    let purged = library.purge_removed.run(&RequestContext::new(STARTUP_REQUEST_ID), PurgeRemovedDocumentsArgs::default())?;
    info!(service = NAME, purged = purged.documents, failed = purged.failed, "removed documents purged at start");

    let Some((engine, worker, read_ahead)) = speech else {
        return Ok(Services { documents, library, speech: None, exports: None });
    };
    let speech = SpeechServices {
        list_voices: ListVoices::new(engine.clone(), settings.default_voice),
        speak_sentence: SpeakSentence::new(SpeakSentenceDeps {
            engine: engine.clone(),
            worker: worker.clone(),
            reader: reader.clone(),
            cache: audio_cache,
            encoder: audio_encoder.clone(),
            read_ahead: read_ahead.clone(),
        }),
        voice_sample: SpeakVoiceSample::new(engine.clone(), worker.clone(), voice_samples, audio_encoder),
        stats: GetSpeechStats::new(engine.clone(), worker, speech_engine_load_ms),
        stop_read_ahead: StopReadAhead::new(read_ahead.clone()),
        stop_all_read_ahead: StopAllReadAhead::new(read_ahead.clone()),
        set_read_ahead_minutes: SetReadAheadMinutes::new(read_ahead),
    };
    let start_deps = StartAudioExportDeps { jobs: jobs.clone(), engine: engine.clone(), reader: reader.clone(), exporter, time_stretch };
    let exports = ExportServices {
        start: StartAudioExport::new(start_deps, settings.export),
        estimate: EstimateAudioExport::new(engine.clone(), reader.clone(), audio_retention, settings.export),
        get: GetAudioExport::new(jobs.clone()),
        list: ListAudioExports::new(jobs.clone(), reader.clone()),
        pause: PauseAudioExport::new(jobs.clone()),
        pause_all: PauseAudioExports::new(jobs.clone()),
        resume: ResumeAudioExports::new(jobs.clone()),
        resume_one: ResumeAudioExport::new(jobs.clone()),
        cancel: CancelAudioExport::new(jobs.clone()),
        file: GetAudioExportFile::new(jobs, reader, engine),
    };
    Ok(Services { documents, library, speech: Some(Arc::new(speech)), exports: Some(Arc::new(exports)) })
}

fn check_default_voice(engine: &dyn SpeechEngine, default_voice: &VoiceId) -> AppResult<()> {
    if engine.voices().iter().any(|voice| voice.id == *default_voice) {
        return Ok(());
    }
    let detail = format!("default voice {default_voice} is not a voice of engine {}", engine.id());
    Err(AppError::new(ErrorName::VoiceUnknown).with_detail(detail))
}
