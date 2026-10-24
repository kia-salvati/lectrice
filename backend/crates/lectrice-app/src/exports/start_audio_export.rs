use std::sync::Arc;

use lectrice_domain::{
    AppError, AppResult, AudioExport, AudioExporter, DocumentId, DocumentReader, ErrorName, ExportFormat, ExportRange, ExportSpeed, SpeechEngine, TimeStretch,
    VoiceId,
};
use tracing::info;

use super::jobs::{ExportJobs, ExportSettings, NewExport};
use super::plan::{PlanOptions, check_range, export_plan};
use crate::{RequestContext, UseCase};

#[derive(Debug, Clone)]
pub struct StartAudioExportArgs {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    /// Read code, tables and captions instead of their cues, like the player's "Read code & tables aloud".
    pub read_skipped: bool,
    /// Baked into the file.
    pub speed: ExportSpeed,
    /// The model bundle to make it with; `None` is the loaded one. Another one's job waits until that one is loaded.
    pub engine_id: Option<String>,
    /// Only these sentences; `None` is the whole book.
    pub range: Option<ExportRange>,
    /// The shell's name for the range (only it knows the book's chapter numbering), added to the title tag.
    pub range_label: Option<String>,
}

/// Queues an export (spec §4a), or continues a paused one for the same file. Refused up front when the voice, the
/// document, the range, the speed or the exporter can't be used.
pub struct StartAudioExport {
    jobs: Arc<ExportJobs>,
    engine: Arc<dyn SpeechEngine>,
    reader: Arc<dyn DocumentReader>,
    exporter: Arc<dyn AudioExporter>,
    time_stretch: Option<Arc<dyn TimeStretch>>,
    settings: ExportSettings,
}

pub struct StartAudioExportDeps {
    pub jobs: Arc<ExportJobs>,
    pub engine: Arc<dyn SpeechEngine>,
    pub reader: Arc<dyn DocumentReader>,
    pub exporter: Arc<dyn AudioExporter>,
    pub time_stretch: Option<Arc<dyn TimeStretch>>,
}

impl StartAudioExport {
    pub fn new(deps: StartAudioExportDeps, settings: ExportSettings) -> Self {
        let StartAudioExportDeps { jobs, engine, reader, exporter, time_stretch } = deps;
        Self { jobs, engine, reader, exporter, time_stretch, settings }
    }
}

impl UseCase for StartAudioExport {
    const NAME: &'static str = "StartAudioExport";
    type Args = StartAudioExportArgs;
    type Output = AudioExport;

    fn run(&self, ctx: &RequestContext, args: StartAudioExportArgs) -> AppResult<AudioExport> {
        let StartAudioExportArgs { document_id, voice_id, format, read_skipped, speed, engine_id, range, range_label } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, voice_id = %voice_id, format = format.extension(), read_skipped, speed_percent = speed.percent(), engine_id = engine_id.as_deref(), ranged = range.is_some(), "starting audio export");

        if !self.engine.voices().iter().any(|voice| voice.id == voice_id) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")));
        }
        if !speed.is_normal() && self.time_stretch.is_none() {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail("this build can't bake a speed into exports"));
        }
        let engine_id = engine_id.map(|id| id.trim().to_string()).unwrap_or_else(|| self.engine.id().to_string());
        if engine_id.is_empty() {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail("engine id must not be empty"));
        }
        let document = self.reader.find(&document_id)?;
        info!(service = Self::NAME, doc_id = %document_id, found = document.is_some(), "document loaded");
        let document = document.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        if let Some(range) = range {
            check_range(&document, range)?;
        }

        self.exporter.check_available()?;
        info!(service = Self::NAME, "exporter available");

        let options = PlanOptions { read_skipped, max_sentences: self.settings.max_sentences, part_minutes: self.settings.part_minutes, range };
        let plan = export_plan(&document, options);
        if range.is_some() && plan.sentences == 0 {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail("the range has nothing to read"));
        }
        let (sentences_total, parts_total) = (plan.sentences, u32::try_from(plan.parts.len()).unwrap_or(u32::MAX));
        let range_label = range.and(range_label.map(|label| label.trim().to_string()).filter(|label| !label.is_empty()));
        let new = NewExport {
            document_id,
            voice_id,
            format,
            engine_id,
            speed,
            range,
            range_label,
            title: document.title.clone(),
            read_skipped,
            sentences_total,
            parts_total,
        };
        let (export, created) = self.jobs.enqueue(new)?;
        info!(service = Self::NAME, export_id = %export.id, created, status = ?export.status, sentences_total = export.sentences_total, waiting_for_engine = export.waiting_for_engine, "audio export queued");
        Ok(export)
    }
}
