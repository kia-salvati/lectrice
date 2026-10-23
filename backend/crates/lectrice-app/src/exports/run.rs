//! One export from start to finish: sentences in playback order, cached clips reused, the rest made by the speech
//! worker at export priority and streamed straight into the encoder (never into the cache).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioDecoder, AudioExport, AudioExporter, AudioKey, Document, DocumentReader, ErrorName, ExportTarget,
    ExportWriter, ExportedFile, Sentence, SpeechEngine, SpeechRequest, VoiceId,
};
use tracing::{info, warn};

use super::plan::{ExportStep, export_plan, spoken_chars};
use super::progress::{SynthesisRate, checkpoint, percent};
use crate::speech::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechWorker, audio_seconds, elapsed_ms};

const NAME: &str = "ExportRunner";
/// Sentences asked for ahead of the encoder, so the worker never idles while a clip is being encoded.
const PIPELINE_DEPTH: usize = 4;
/// How often a wait for speech checks whether the export was cancelled.
const CANCEL_POLL: Duration = Duration::from_millis(200);

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
    /// Exports only the first sentences when set (a short export for testing).
    max_sentences: Option<u32>,
}

pub(crate) enum RunOutcome {
    Done(ExportedFile),
    Cancelled,
    Failed(AppError),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Progress {
    pub sentences_done: u32,
    pub estimated_seconds_left: Option<u32>,
    /// Another 5 % was reached: worth saving.
    pub checkpoint: bool,
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

struct Counts {
    cached: u32,
    synthesized: u32,
    silent: u32,
    audio_seconds: f64,
}

impl ExportRunner {
    pub fn new(deps: ExportRunnerDeps, max_sentences: Option<u32>) -> Self {
        Self { deps, max_sentences }
    }

    pub fn run(&self, export: &AudioExport, read_skipped: bool, cancel: &AtomicBool, report: &mut dyn FnMut(Progress)) -> RunOutcome {
        match self.try_run(export, read_skipped, cancel, report) {
            Ok(Some(file)) => RunOutcome::Done(file),
            Ok(None) => RunOutcome::Cancelled,
            Err(error) => RunOutcome::Failed(error),
        }
    }

    /// `Ok(None)` when cancelled.
    fn try_run(&self, export: &AudioExport, read_skipped: bool, cancel: &AtomicBool, report: &mut dyn FnMut(Progress)) -> AppResult<Option<ExportedFile>> {
        let started = Instant::now();
        let document = self.deps.reader.find(&export.document_id)?.ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        let plan = export_plan(&document, read_skipped, self.max_sentences);
        let chapters = plan.steps.len() - plan.sentences as usize;
        info!(service = NAME, export_id = %export.id, doc_id = %export.document_id, voice_id = %export.voice_id, format = export.format.extension(), read_skipped, sentences = plan.sentences, chapters, spoken_chars = plan.spoken_chars, "export started");

        let target = ExportTarget {
            document_id: export.document_id.clone(),
            voice_id: export.voice_id.clone(),
            format: export.format,
            title: document.title.clone(),
            artist: voice_name(self.deps.engine.as_ref(), &export.voice_id),
        };
        let mut writer: Option<Box<dyn ExportWriter>> = None;
        let mut chapters_before_audio = Vec::new();
        let mut window: VecDeque<Prepared> = VecDeque::new();
        let mut steps = plan.steps.iter();
        let mut counts = Counts { cached: 0, synthesized: 0, silent: 0, audio_seconds: 0.0 };
        let (mut sentences_done, mut done_chars, mut last_checkpoint) = (0, 0, 0);
        let mut rate = SynthesisRate::default();
        let mut last_sentence_end = Instant::now();

        loop {
            while window.len() < PIPELINE_DEPTH {
                let Some(step) = steps.next() else { break };
                window.push_back(self.prepare(&document, export, step));
            }
            let Some(prepared) = window.pop_front() else { break };
            if cancel.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let (clip, chars) = match prepared {
                Prepared::Chapter(title) => {
                    match writer.as_mut() {
                        Some(writer) => writer.start_chapter(&title)?,
                        None => chapters_before_audio.push(title),
                    }
                    continue;
                }
                Prepared::Sentence { clip, chars } => (clip, chars),
            };
            let synthesized_before = counts.synthesized;
            match receive(clip, cancel, &mut counts)? {
                Received::Cancelled => return Ok(None),
                Received::Silent => {}
                Received::Audio(audio) => {
                    counts.audio_seconds += audio_seconds(&audio);
                    let writer = match writer.as_mut() {
                        Some(writer) => writer,
                        None => writer.insert(self.begin(&target, audio.sample_rate, &mut chapters_before_audio)?),
                    };
                    writer.append(&audio)?;
                }
            }

            sentences_done += 1;
            done_chars += chars;
            // With the pipeline full, the time between two sentences coming out is what the engine took for one.
            let sentence_end = Instant::now();
            if counts.synthesized > synthesized_before {
                rate.record(chars, sentence_end - last_sentence_end);
            }
            last_sentence_end = sentence_end;
            let reached = checkpoint(sentences_done, plan.sentences);
            let progress = Progress {
                sentences_done,
                estimated_seconds_left: rate.seconds_left(plan.spoken_chars.saturating_sub(done_chars)),
                checkpoint: reached > last_checkpoint,
            };
            if progress.checkpoint {
                last_checkpoint = reached;
                info!(service = NAME, export_id = %export.id, sentences_done, sentences_total = plan.sentences, percent = percent(sentences_done, plan.sentences), cached = counts.cached, synthesized = counts.synthesized, silent = counts.silent, audio_seconds = counts.audio_seconds, estimated_seconds_left = progress.estimated_seconds_left, elapsed_ms = elapsed_ms(started), "export progress");
            }
            report(progress);
        }

        let Some(writer) = writer else {
            return Err(AppError::new(ErrorName::Internal).with_detail("the export has no audio (no sentence to speak)"));
        };
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let file = writer.finish()?;
        info!(service = NAME, export_id = %export.id, path = %file.path.display(), bytes = file.size_bytes, audio_seconds = counts.audio_seconds, cached = counts.cached, synthesized = counts.synthesized, elapsed_ms = elapsed_ms(started), "export file written");
        Ok(Some(file))
    }

    fn begin(&self, target: &ExportTarget, sample_rate: u32, chapters: &mut Vec<String>) -> AppResult<Box<dyn ExportWriter>> {
        let mut writer = self.deps.exporter.begin(target, sample_rate)?;
        for title in chapters.drain(..) {
            writer.start_chapter(&title)?;
        }
        Ok(writer)
    }

    fn prepare(&self, document: &Document, export: &AudioExport, step: &ExportStep<'_>) -> Prepared {
        match step {
            ExportStep::Chapter(title) => Prepared::Chapter(title.clone()),
            ExportStep::Sentence(sentence) => Prepared::Sentence { clip: self.clip(document, export, sentence), chars: spoken_chars(sentence) },
        }
    }

    /// A cached clip, or a request to the speech worker. Sentences with nothing to say (e.g. only a URL) add no audio.
    fn clip(&self, document: &Document, export: &AudioExport, sentence: &Sentence) -> Clip {
        let text = sentence.spoken_text().trim();
        if text.is_empty() {
            return Clip::Silent;
        }
        let key = AudioKey {
            document_id: export.document_id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: self.deps.engine.id().to_string(),
            voice_id: export.voice_id.clone(),
            sentence_id: sentence.id,
        };
        if let Some(audio) = self.cached(&key) {
            return Clip::Ready(Arc::new(audio));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let job = SpeechJob { key, request: SpeechRequest { text: text.to_string(), voice_id: export.voice_id.clone() }, priority: SpeechPriority::Export };
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
    Cancelled,
}

fn receive(clip: Clip, cancel: &AtomicBool, counts: &mut Counts) -> AppResult<Received> {
    match clip {
        Clip::Silent => {
            counts.silent += 1;
            Ok(Received::Silent)
        }
        Clip::Ready(audio) => {
            counts.cached += 1;
            Ok(Received::Audio(audio))
        }
        Clip::Pending(pending) => loop {
            match pending.receiver.recv_timeout(CANCEL_POLL) {
                Ok(outcome) => {
                    counts.synthesized += 1;
                    return Ok(Received::Audio(outcome?));
                }
                Err(RecvTimeoutError::Timeout) if cancel.load(Ordering::SeqCst) => return Ok(Received::Cancelled),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(AppError::new(ErrorName::Internal).with_detail("the speech worker dropped an export sentence"));
                }
            }
        },
    }
}

/// The voice's display name ("Heart"), or its id if the engine doesn't know it.
pub(crate) fn voice_name(engine: &dyn SpeechEngine, voice_id: &VoiceId) -> String {
    engine.voices().into_iter().find(|voice| voice.id == *voice_id).map_or_else(|| voice_id.to_string(), |voice| voice.name)
}
