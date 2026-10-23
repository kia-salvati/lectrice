//! Read-ahead (spec §4): after every audio request, queue the next sentences at read-ahead priority and
//! keep the cache window around the listener. One plan per (document, voice); a new position, rate or
//! `readSkipped` re-plans and cancels the queued jobs the new plan doesn't need.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioEncoder, AudioKey, AudioRetention, Document, DocumentId, ErrorName, SentenceId, SpeechRequest, VoiceId,
};
use tracing::{info, warn};

use super::read_ahead_plan::{ReadAheadInput, SECONDS_PER_MINUTE, plan_read_ahead};
use super::worker::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechWorker};

const NAME: &str = "ReadAhead";
/// Plans kept at once; the least recently used one is dropped (and its jobs cancelled) past this.
const MAX_PLANS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct ReadAheadSettings {
    pub read_ahead_minutes: u32,
    pub keep_minutes_per_document: u32,
}

impl ReadAheadSettings {
    /// The cache window must be this much larger than the read-ahead. The plan estimates audio length from
    /// the text (B5 measured real audio 6 % longer), and a window smaller than the real audio ahead would
    /// delete the farthest clips and make them again on every request (review L3).
    pub const MIN_KEEP_TO_READ_AHEAD_RATIO: f64 = 1.5;

    pub fn keeps_enough(&self) -> bool {
        f64::from(self.keep_minutes_per_document) >= f64::from(self.read_ahead_minutes) * Self::MIN_KEEP_TO_READ_AHEAD_RATIO
    }
}

/// Where the listener is: the sentence just requested, at what speed, with which order.
#[derive(Debug, Clone, Copy)]
pub struct ListenerPosition<'a> {
    pub document: &'a Document,
    /// The requested sentence's key (document, cleaner version, engine, voice, sentence).
    pub key: &'a AudioKey,
    pub rate: f32,
    pub read_skipped: bool,
}

/// What one call did, for the log and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadAheadOutcome {
    pub planned: usize,
    pub queued: usize,
    /// Planned sentences whose job from the previous plan is still queued or running.
    pub kept: usize,
    pub cached: usize,
    pub cancelled: usize,
    pub unchanged: bool,
}

pub struct ReadAhead {
    worker: Arc<SpeechWorker>,
    cache: Arc<dyn AudioCache>,
    retention: Arc<dyn AudioRetention>,
    encoder: Arc<dyn AudioEncoder>,
    settings: ReadAheadSettings,
    plans: Arc<Plans>,
}

type PlanKey = (DocumentId, VoiceId);

#[derive(Default)]
struct Plans {
    state: Mutex<PlansState>,
}

impl Plans {
    /// Callbacks and cancels never panic under this lock, so a poisoned lock still holds consistent plans.
    fn lock(&self) -> MutexGuard<'_, PlansState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Default)]
struct PlansState {
    by_key: HashMap<PlanKey, ActivePlan>,
    last_tick: u64,
}

impl PlansState {
    fn tick(&mut self) -> u64 {
        self.last_tick += 1;
        self.last_tick
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Anchor {
    cleaner_version: u32,
    engine_id: String,
    sentence_id: SentenceId,
    rate: f32,
    read_skipped: bool,
}

struct ActivePlan {
    anchor: Anchor,
    jobs: HashMap<SentenceId, QueuedJob>,
    last_used: u64,
}

struct QueuedJob {
    /// Lets a finished job's callback remove only its own entry, not a newer job for the same sentence.
    token: u64,
    cancel: CancelHandle,
}

impl ReadAhead {
    pub fn new(
        worker: Arc<SpeechWorker>,
        cache: Arc<dyn AudioCache>,
        retention: Arc<dyn AudioRetention>,
        encoder: Arc<dyn AudioEncoder>,
        settings: ReadAheadSettings,
    ) -> AppResult<Self> {
        if !settings.keeps_enough() {
            let detail = format!(
                "keep_minutes_per_document {} must be at least {} x read_ahead_minutes {}",
                settings.keep_minutes_per_document,
                ReadAheadSettings::MIN_KEEP_TO_READ_AHEAD_RATIO,
                settings.read_ahead_minutes
            );
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail(detail));
        }
        Ok(Self { worker, cache, retention, encoder, settings, plans: Arc::default() })
    }

    /// Trims the cache around the listener, then (re-)plans and queues the read-ahead. Never fails the
    /// caller: errors are logged, because playback of the requested sentence doesn't depend on them.
    pub fn follow(&self, position: ListenerPosition<'_>) -> ReadAheadOutcome {
        self.trim_cache(&position);
        self.plan(&position)
    }

    fn trim_cache(&self, position: &ListenerPosition<'_>) {
        let key = position.key;
        let keep_seconds = f64::from(self.settings.keep_minutes_per_document) * SECONDS_PER_MINUTE * f64::from(position.rate);
        match self.retention.retain_around(key, keep_seconds) {
            Ok(eviction) if eviction.evicted_clips() > 0 => info!(
                service = NAME,
                doc_id = %key.document_id,
                voice_id = %key.voice_id,
                sentence_id = key.sentence_id.0,
                keep_seconds,
                window_clips = eviction.window_clips,
                cap_clips = eviction.cap_clips,
                freed_bytes = eviction.freed_bytes,
                kept_seconds = eviction.kept_seconds,
                cache_bytes = eviction.cache_bytes,
                "audio cache evicted"
            ),
            Ok(_) => {}
            Err(error) => warn!(service = NAME, doc_id = %key.document_id, error = %error, "audio cache eviction failed"),
        }
    }

    fn plan(&self, position: &ListenerPosition<'_>) -> ReadAheadOutcome {
        let key = position.key;
        let anchor = Anchor {
            cleaner_version: key.cleaner_version,
            engine_id: key.engine_id.clone(),
            sentence_id: key.sentence_id,
            rate: position.rate,
            read_skipped: position.read_skipped,
        };
        let plan_key = (key.document_id.clone(), key.voice_id.clone());

        let mut state = self.plans.lock();
        let tick = state.tick();
        if let Some(active) = state.by_key.get_mut(&plan_key)
            && active.anchor == anchor
        {
            active.last_used = tick;
            return ReadAheadOutcome { unchanged: true, ..ReadAheadOutcome::default() };
        }

        let input = ReadAheadInput {
            document: position.document,
            from: key.sentence_id,
            rate: position.rate,
            read_skipped: position.read_skipped,
            minutes: self.settings.read_ahead_minutes,
        };
        let plan = plan_read_ahead(input);
        let wanted: HashSet<SentenceId> = plan.sentences.iter().map(|sentence| sentence.id).collect();
        let previous = state.by_key.remove(&plan_key);
        let mut outcome = ReadAheadOutcome { planned: plan.sentences.len(), ..ReadAheadOutcome::default() };

        let (mut jobs, cancelled) = previous.map_or_else(|| (HashMap::new(), 0), |previous| carry_over(previous, &anchor, &wanted));
        outcome.cancelled = cancelled;

        for sentence in &plan.sentences {
            if jobs.contains_key(&sentence.id) {
                outcome.kept += 1;
                continue;
            }
            let sentence_key = AudioKey { sentence_id: sentence.id, ..key.clone() };
            match self.retention.contains(&sentence_key) {
                Ok(true) => {
                    outcome.cached += 1;
                    continue;
                }
                Ok(false) => {}
                Err(error) => warn!(service = NAME, doc_id = %key.document_id, sentence_id = sentence.id.0, error = %error, "audio cache lookup failed"),
            }
            let token = state.tick();
            let cancel = self.queue(sentence_key, sentence.spoken_text(), plan_key.clone(), token);
            jobs.insert(sentence.id, QueuedJob { token, cancel });
            outcome.queued += 1;
        }

        state.by_key.insert(plan_key, ActivePlan { anchor, jobs, last_used: tick });
        outcome.cancelled += drop_least_recent_plans(&mut state);
        drop(state);

        info!(
            service = NAME,
            doc_id = %key.document_id,
            voice_id = %key.voice_id,
            sentence_id = key.sentence_id.0,
            rate = position.rate,
            read_skipped = position.read_skipped,
            planned = outcome.planned,
            planned_seconds = plan.estimated_seconds,
            queued = outcome.queued,
            kept = outcome.kept,
            cached = outcome.cached,
            cancelled = outcome.cancelled,
            "read-ahead planned"
        );
        outcome
    }

    /// Submits one read-ahead job; its callback (on the worker thread) encodes and caches the audio.
    fn queue(&self, key: AudioKey, text: &str, plan_key: PlanKey, token: u64) -> CancelHandle {
        let request = SpeechRequest { text: text.to_string(), voice_id: key.voice_id.clone() };
        let job = SpeechJob { key: key.clone(), request, priority: SpeechPriority::ReadAhead };
        let cache = self.cache.clone();
        let encoder = self.encoder.clone();
        let plans = self.plans.clone();
        self.worker.submit_with(job, move |outcome| {
            store_clip(&key, outcome, cache.as_ref(), encoder.as_ref());
            forget_job(&plans, &plan_key, key.sentence_id, token);
        })
    }
}

fn store_clip(key: &AudioKey, outcome: SpeechOutcome, cache: &dyn AudioCache, encoder: &dyn AudioEncoder) {
    let stored = outcome.and_then(|audio| encoder.encode(&audio)).and_then(|bytes| cache.put(key, &bytes).map(|()| bytes.len()));
    match stored {
        Ok(bytes) => {
            info!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, voice_id = %key.voice_id, bytes, "read-ahead audio cached")
        }
        Err(error) => warn!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "read-ahead audio failed"),
    }
}

/// Keeps the previous plan's jobs that the new plan still wants and cancels the rest; returns the kept jobs
/// and how many were cancelled. A job that is already running only loses its result when cancelled.
fn carry_over(previous: ActivePlan, anchor: &Anchor, wanted: &HashSet<SentenceId>) -> (HashMap<SentenceId, QueuedJob>, usize) {
    let same_audio = previous.anchor.cleaner_version == anchor.cleaner_version && previous.anchor.engine_id == anchor.engine_id;
    let mut kept = HashMap::new();
    let mut cancelled = 0;
    for (sentence_id, job) in previous.jobs {
        if same_audio && wanted.contains(&sentence_id) {
            kept.insert(sentence_id, job);
        } else if job.cancel.cancel() {
            cancelled += 1;
        }
    }
    (kept, cancelled)
}

fn forget_job(plans: &Plans, plan_key: &PlanKey, sentence_id: SentenceId, token: u64) {
    let mut state = plans.lock();
    if let Some(active) = state.by_key.get_mut(plan_key)
        && active.jobs.get(&sentence_id).is_some_and(|job| job.token == token)
    {
        active.jobs.remove(&sentence_id);
    }
}

/// Keeps at most [`MAX_PLANS`] plans; returns how many queued jobs were cancelled.
fn drop_least_recent_plans(state: &mut PlansState) -> usize {
    let mut cancelled = 0;
    while state.by_key.len() > MAX_PLANS {
        let Some(oldest) = state.by_key.iter().min_by_key(|(_, plan)| plan.last_used).map(|(key, _)| key.clone()) else {
            break;
        };
        if let Some(plan) = state.by_key.remove(&oldest) {
            cancelled += plan.jobs.values().filter(|job| job.cancel.cancel()).count();
        }
    }
    cancelled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::fakes::{CountingEncoder, GatedEngine, MemoryCache};
    use lectrice_domain::{Block, BlockKind, Sentence};
    use std::time::{Duration, Instant};

    const WAIT_LIMIT: Duration = Duration::from_secs(5);
    /// A 4-digit id + 14 chars = 18 chars = 1 s at 1x, so each sentence is one second of estimated audio.
    const SENTENCE_TAIL: &str = "abcdefghijklm.";

    fn document(sentences: u32) -> Document {
        let blocks = (0..sentences)
            .map(|id| Block {
                id,
                kind: BlockKind::Paragraph,
                level: None,
                page_index: 1,
                spoken: true,
                text: None,
                sentences: vec![Sentence { id: SentenceId(id), text: format!("{id:04}{SENTENCE_TAIL}"), page_index: 1, speech_text: None }],
                cue: None,
            })
            .collect();
        Document {
            id: DocumentId::parse(&format!("{:064x}", 1)).unwrap(),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: Vec::new(),
            toc: Vec::new(),
            blocks,
        }
    }

    fn key(document: &Document, sentence: u32, voice: &str) -> AudioKey {
        AudioKey {
            document_id: document.id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: "fake-engine".into(),
            voice_id: VoiceId::new(voice),
            sentence_id: SentenceId(sentence),
        }
    }

    struct Setup {
        engine: Arc<GatedEngine>,
        cache: Arc<MemoryCache>,
        read_ahead: ReadAhead,
    }

    /// The engine blocks on `gate`, so jobs queued behind it stay queued until `release`.
    fn setup(gate: &str, read_ahead_minutes: u32) -> Setup {
        let engine = Arc::new(GatedEngine::new(gate));
        let cache = Arc::new(MemoryCache::default());
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
        let settings = ReadAheadSettings { read_ahead_minutes, keep_minutes_per_document: 60 };
        let read_ahead = ReadAhead::new(worker, cache.clone(), cache.clone(), Arc::new(CountingEncoder), settings).unwrap();
        Setup { engine, cache, read_ahead }
    }

    fn follow(setup: &Setup, document: &Document, sentence: u32, rate: f32) -> ReadAheadOutcome {
        let key = key(document, sentence, "af_heart");
        setup.read_ahead.follow(ListenerPosition { document, key: &key, rate, read_skipped: false })
    }

    fn wait_for_puts(cache: &MemoryCache, puts: usize) {
        let started = Instant::now();
        while cache.puts() < puts {
            assert!(started.elapsed() < WAIT_LIMIT, "expected {puts} cached clips, got {}", cache.puts());
            std::thread::yield_now();
        }
    }

    fn spoken_ids(engine: &GatedEngine) -> Vec<u32> {
        engine.spoken().iter().map(|text| text[..4].parse().unwrap()).collect()
    }

    #[test]
    fn the_plan_is_queued_encoded_and_cached() {
        let book = document(200);
        let setup = setup("never", 1);

        let outcome = follow(&setup, &book, 0, 1.0);

        assert_eq!((outcome.planned, outcome.queued, outcome.cancelled), (60, 60, 0));
        wait_for_puts(&setup.cache, 60);
        assert_eq!(spoken_ids(&setup.engine), (1..=60).collect::<Vec<_>>());
    }

    #[test]
    fn at_2x_twice_as_much_is_queued() {
        let book = document(400);
        let setup = setup("never", 1);

        assert_eq!(follow(&setup, &book, 0, 2.0).queued, 120);
    }

    #[test]
    fn cached_sentences_are_skipped() {
        let book = document(200);
        let setup = setup("never", 1);
        for sentence in [1, 2, 5] {
            setup.cache.put(&key(&book, sentence, "af_heart"), b"x").unwrap();
        }

        let outcome = follow(&setup, &book, 0, 1.0);

        assert_eq!((outcome.planned, outcome.cached, outcome.queued), (60, 3, 57));
        wait_for_puts(&setup.cache, 60);
        assert!(!spoken_ids(&setup.engine).iter().any(|id| [1, 2, 5].contains(id)));
    }

    #[test]
    fn the_same_request_again_does_not_re_plan() {
        let book = document(200);
        let setup = setup(&book.blocks[1].sentences[0].text, 1);

        follow(&setup, &book, 0, 1.0);
        let again = follow(&setup, &book, 0, 1.0);

        assert!(again.unchanged);
        setup.engine.release();
    }

    #[test]
    fn a_jump_cancels_the_stale_queued_jobs() {
        let book = document(400);
        let setup = setup(&book.blocks[1].sentences[0].text, 1);
        follow(&setup, &book, 0, 1.0);
        setup.engine.wait_until_blocked();

        let jumped = follow(&setup, &book, 300, 1.0);

        // Sentence 1 is already running: it can't be stopped, only its result is dropped. The other 59 never run.
        assert_eq!((jumped.cancelled, jumped.queued), (60, 60));
        setup.engine.release();
        wait_for_puts(&setup.cache, 60);
        let spoken = spoken_ids(&setup.engine);
        assert_eq!(spoken[0], 1);
        assert_eq!(spoken[1..], (301..=360).collect::<Vec<_>>());
        assert!(!setup.cache.keys().iter().any(|key| key.sentence_id.0 < 300));
    }

    #[test]
    fn a_small_step_keeps_the_overlap_and_a_speed_change_re_plans() {
        let book = document(400);
        let setup = setup(&book.blocks[1].sentences[0].text, 1);
        follow(&setup, &book, 0, 1.0);
        setup.engine.wait_until_blocked();

        let stepped = follow(&setup, &book, 1, 1.0);
        assert_eq!((stepped.kept, stepped.queued, stepped.cancelled), (59, 1, 1));

        let faster = follow(&setup, &book, 1, 2.0);
        assert_eq!((faster.kept, faster.queued, faster.cancelled), (60, 60, 0));

        let slower = follow(&setup, &book, 1, 1.0);
        assert_eq!((slower.kept, slower.queued, slower.cancelled), (60, 0, 60));
        setup.engine.release();
    }

    #[test]
    fn voices_have_separate_plans() {
        let book = document(400);
        let setup = setup(&book.blocks[1].sentences[0].text, 1);
        follow(&setup, &book, 0, 1.0);
        setup.engine.wait_until_blocked();

        let other_key = key(&book, 0, "bm_lewis");
        let other = setup.read_ahead.follow(ListenerPosition { document: &book, key: &other_key, rate: 1.0, read_skipped: false });

        assert_eq!((other.queued, other.cancelled), (60, 0));
        assert!(follow(&setup, &book, 0, 1.0).unchanged);
        setup.engine.release();
    }

    #[test]
    fn concurrent_requests_leave_one_consistent_plan() {
        let book = Arc::new(document(2000));
        let setup = Arc::new(setup("never", 1));
        let threads: Vec<_> = (0..8)
            .map(|thread| {
                let (book, setup) = (book.clone(), setup.clone());
                std::thread::spawn(move || {
                    for step in 0..20 {
                        follow(&setup, &book, thread * 200 + step, 1.0);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }

        let last = follow(&setup, &book, 1900, 1.0);
        assert_eq!(last.planned, 60);
        assert_eq!(last.kept + last.queued + last.cached, 60);
        let started = Instant::now();
        while setup.read_ahead.plans.lock().by_key.values().any(|plan| !plan.jobs.is_empty()) {
            assert!(started.elapsed() < WAIT_LIMIT, "read-ahead jobs never finished");
            std::thread::yield_now();
        }
        assert_eq!(setup.read_ahead.plans.lock().by_key.len(), 1);
        let cached: HashSet<u32> = setup.cache.keys().iter().map(|key| key.sentence_id.0).collect();
        assert!((1901..=1960).all(|sentence| cached.contains(&sentence)));
    }
}
