//! One export from start (or from its first unfinished part) to finish: sentences in playback order, cached clips
//! reused, the rest made by the speech worker at export priority and streamed straight into the part being written
//! (never into the cache). Finished parts are reported one by one, then joined into the book's file.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioDecoder, AudioExporter, AudioKey, ChapterMark, Document, DocumentReader, ErrorName, ExportPart, ExportRecord,
    ExportTarget, ExportWriter, ExportedFile, Sentence, SpeechEngine, SpeechRequest, VoiceId,
};
use tracing::{info, warn};

use super::jobs::ExportSettings;
use super::plan::{ExportPlan, ExportStep, PartPlan, export_plan, spoken_chars};
use super::progress::{SynthesisRate, checkpoint, percent};
use crate::speech::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechWorker, elapsed_ms};

const NAME: &str = "ExportRunner";
/// Sentences asked for ahead of the encoder, so the worker never idles while a clip is being encoded.
const PIPELINE_DEPTH: usize = 4;
/// How often a wait for speech checks whether the export was stopped.
const STOP_POLL: Duration = Duration::from_millis(200);
const MILLIS_PER_SECOND: u64 = 1000;

pub(crate) struct ExportRunnerDeps {
    pub worker: Arc<SpeechWorker>,
    pub engine: Arc<dyn SpeechEngine>,
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub decoder: Arc<dyn AudioDecoder>,
    pub exporter: Arc<dyn AudioExporter>,
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

/// What the runner tells the job registry while it works.
pub(crate) trait RunReport {
    fn progress(&mut self, progress: Progress);
    /// Saved before the next part starts, so a restart continues after this part.
    fn part_finished(&mut self, part: ExportPart, chapters: Vec<ChapterMark>);
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
    samples_written: u64,
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
        let plan = export_plan(&document, record.read_skipped, self.settings.max_sentences, self.settings.part_minutes);
        let first_part = record.parts.len();
        if plan.sentences != export.sentences_total || first_part > plan.parts.len() {
            let detail = format!("the document changed since export {} started ({} sentences, was {})", export.id, plan.sentences, export.sentences_total);
            return Err(AppError::new(ErrorName::Internal).with_detail(detail));
        }
        let chapters = plan.steps.len() - plan.sentences as usize;
        info!(service = NAME, export_id = %export.id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), read_skipped = record.read_skipped, sentences = plan.sentences, chapters, spoken_chars = plan.spoken_chars, parts_total = plan.parts.len(), first_part, "export started");

        let run = Run {
            record,
            document: &document,
            plan: &plan,
            target: ExportTarget {
                export_id: export.id.clone(),
                document_id: export.document_id.clone(),
                voice_id: export.voice_id.clone(),
                format: export.format,
                title: document.title.clone(),
                artist: voice_name(self.deps.engine.as_ref(), &export.voice_id),
            },
            sample_rate: self.deps.engine.sample_rate(),
        };
        let finished = &plan.parts[..first_part];
        let mut state = RunState {
            started: Instant::now(),
            samples_written: record.parts.iter().map(|part| part.samples).sum(),
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
            report.part_finished(part, marks);
        }

        if state.samples_written == 0 {
            return Err(AppError::new(ErrorName::Internal).with_detail("the export has no audio (no sentence to speak)"));
        }
        if stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
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
        let mut writer: Box<dyn ExportWriter> = self.deps.exporter.begin_part(&run.target, part_index, run.sample_rate)?;
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
                    marks.push(ChapterMark { title, start_ms: to_millis(state.samples_written, run.sample_rate) });
                    continue;
                }
                Prepared::Sentence { clip, chars } => (clip, chars),
            };
            let synthesized_before = state.synthesized;
            match receive(clip, stop, state)? {
                Received::Stopped => return Ok(None),
                Received::Silent => {}
                Received::Audio(audio) => {
                    writer.append(&audio)?;
                    state.samples_written += audio.samples.len() as u64;
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
                info!(service = NAME, export_id = %export.id, sentences_done = state.sentences_done, sentences_total = total, percent = percent(state.sentences_done, total), cached = state.cached, synthesized = state.synthesized, silent = state.silent, audio_seconds = state.samples_written as f64 / f64::from(run.sample_rate.max(1)), estimated_seconds_left = progress.estimated_seconds_left, elapsed_ms = elapsed_ms(state.started), "export progress");
            }
            report.progress(progress);
        }

        if stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok(Some((writer.finish()?, marks)))
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

fn to_millis(samples: u64, sample_rate: u32) -> u64 {
    samples * MILLIS_PER_SECOND / u64::from(sample_rate.max(1))
}

/// The voice's display name ("Heart"), or its id if the engine doesn't know it.
pub(crate) fn voice_name(engine: &dyn SpeechEngine, voice_id: &VoiceId) -> String {
    engine.voices().into_iter().find(|voice| voice.id == *voice_id).map_or_else(|| voice_id.to_string(), |voice| voice.name)
}
