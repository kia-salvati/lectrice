use std::sync::Arc;

use lectrice_app::documents::{GetDocument, ImportPdf, ListRecentDocuments, SaveReadingPosition};
use lectrice_app::exports::{CancelAudioExport, GetAudioExport, GetAudioExportFile, ListAudioExports, StartAudioExport};
use lectrice_app::speech::{ListVoices, SpeakSentence};
use lectrice_domain::{AppError, AppResult, ErrorName};

/// Shared handles for controllers, built once in the composition root (`composition.rs`).
#[derive(Clone)]
pub struct AppState {
    pub documents: Arc<DocumentServices>,
    /// `None` when the speech engine failed to start; the server still runs and reports it in `/api/health`.
    pub speech: Option<Arc<SpeechServices>>,
    /// `None` with the speech engine: exports need it.
    pub exports: Option<Arc<ExportServices>>,
    /// `upload.max_mb` in bytes; the upload route stops reading past it.
    pub upload_max_bytes: usize,
}

impl AppState {
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
    pub cancel: CancelAudioExport,
    pub file: GetAudioExportFile,
}
