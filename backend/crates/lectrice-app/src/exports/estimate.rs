use std::sync::Arc;
use std::time::Instant;

use lectrice_domain::{
    AppError, AppResult, AudioKey, AudioRetention, Document, DocumentId, DocumentReader, ErrorName, ExportRange, ExportSpeed, SentenceId, SpeechEngine, VoiceId,
};
use tracing::{info, warn};

use super::jobs::ExportSettings;
use super::plan::{ExportPlan, ExportStep, PlanOptions, check_range, export_plan, spoken_chars};
use crate::speech::{chars_to_ms, elapsed_ms};
use crate::{RequestContext, UseCase};

#[derive(Debug, Clone)]
pub struct EstimateAudioExportArgs {
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    /// `None` is the loaded engine.
    pub engine_id: Option<String>,
    pub read_skipped: bool,
    pub speed: ExportSpeed,
    pub range: Option<ExportRange>,
}

/// Sizes an export before it starts, from the text (18 characters a second at 1x), without making any audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEstimate {
    pub sentences: u32,
    pub parts: u32,
    pub audio_ms_1x: u64,
    /// `audio_ms_1x` at the chosen speed.
    pub output_ms: u64,
    /// The part of `audio_ms_1x` already in the sentence cache for that voice and engine.
    pub cached_ms_1x: u64,
    /// The whole book's export at 1x; with the offsets, any range's length is a difference of two numbers.
    pub total_audio_ms_1x: u64,
    /// One per TOC entry, over the whole book's export.
    pub toc_offsets: Vec<TocOffset>,
    /// One per planned part of the whole book's export (for books without a TOC).
    pub part_offsets: Vec<PartOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocOffset {
    pub toc_index: u32,
    pub first_sentence: SentenceId,
    pub audio_ms_1x_before: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartOffset {
    pub first_sentence: SentenceId,
    /// The printed page label of the part's first sentence (the PDF page number when the book has none).
    pub page_label: String,
    pub audio_ms_1x_before: u64,
}

pub struct EstimateAudioExport {
    engine: Arc<dyn SpeechEngine>,
    reader: Arc<dyn DocumentReader>,
    retention: Arc<dyn AudioRetention>,
    settings: ExportSettings,
}

impl EstimateAudioExport {
    pub fn new(engine: Arc<dyn SpeechEngine>, reader: Arc<dyn DocumentReader>, retention: Arc<dyn AudioRetention>, settings: ExportSettings) -> Self {
        Self { engine, reader, retention, settings }
    }

    /// The system may have cleared clips behind the cache's back, so the index is re-read first.
    fn cached_chars(&self, document: &Document, plan: &ExportPlan<'_>, engine_id: &str, voice_id: &VoiceId) -> u64 {
        if let Err(error) = self.retention.refresh() {
            warn!(service = Self::NAME, doc_id = %document.id, error = %error, "audio cache could not be re-read");
        }
        let key = |sentence_id| AudioKey {
            document_id: document.id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: engine_id.to_string(),
            voice_id: voice_id.clone(),
            sentence_id,
        };
        sentences(plan).filter(|sentence| self.retention.contains(&key(sentence.id)).unwrap_or(false)).map(spoken_chars).sum()
    }
}

impl UseCase for EstimateAudioExport {
    const NAME: &'static str = "EstimateAudioExport";
    type Args = EstimateAudioExportArgs;
    type Output = ExportEstimate;

    fn run(&self, ctx: &RequestContext, args: EstimateAudioExportArgs) -> AppResult<ExportEstimate> {
        let EstimateAudioExportArgs { document_id, voice_id, engine_id, read_skipped, speed, range } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, voice_id = %voice_id, speed_percent = speed.percent(), ranged = range.is_some(), "estimating audio export");
        let started = Instant::now();
        if !self.engine.voices().iter().any(|voice| voice.id == voice_id) {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {voice_id}")));
        }
        let document = self.reader.find(&document_id)?.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        if let Some(range) = range {
            check_range(&document, range)?;
        }
        let engine_id = engine_id.unwrap_or_else(|| self.engine.id().to_string());

        let whole_book = PlanOptions { read_skipped, max_sentences: self.settings.max_sentences, part_minutes: self.settings.part_minutes, range: None };
        let book = export_plan(&document, whole_book);
        let chosen = export_plan(&document, PlanOptions { range, ..whole_book });
        let audio_ms_1x = chars_to_ms(chosen.spoken_chars);
        let estimate = ExportEstimate {
            sentences: chosen.sentences,
            parts: u32::try_from(chosen.parts.len()).unwrap_or(u32::MAX),
            audio_ms_1x,
            output_ms: (audio_ms_1x as f64 / f64::from(speed.as_factor())).round() as u64,
            cached_ms_1x: chars_to_ms(self.cached_chars(&document, &chosen, &engine_id, &voice_id)),
            total_audio_ms_1x: chars_to_ms(book.spoken_chars),
            toc_offsets: toc_offsets(&document, &book),
            part_offsets: part_offsets(&document, &book),
        };
        info!(service = Self::NAME, doc_id = %document_id, sentences = estimate.sentences, parts = estimate.parts, audio_ms_1x = estimate.audio_ms_1x, cached_ms_1x = estimate.cached_ms_1x, duration_ms = elapsed_ms(started), "audio export estimated");
        Ok(estimate)
    }
}

fn sentences<'a>(plan: &'a ExportPlan<'a>) -> impl Iterator<Item = &'a lectrice_domain::Sentence> + 'a {
    plan.steps.iter().filter_map(|step| match step {
        ExportStep::Sentence(sentence) => Some(*sentence),
        ExportStep::Chapter(_) => None,
    })
}

/// Each TOC entry's start: the spoken characters planned before its sentence.
fn toc_offsets(document: &Document, plan: &ExportPlan<'_>) -> Vec<TocOffset> {
    let mut starts: Vec<(SentenceId, u64)> = Vec::new();
    let mut chars = 0;
    for sentence in sentences(plan) {
        starts.push((sentence.id, chars));
        chars += spoken_chars(sentence);
    }
    document
        .toc
        .iter()
        .map(|entry| {
            let first_after = starts.partition_point(|(id, _)| *id < entry.sentence_id);
            let before = starts.get(first_after).map_or(chars, |(_, before)| *before);
            TocOffset { toc_index: entry.id, first_sentence: entry.sentence_id, audio_ms_1x_before: chars_to_ms(before) }
        })
        .collect()
}

fn part_offsets(document: &Document, plan: &ExportPlan<'_>) -> Vec<PartOffset> {
    let mut before = 0;
    let mut offsets = Vec::new();
    for part in &plan.parts {
        let first = plan.steps[part.steps.clone()].iter().find_map(|step| match step {
            ExportStep::Sentence(sentence) => Some(*sentence),
            ExportStep::Chapter(_) => None,
        });
        if let Some(sentence) = first {
            let page_label =
                document.pages.iter().find(|page| page.index == sentence.page_index).map_or_else(|| sentence.page_index.to_string(), |page| page.label.clone());
            offsets.push(PartOffset { first_sentence: sentence.id, page_label, audio_ms_1x_before: chars_to_ms(before) });
        }
        before += part.spoken_chars;
    }
    offsets
}
