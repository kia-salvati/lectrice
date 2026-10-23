use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, AudioExport, AudioExporter, DocumentId, DocumentReader, ErrorName, ExportFormat, SpeechEngine, VoiceId};
use tracing::info;

use super::jobs::{ExportJobs, ExportSettings, NewExport};
use super::plan::export_plan;
use crate::{RequestContext, UseCase};

#[derive(Debug, Clone)]
pub struct StartAudioExportArgs {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    /// Read code, tables and captions instead of their cues, like the player's "Read code & tables aloud".
    pub read_skipped: bool,
}

/// Queues a whole-book export (spec §4a). Refused up front when the voice, the document or the exporter is missing.
pub struct StartAudioExport {
    jobs: Arc<ExportJobs>,
    engine: Arc<dyn SpeechEngine>,
    reader: Arc<dyn DocumentReader>,
    exporter: Arc<dyn AudioExporter>,
    settings: ExportSettings,
}

impl StartAudioExport {
    pub fn new(
        jobs: Arc<ExportJobs>,
        engine: Arc<dyn SpeechEngine>,
        reader: Arc<dyn DocumentReader>,
        exporter: Arc<dyn AudioExporter>,
        settings: ExportSettings,
    ) -> Self {
        Self { jobs, engine, reader, exporter, settings }
    }
}

impl UseCase for StartAudioExport {
    const NAME: &'static str = "StartAudioExport";
    type Args = StartAudioExportArgs;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, args: StartAudioExportArgs) -> AppResult<AudioExport> {
        let StartAudioExportArgs { document_id, voice_id, format, read_skipped } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, voice_id = %voice_id, format = format.extension(), read_skipped, "starting audio export");

        if !self.engine.voices().iter().any(|voice| voice.id == voice_id) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")));
        }
        let document = self.reader.find(&document_id)?;
        info!(service = Self::NAME, doc_id = %document_id, found = document.is_some(), "document loaded");
        let document = document.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;

        self.exporter.check_available()?;
        info!(service = Self::NAME, "exporter available");

        let sentences_total = export_plan(&document, read_skipped, self.settings.max_sentences).sentences;
        let (export, created) = self.jobs.enqueue(NewExport { document_id, voice_id, format, read_skipped, sentences_total })?;
        info!(service = Self::NAME, export_id = %export.id, created, status = ?export.status, sentences_total = export.sentences_total, "audio export queued");
        Ok(export)
    }
}
