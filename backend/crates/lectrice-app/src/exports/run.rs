//! One export from start (or from its first unfinished part) to finish: sentences in playback order, cached clips
//! reused, the rest made by the speech worker at export priority and streamed straight into the part being written
//! (never into the cache). A baked-in speed goes through a time-stretcher between the clip and the part. Finished
//! parts are reported one by one, then joined into the book's file.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioDecoder, AudioExport, AudioExporter, AudioKey, ChapterMark, Document, DocumentReader, ErrorName, ExportPart,
    ExportRecord, ExportSpeed, ExportTarget, ExportWriter, ExportedFile, Sentence, SpeechEngine, SpeechRequest, TimeStretch, TimeStretcher, VoiceId,
    export_file_stem,
};
use tracing::{info, warn};

use super::jobs::ExportSettings;
use super::plan::{ExportPlan, ExportStep, PartPlan, PlanOptions, export_plan, spoken_chars};
use super::progress::{SynthesisRate, checkpoint, percent};
use crate::speech::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechWorker, elapsed_ms};

const NAME: &str = "ExportRunner";
/// Sentences asked for ahead of the encoder, so the worker never idles while a clip is being encoded.
const PIPELINE_DEPTH: usize = 4;
/// How often a wait for speech checks whether the export was stopped.
const STOP_POLL: Duration = Duration::from_millis(200);
const MILLIS_PER_SECOND: f64 = 1000.0;

pub(crate) struct ExportRunnerDeps {
    pub worker: Arc<SpeechWorker>,
    pub engine: Arc<dyn SpeechEngine>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub decoder: Arc<dyn AudioDecoder>,
    pub exporter: Arc<dyn AudioExporter>,
    /// `None` when the shell has none; exports are then refused any speed but 1x.
    pub time_stretch: Option<Arc<dyn TimeStretch>>,
}

pub(crate) struct ExportRunner {
    deps: ExportRunnerDeps,
    settings: ExportSettings,
}

pub(crate) enum RunOutcome {
    Done(ExportedFile),
    /// Paused, cancelled or shut down; the job registry knows which.
    Stopped,
    Failed(AppError),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Progress {
    pub sentences_done: u32,
    pub estimated_seconds_left: Option<u32>,
    /// Another 5 % was reached: worth saving.
    pub checkpoint: bool,
}

/// A part as the runner hands it to the registry: the file, its chapter marks and the 1x samples written so far.
pub(crate) struct FinishedPart {
    pub part: ExportPart,
    pub chapters: Vec<ChapterMark>,
    pub source_samples: u64,
}

/// What the runner tells the job registry while it works.
pub(crate) trait RunReport {
    fn progress(&mut self, progress: Progress);
    /// Saved before the next part starts, so a restart continues after this part.
    fn part_finished(&mut self, finished: FinishedPart);
    /// Every part is written; joining them starts.
    fn assembling(&mut self);
}

enum Clip {
    Ready(Arc<Audio>),
    Silent,
    Pending(PendingSpeech),
}

/// A sentence asked of the speech worker. Dropping it withdraws the request, so an early return never leaves jobs queued.
struct PendingSpeech {
    receiver: Receiver<SpeechOutcome>,
    cancel: CancelHandle,
}

impl Drop for PendingSpeech {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

enum Prepared {
    Chapter(String),
    Sentence { clip: Clip, chars: u64 },
}

/// Counters across the whole run, including the parts finished before a resume.
struct RunState {
    started: Instant,
    /// What the files hold (fewer samples than the 1x audio when the speed is baked in).
    output_samples: u64,
    /// The 1x audio so far; chapter marks are timed from it, which avoids the stretcher's delay.
    source_samples: u64,
    sentences_done: u32,
    done_chars: u64,
    last_checkpoint: u64,
    rate: SynthesisRate,
    last_sentence_end: Instant,
    cached: u32,
    synthesized: u32,
    silent: u32,
}

/// What one run works on: the job, its document and plan, and where its output goes.
struct Run<'a> {
    record: &'a ExportRecord,
    document: &'a Document,
    plan: &'a ExportPlan<'a>,
    target: ExportTarget,
    sample_rate: u32,
}

impl ExportRunner {
    pub fn new(deps: ExportRunnerDeps, settings: ExportSettings) -> Self {
        Self { deps, settings }
    }

    pub fn run(&self, record: &ExportRecord, stop: &AtomicBool, report: &mut dyn RunReport) -> RunOutcome {
        match self.try_run(record, stop, report) {
            Ok(Some(file)) => RunOutcome::Done(file),
            Ok(None) => RunOutcome::Stopped,
            Err(error) => RunOutcome::Failed(error),
        }
    }

    /// `Ok(None)` when stopped.
    fn try_run(&self, record: &ExportRecord, stop: &AtomicBool, report: &mut dyn RunReport) -> AppResult<Option<ExportedFile>> {
        let export = &record.export;
        let document = self.deps.reader.find(&export.document_id)?.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        let options = PlanOptions {
            read_skipped: record.read_skipped,
            max_sentences: self.settings.max_sentences,
            part_minutes: self.settings.part_minutes,
            range: export.range,
        };
        let plan = export_plan(&document, options);
        let first_part = record.parts.len();
        if plan.sentences != export.sentences_total || first_part > plan.parts.len() {
            let detail = format!("the document changed since export {} started ({} sentences, was {})", export.id, plan.sentences, export.sentences_total);
            return Err(AppError::new(ErrorName::Internal).with_detail(detail));
        }
        if !export.speed.is_normal() && self.deps.time_stretch.is_none() {
            return Err(AppError::new(ErrorName::Internal).with_detail(format!("export {} needs a time-stretch and there is none", export.id)));
        }
        let chapters = plan.steps.len() - plan.sentences as usize;
        info!(service = NAME, export_id = %export.id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), engine_id = %export.engine_id, speed_percent = export.speed.percent(), ranged = export.range.is_some(), read_skipped = record.read_skipped, sentences = plan.sentences, chapters, spoken_chars = plan.spoken_chars, parts_total = plan.parts.len(), first_part, "export started");

        let run = Run { record, document: &document, plan: &plan, target: self.target(export, &document), sample_rate: self.deps.engine.sample_rate() };
        let finished = &plan.parts[..first_part];
        let output_samples: u64 = record.parts.iter().map(|part| part.samples).sum();
        let mut state = RunState {
            started: Instant::now(),
            output_samples,
            // Records saved before v1.1 have no count; they were all made at 1x.
            source_samples: if record.source_samples == 0 { output_samples } else { record.source_samples },
            sentences_done: finished.iter().map(|part| part.sentences).sum(),
            done_chars: finished.iter().map(|part| part.spoken_chars).sum(),
            last_checkpoint: 0,
            rate: SynthesisRate::default(),
            last_sentence_end: Instant::now(),
            cached: 0,
            synthesized: 0,
            silent: 0,
        };
        let mut parts = record.parts.clone();
        let mut chapter_marks = record.chapters.clone();

        for (index, part_plan) in plan.parts.iter().enumerate().skip(first_part) {
            let part_index = u32::try_from(index).map_err(|_| AppError::new(ErrorName::Internal).with_detail("too many export parts"))?;
            let Some((part, marks)) = self.write_part(&run, part_plan, part_index, stop, &mut state, report)? else {
                return Ok(None);
            };
            info!(service = NAME, export_id = %export.id, part_index, samples = part.samples, chapters = marks.len(), sentences_done = state.sentences_done, elapsed_ms = elapsed_ms(state.started), "export part written");
            parts.push(part.clone());
            chapter_marks.extend(marks.iter().cloned());
            report.part_finished(FinishedPart { part, chapters: marks, source_samples: state.source_samples });
        }

        if state.output_samples == 0 {
            return Err(AppError::new(ErrorName::Internal).with_detail("the export has no audio (no sentence to speak)"));
        }
        if stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
        report.assembling();
        let file = self.deps.exporter.assemble(&run.target, &parts, &chapter_marks)?;
        info!(service = NAME, export_id = %export.id, path = %file.path.display(), bytes = file.size_bytes, parts = parts.len(), chapters = chapter_marks.len(), cached = state.cached, synthesized = state.synthesized, elapsed_ms = elapsed_ms(state.started), "export file written");
        Ok(Some(file))
    }

    /// `Ok(None)` when stopped; the unfinished part is dropped, which abandons it.
    fn write_part(
        &self,
        run: &Run<'_>,
        part: &PartPlan,
        part_index: u32,
        stop: &AtomicBool,
        state: &mut RunState,
        report: &mut dyn RunReport,
    ) -> AppResult<Option<(ExportPart, Vec<ChapterMark>)>> {
        let export = &run.record.export;
        let mut writer = PartWriter {
            writer: self.deps.exporter.begin_part(&run.target, part_index, run.sample_rate)?,
            stretcher: self.stretcher(export.speed, run.sample_rate),
            sample_rate: run.sample_rate,
        };
        // Marks are timed from what the earlier parts really hold, so a stretcher's small error per part never adds up.
        let part_start = PartStart { output_samples: state.output_samples, source_samples: state.source_samples };
        let mut marks = Vec::new();
        let mut window: VecDeque<Prepared> = VecDeque::new();
        let mut steps = run.plan.steps[part.steps.clone()].iter();

        loop {
            while window.len() < PIPELINE_DEPTH {
                let Some(step) = steps.next() else { break };
                window.push_back(self.prepare(run.document, export.voice_id.clone(), step));
            }
            let Some(prepared) = window.pop_front() else { break };
            if stop.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let (clip, chars) = match prepared {
                Prepared::Chapter(title) => {
                    marks.push(ChapterMark { title, start_ms: part_start.mark_ms(state.source_samples, run.sample_rate, export.speed) });
                    continue;
                }
                Prepared::Sentence { clip, chars } => (clip, chars),
            };
            let synthesized_before = state.synthesized;
            match receive(clip, stop, state)? {
                Received::Stopped => return Ok(None),
                Received::Silent => {}
                Received::Audio(audio) => {
                    state.output_samples += writer.append(&audio)?;
                    state.source_samples += audio.samples.len() as u64;
                }
            }

            state.sentences_done += 1;
            state.done_chars += chars;
            // With the pipeline full, the time between two sentences coming out is what the engine took for one.
            let sentence_end = Instant::now();
            if state.synthesized > synthesized_before {
                state.rate.record(chars, sentence_end - state.last_sentence_end);
            }
            state.last_sentence_end = sentence_end;
            let total = run.plan.sentences;
            let reached = checkpoint(state.sentences_done, total);
            let progress = Progress {
                sentences_done: state.sentences_done,
                estimated_seconds_left: state.rate.seconds_left(run.plan.spoken_chars.saturating_sub(state.done_chars)),
                checkpoint: reached > state.last_checkpoint,
            };
            if progress.checkpoint {
                state.last_checkpoint = reached;
                info!(service = NAME, export_id = %export.id, sentences_done = state.sentences_done, sentences_total = total, percent = percent(state.sentences_done, total), cached = state.cached, synthesized = state.synthesized, silent = state.silent, audio_seconds = state.source_samples as f64 / f64::from(run.sample_rate.max(1)), estimated_seconds_left = progress.estimated_seconds_left, elapsed_ms = elapsed_ms(state.started), "export progress");
            }
            report.progress(progress);
        }

        if stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let (part, flushed) = writer.finish()?;
        state.output_samples += flushed;
        Ok(Some((part, marks)))
    }

    /// No stretcher at 1x, so the output is the clips' samples unchanged. A new one per part: a part is a whole stream.
    fn stretcher(&self, speed: ExportSpeed, sample_rate: u32) -> Option<Box<dyn TimeStretcher>> {
        if speed.is_normal() {
            return None;
        }
        self.deps.time_stretch.as_ref().map(|time_stretch| time_stretch.stretcher(sample_rate, speed))
    }

    fn target(&self, export: &AudioExport, document: &Document) -> ExportTarget {
        let short_names = self.settings.short_file_names;
        let book_title = if export.title.is_empty() { document.title.clone() } else { export.title.clone() };
        ExportTarget {
            export_id: export.id.clone(),
            document_id: export.document_id.clone(),
            voice_id: export.voice_id.clone(),
            format: export.format,
            engine_id: export.engine_id.clone(),
            speed: export.speed,
            range: export.range,
            file_stem: export_file_stem(&export.voice_id, &export.engine_id, export.speed, export.range, short_names),
            title: export.range_label.as_ref().map_or_else(|| book_title.clone(), |label| format!("{book_title} ({label})")),
            artist: voice_name(self.deps.engine.as_ref(), &export.voice_id),
        }
    }

    fn prepare(&self, document: &Document, voice_id: VoiceId, step: &ExportStep<'_>) -> Prepared {
        match step {
            ExportStep::Chapter(title) => Prepared::Chapter(title.clone()),
            ExportStep::Sentence(sentence) => Prepared::Sentence { clip: self.clip(document, voice_id, sentence), chars: spoken_chars(sentence) },
        }
    }

    /// A cached clip, or a request to the speech worker. Sentences with nothing to say (e.g. only a URL) add no audio.
    fn clip(&self, document: &Document, voice_id: VoiceId, sentence: &Sentence) -> Clip {
        let text = sentence.spoken_text().trim();
        if text.is_empty() {
            return Clip::Silent;
        }
        let key = AudioKey {
            document_id: document.id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: self.deps.engine.id().to_string(),
            voice_id: voice_id.clone(),
            sentence_id: sentence.id,
        };
        if let Some(audio) = self.cached(&key) {
            return Clip::Ready(Arc::new(audio));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let job = SpeechJob { key, request: SpeechRequest { text: text.to_string(), voice_id }, priority: SpeechPriority::Export };
        let cancel = self.deps.worker.submit_with(job, move |outcome| {
            let _ = sender.send(outcome);
        });
        Clip::Pending(PendingSpeech { receiver, cancel })
    }

    /// A cache that can't be read only costs a re-synthesis.
    fn cached(&self, key: &AudioKey) -> Option<Audio> {
        let bytes = match self.deps.cache.get(key) {
            Ok(bytes) => bytes?,
            Err(error) => {
                warn!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "cached clip unreadable, synthesizing it");
                return None;
            }
        };
        match self.deps.decoder.decode(&bytes) {
            Ok(audio) => Some(audio),
            Err(error) => {
                warn!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "cached clip undecodable, synthesizing it");
                None
            }
        }
    }
}

enum Received {
    Audio(Arc<Audio>),
    Silent,
    Stopped,
}

fn receive(clip: Clip, stop: &AtomicBool, state: &mut RunState) -> AppResult<Received> {
    match clip {
        Clip::Silent => {
            state.silent += 1;
            Ok(Received::Silent)
        }
        Clip::Ready(audio) => {
            state.cached += 1;
            Ok(Received::Audio(audio))
        }
        Clip::Pending(pending) => loop {
            match pending.receiver.recv_timeout(STOP_POLL) {
                Ok(outcome) => {
                    state.synthesized += 1;
                    return Ok(Received::Audio(outcome?));
                }
                Err(RecvTimeoutError::Timeout) if stop.load(Ordering::SeqCst) => return Ok(Received::Stopped),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(AppError::new(ErrorName::Internal).with_detail("the speech worker dropped an export sentence"));
                }
            }
        },
    }
}

/// Where a part starts, in output samples (what the earlier parts hold) and in 1x samples.
struct PartStart {
    output_samples: u64,
    source_samples: u64,
}

impl PartStart {
    /// Where the 1x sample `source_samples` lands in the output: the earlier parts' real length, plus this part's 1x
    /// audio so far ÷ speed (timed from the input, which avoids the stretcher's delay).
    fn mark_ms(&self, source_samples: u64, sample_rate: u32, speed: ExportSpeed) -> u64 {
        let in_part = source_samples.saturating_sub(self.source_samples) as f64 / f64::from(speed.as_factor());
        ((self.output_samples as f64 + in_part) * MILLIS_PER_SECOND / f64::from(sample_rate.max(1))).round() as u64
    }
}

/// The part being written, with its stretcher when the speed is baked in. Each call returns the samples it wrote.
struct PartWriter {
    writer: Box<dyn ExportWriter>,
    stretcher: Option<Box<dyn TimeStretcher>>,
    sample_rate: u32,
}

impl PartWriter {
    fn append(&mut self, audio: &Audio) -> AppResult<u64> {
        let Some(stretcher) = self.stretcher.as_mut() else {
            self.writer.append(audio)?;
            return Ok(audio.samples.len() as u64);
        };
        let mut stretched = Vec::with_capacity(audio.samples.len());
        stretcher.push(&audio.samples, &mut stretched);
        self.write(stretched)
    }

    fn write(&mut self, samples: Vec<f32>) -> AppResult<u64> {
        if samples.is_empty() {
            return Ok(0);
        }
        let written = samples.len() as u64;
        self.writer.append(&Audio { samples, sample_rate: self.sample_rate })?;
        Ok(written)
    }

    /// Drains the stretcher into the part, then finishes it. Returns the part and the samples the drain added.
    fn finish(mut self) -> AppResult<(ExportPart, u64)> {
        let mut flushed = 0;
        if let Some(mut stretcher) = self.stretcher.take() {
            let mut rest = Vec::new();
            stretcher.flush(&mut rest);
            flushed = self.write(rest)?;
        }
        Ok((self.writer.finish()?, flushed))
    }
}

/// The voice's display name ("Heart"), or its id if the engine doesn't know it.
pub(crate) fn voice_name(engine: &dyn SpeechEngine, voice_id: &VoiceId) -> String {
    engine.voices().into_iter().find(|voice| voice.id == *voice_id).map_or_else(|| voice_id.to_string(), |voice| voice.name)
}
