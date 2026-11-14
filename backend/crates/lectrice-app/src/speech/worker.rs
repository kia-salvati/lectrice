//! The speech worker (spec §4): one thread owns synthesis and takes jobs from a priority queue.
//!
//! - Priorities: [`SpeechPriority::Now`] > [`SpeechPriority::ReadAhead`] > [`SpeechPriority::Export`]. `Now` jobs run
//!   lowest sentence first (a player asks for the next few at once and waits for the first); the others run in
//!   submission order.
//! - While nobody listens (no sentence was asked for within [`LISTENER_IDLE_AFTER`]), exports go before
//!   read-ahead, and read-ahead asks the [`SynthesisGate`] for its turn at export priority, so a paused player
//!   holds up neither an export nor a phone's audio.
//! - Jobs with the same [`AudioKey`] that are queued or running are merged: the engine runs once and
//!   every waiter gets the same audio. A merge with a higher priority moves the queued job up.
//! - A job that is already running can't be stopped (the engine call is synchronous); cancelling it only
//!   drops that waiter's result. Cancelling one waiter of a queued job lowers the job to the highest
//!   priority its remaining waiters asked for.
//! - A panic in a waiter's callback is caught and logged, so the worker thread keeps running.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use lectrice_domain::{AppError, AppResult, Audio, AudioKey, ErrorName, SpeechEngine, SpeechRequest};
use tracing::{info, warn};

use crate::Cancellation;

const NAME: &str = "SpeechWorker";
const THREAD_NAME: &str = "speech-worker";
/// How often a waiting caller checks whether it was cancelled.
const CANCEL_POLL: Duration = Duration::from_millis(100);
/// Synthesized jobs kept for the real-time factor (spec 19, C17).
const RECENT_TIMINGS: usize = 50;
/// Fewer measured jobs give no real-time factor.
const MIN_TIMINGS_FOR_RTF: usize = 10;
const MILLIS_PER_SECOND: f64 = 1000.0;
/// A listener who asked for no sentence for this long has paused (longer than almost any sentence plays).
pub const LISTENER_IDLE_AFTER: Duration = Duration::from_secs(60);

/// Declared lowest first, so the derived order makes `Now` the greatest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpeechPriority {
    Export,
    ReadAhead,
    Now,
}

const PRIORITIES: usize = 3;
const LISTENING_ORDER: [SpeechPriority; PRIORITIES] = [SpeechPriority::Now, SpeechPriority::ReadAhead, SpeechPriority::Export];
const IDLE_ORDER: [SpeechPriority; PRIORITIES] = [SpeechPriority::Now, SpeechPriority::Export, SpeechPriority::ReadAhead];

#[derive(Debug, Clone)]
pub struct SpeechJob {
    pub key: AudioKey,
    pub request: SpeechRequest,
    pub priority: SpeechPriority,
}

/// Shared by every waiter of a merged job.
pub type SpeechOutcome = AppResult<Arc<Audio>>;

/// One synthesis at a time on the machine, shared with other users of the CPU (the desktop's helper server makes
/// phones' items through the same gate; file 24 C48). The worker asks for a turn with the job's priority, so the
/// shell can rank its own `Now`, read-ahead and export work against the phones' items.
pub trait SynthesisGate: Send + Sync {
    /// Runs `synthesize` once it is this priority's turn; the turn ends when it returns.
    fn with_turn(&self, priority: SpeechPriority, synthesize: &mut dyn FnMut());
}

type OnDone = Box<dyn FnOnce(SpeechOutcome) + Send>;

/// Owns the synthesis thread. Dropping it stops the thread after the running job; queued waiters
/// are dropped without a result.
pub struct SpeechWorker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl SpeechWorker {
    pub fn start(engine: Arc<dyn SpeechEngine>) -> AppResult<Self> {
        Self::start_gated(engine, None)
    }

    /// Like [`Self::start`], with every synthesis waiting for its turn at `gate`.
    pub fn start_gated(engine: Arc<dyn SpeechEngine>, gate: Option<Arc<dyn SynthesisGate>>) -> AppResult<Self> {
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || run_loop(&thread_shared, engine.as_ref(), gate.as_deref()))
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the speech worker thread").with_source(error))?;
        Ok(Self { shared, thread: Some(thread) })
    }

    /// Queues a job and returns a ticket to wait for its audio.
    pub fn submit(&self, job: SpeechJob) -> SpeechTicket {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = self.submit_with(job, move |outcome| {
            let _ = sender.send(outcome);
        });
        SpeechTicket { receiver, cancel }
    }

    /// Queues a job and calls `on_done` with its audio. `on_done` runs on the worker thread, so it must be
    /// short and must never wait for another speech job. It is never called if the job is cancelled.
    pub fn submit_with(&self, job: SpeechJob, on_done: impl FnOnce(SpeechOutcome) + Send + 'static) -> CancelHandle {
        let SpeechJob { key, request, priority } = job;
        let mut queue = self.shared.lock();
        let waiter_id = queue.next_id();
        let waiter = Waiter { id: waiter_id, priority, on_done: Box::new(on_done) };

        if let Some(pending) = queue.jobs.get_mut(&key) {
            pending.waiters.push(waiter);
            if !pending.running && priority > pending.priority {
                pending.priority = priority;
                let entry_id = queue.next_id();
                queue.requeue(&key, priority, entry_id);
            }
        } else {
            let entry_id = queue.next_id();
            queue.jobs.insert(key.clone(), PendingJob { request, priority, entry_id, running: false, waiters: vec![waiter] });
            queue.push(QueueEntry { priority, entry_id, key: key.clone() });
        }
        drop(queue);
        self.shared.wake.notify_one();
        CancelHandle { shared: self.shared.clone(), key, waiter_id }
    }
}

impl Drop for SpeechWorker {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Waiters hold the reply channels; dropping them ends every pending `SpeechTicket::wait`.
        let mut queue = self.shared.lock();
        queue.queues.iter_mut().for_each(BinaryHeap::clear);
        let abandoned = std::mem::take(&mut queue.jobs);
        drop(queue);
        drop(abandoned);
    }
}

/// Waits for one submitted job.
pub struct SpeechTicket {
    receiver: Receiver<SpeechOutcome>,
    cancel: CancelHandle,
}

impl SpeechTicket {
    /// Blocks until the audio is ready. A cancelled job, or a worker that stopped, gives `Internal`.
    pub fn wait(self) -> SpeechOutcome {
        self.receiver.recv().unwrap_or_else(|_| Err(not_delivered()))
    }

    /// Like [`Self::wait`], but withdraws this waiter and gives `RequestCancelled` once `cancellation` is set,
    /// so a request nobody waits for any more doesn't hold its place in the queue.
    pub fn wait_unless_cancelled(self, cancellation: &Cancellation) -> SpeechOutcome {
        loop {
            match self.receiver.recv_timeout(CANCEL_POLL) {
                Ok(outcome) => return outcome,
                Err(RecvTimeoutError::Disconnected) => return Err(not_delivered()),
                Err(RecvTimeoutError::Timeout) if cancellation.is_cancelled() => {
                    self.cancel.cancel();
                    return Err(AppError::new(ErrorName::RequestCancelled).with_detail("the caller stopped waiting for speech"));
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    /// The outcome if it arrives within `timeout`; `None` means it is still coming.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<SpeechOutcome> {
        match self.receiver.recv_timeout(timeout) {
            Ok(outcome) => Some(outcome),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(Err(not_delivered())),
        }
    }

    pub fn cancel_handle(&self) -> CancelHandle {
        self.cancel.clone()
    }
}

/// Withdraws one waiter. The job itself is dropped once it has no waiters left and hasn't started.
#[derive(Clone)]
pub struct CancelHandle {
    shared: Arc<Shared>,
    key: AudioKey,
    waiter_id: u64,
}

impl CancelHandle {
    /// True if the waiter was withdrawn before its result was delivered.
    pub fn cancel(&self) -> bool {
        self.withdraw(true)
    }

    /// Like [`Self::cancel`], but leaves a job that is already running alone, so its result is still delivered.
    pub fn cancel_if_queued(&self) -> bool {
        self.withdraw(false)
    }

    fn withdraw(&self, even_if_running: bool) -> bool {
        let mut queue = self.shared.lock();
        let Some(pending) = queue.jobs.get_mut(&self.key) else {
            return false;
        };
        if pending.running && !even_if_running {
            return false;
        }
        let before = pending.waiters.len();
        pending.waiters.retain(|waiter| waiter.id != self.waiter_id);
        let withdrawn = pending.waiters.len() < before;
        if pending.running {
            return withdrawn;
        }
        match pending.waiters.iter().map(|waiter| waiter.priority).max() {
            None => {
                queue.jobs.remove(&self.key);
            }
            Some(highest) if highest < pending.priority => {
                pending.priority = highest;
                let entry_id = queue.next_id();
                queue.requeue(&self.key, highest, entry_id);
            }
            Some(_) => {}
        }
        withdrawn
    }
}

#[derive(Default)]
struct Shared {
    queue: Mutex<QueueState>,
    wake: Condvar,
    recent: Mutex<VecDeque<Timing>>,
    /// When a listener last asked for a sentence; `None` (never told) keeps the listening order.
    last_listener: Mutex<Option<Instant>>,
}

impl Shared {
    /// Waiter callbacks never run under the lock, so a poisoned lock still holds a consistent queue.
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn recent(&self) -> MutexGuard<'_, VecDeque<Timing>> {
        self.recent.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn listening(&self) -> bool {
        self.last_listener.lock().unwrap_or_else(PoisonError::into_inner).is_none_or(|at| at.elapsed() < LISTENER_IDLE_AFTER)
    }

    fn record(&self, timing: Timing) {
        let mut recent = self.recent();
        if recent.len() == RECENT_TIMINGS {
            recent.pop_front();
        }
        recent.push_back(timing);
    }
}

/// One synthesized job: how much audio it made and how long that took.
#[derive(Debug, Clone, Copy)]
struct Timing {
    audio_ms: u64,
    synth_ms: u64,
}

/// The real-time factor (synthesis time ÷ audio length) over the last synthesized jobs. Cache hits are not jobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynthesisTimings {
    /// `None` until enough jobs were measured to be meaningful.
    pub rtf_median: Option<f32>,
    pub samples: u32,
}

impl SpeechWorker {
    /// A listener asked for a sentence (cached or not): read-ahead goes before exports for a while.
    pub fn note_listener(&self) {
        *self.shared.last_listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    pub fn timings(&self) -> SynthesisTimings {
        let recent = self.shared.recent();
        let mut factors: Vec<f32> = recent.iter().filter(|timing| timing.audio_ms > 0).map(|timing| timing.synth_ms as f32 / timing.audio_ms as f32).collect();
        let samples = u32::try_from(factors.len()).unwrap_or(u32::MAX);
        if factors.len() < MIN_TIMINGS_FOR_RTF {
            return SynthesisTimings { rtf_median: None, samples };
        }
        factors.sort_by(f32::total_cmp);
        let middle = factors.len() / 2;
        let median = if factors.len().is_multiple_of(2) { (factors[middle - 1] + factors[middle]) / 2.0 } else { factors[middle] };
        SynthesisTimings { rtf_median: Some(median), samples }
    }
}

#[derive(Default)]
struct QueueState {
    jobs: HashMap<AudioKey, PendingJob>,
    /// One queue per priority, lowest first. They may hold stale entries (cancelled or re-prioritized jobs); those
    /// are skipped when popped.
    queues: [BinaryHeap<QueueEntry>; PRIORITIES],
    last_id: u64,
    shutdown: bool,
}

impl QueueState {
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    fn requeue(&mut self, key: &AudioKey, priority: SpeechPriority, entry_id: u64) {
        if let Some(pending) = self.jobs.get_mut(key) {
            pending.entry_id = entry_id;
            self.push(QueueEntry { priority, entry_id, key: key.clone() });
        }
    }

    fn push(&mut self, entry: QueueEntry) {
        self.queues[entry.priority as usize].push(entry);
    }

    /// The next job to run, marked running, with the priority it asks the gate for its turn at.
    fn take_next(&mut self, listening: bool) -> Option<(AudioKey, SpeechRequest, SpeechPriority)> {
        let order = if listening { LISTENING_ORDER } else { IDLE_ORDER };
        for priority in order {
            while let Some(entry) = self.queues[priority as usize].pop() {
                let Some(pending) = self.jobs.get_mut(&entry.key) else {
                    continue;
                };
                if pending.entry_id != entry.entry_id || pending.running {
                    continue;
                }
                pending.running = true;
                let turn = if !listening && pending.priority == SpeechPriority::ReadAhead { SpeechPriority::Export } else { pending.priority };
                return Some((entry.key, pending.request.clone(), turn));
            }
        }
        None
    }
}

struct PendingJob {
    request: SpeechRequest,
    priority: SpeechPriority,
    /// The heap entry that is current for this job.
    entry_id: u64,
    running: bool,
    waiters: Vec<Waiter>,
}

struct Waiter {
    id: u64,
    /// What this waiter asked for; the job runs at the highest one.
    priority: SpeechPriority,
    on_done: OnDone,
}

struct QueueEntry {
    priority: SpeechPriority,
    entry_id: u64,
    key: AudioKey,
}

impl Ord for QueueEntry {
    /// Within one priority queue: for `Now` the lowest sentence first, then the older entry first.
    fn cmp(&self, other: &Self) -> Ordering {
        let by_sentence = if self.priority == SpeechPriority::Now { other.key.sentence_id.cmp(&self.key.sentence_id) } else { Ordering::Equal };
        self.priority.cmp(&other.priority).then(by_sentence).then_with(|| other.entry_id.cmp(&self.entry_id))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for QueueEntry {}

fn run_loop(shared: &Shared, engine: &dyn SpeechEngine, gate: Option<&dyn SynthesisGate>) {
    while let Some((key, request, priority)) = next_job(shared) {
        let mut request = Some(request);
        let mut outcome = None;
        let mut started = Instant::now();
        let mut run = || {
            // Timed from the turn on, so waiting for a phone's item doesn't count as synthesis time.
            started = Instant::now();
            outcome = request.take().map(|request| synthesize(engine, request));
        };
        match gate {
            Some(gate) => gate.with_turn(priority, &mut run),
            None => run(),
        }
        let outcome = outcome.unwrap_or_else(|| Err(AppError::new(ErrorName::SpeechSynthesisFailed).with_detail("the synthesis gate gave no turn")));
        let synth_ms = elapsed_ms(started);
        if let Ok(audio) = &outcome {
            shared.record(Timing { audio_ms: (audio_seconds(audio) * MILLIS_PER_SECOND).round() as u64, synth_ms });
        }

        let waiters = shared.lock().jobs.remove(&key).map(|pending| pending.waiters).unwrap_or_default();
        log_outcome(&key, priority, &outcome, synth_ms, waiters.len());
        deliver(&key, outcome, waiters);
    }
}

fn next_job(shared: &Shared) -> Option<(AudioKey, SpeechRequest, SpeechPriority)> {
    let mut queue = shared.lock();
    loop {
        if queue.shutdown {
            return None;
        }
        if let Some(job) = queue.take_next(shared.listening()) {
            return Some(job);
        }
        queue = shared.wake.wait(queue).unwrap_or_else(PoisonError::into_inner);
    }
}

/// A panicking engine fails this job only; the worker keeps running.
fn synthesize(engine: &dyn SpeechEngine, request: SpeechRequest) -> SpeechOutcome {
    match catch_unwind(AssertUnwindSafe(|| engine.synthesize(request))) {
        Ok(result) => result.map(Arc::new),
        Err(_) => Err(AppError::new(ErrorName::SpeechSynthesisFailed).with_detail("the speech engine panicked")),
    }
}

fn log_outcome(key: &AudioKey, priority: SpeechPriority, outcome: &SpeechOutcome, synth_ms: u64, waiters: usize) {
    let (doc_id, sentence_id, voice_id) = (&key.document_id, key.sentence_id.0, &key.voice_id);
    match outcome {
        Ok(audio) => {
            let audio_seconds = audio_seconds(audio);
            info!(service = NAME, doc_id = %doc_id, sentence_id, voice_id = %voice_id, priority = ?priority, audio_seconds, synth_ms, waiters, "speech synthesized");
        }
        Err(error) => {
            warn!(service = NAME, doc_id = %doc_id, sentence_id, voice_id = %voice_id, priority = ?priority, error = %error, synth_ms, waiters, "speech synthesis failed");
        }
    }
}

/// The last waiter gets the original outcome; the others get a copy of the error without its source
/// (the source isn't cloneable). A panicking callback is logged and the others still run.
fn deliver(key: &AudioKey, outcome: SpeechOutcome, waiters: Vec<Waiter>) {
    let mut original = Some(outcome);
    let count = waiters.len();
    for (index, waiter) in waiters.into_iter().enumerate() {
        let is_last = index + 1 == count;
        let outcome = if is_last { original.take().unwrap_or_else(|| Err(AppError::new(ErrorName::Internal))) } else { copy_outcome(original.as_ref()) };
        let on_done = waiter.on_done;
        if catch_unwind(AssertUnwindSafe(move || on_done(outcome))).is_err() {
            warn!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, voice_id = %key.voice_id, "speech callback panicked");
        }
    }
}

fn not_delivered() -> AppError {
    AppError::new(ErrorName::Internal).with_detail("speech job was cancelled or the worker stopped")
}

fn copy_outcome(outcome: Option<&SpeechOutcome>) -> SpeechOutcome {
    match outcome {
        Some(Ok(audio)) => Ok(audio.clone()),
        Some(Err(error)) => Err(copy_error(error)),
        None => Err(AppError::new(ErrorName::Internal)),
    }
}

fn copy_error(error: &AppError) -> AppError {
    let mut names = error.names().iter().copied();
    let mut copy = AppError::new(names.next().unwrap_or(ErrorName::Internal));
    for name in names {
        copy = copy.and(name);
    }
    match error.detail() {
        Some(detail) => copy.with_detail(detail),
        None => copy,
    }
}

/// Whole milliseconds as a number (tracing records `u128` as a string).
pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn audio_seconds(audio: &Audio) -> f64 {
    if audio.sample_rate == 0 {
        return 0.0;
    }
    audio.samples.len() as f64 / f64::from(audio.sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::fakes::{GatedEngine, key_for};
    use lectrice_domain::VoiceId;

    fn job(sentence: u32, priority: SpeechPriority) -> SpeechJob {
        SpeechJob { key: key_for(sentence), request: SpeechRequest { text: format!("s{sentence}"), voice_id: VoiceId::new("af_heart") }, priority }
    }

    /// Starts a worker whose engine is blocked on sentence 0, so later jobs pile up in the queue.
    fn blocked_worker() -> (Arc<GatedEngine>, SpeechWorker, SpeechTicket) {
        let engine = Arc::new(GatedEngine::new("s0"));
        let worker = SpeechWorker::start(engine.clone()).unwrap();
        let first = worker.submit(job(0, SpeechPriority::Export));
        engine.wait_until_blocked();
        (engine, worker, first)
    }

    #[test]
    fn runs_now_before_read_ahead_before_export() {
        let (engine, worker, first) = blocked_worker();
        let tickets = vec![
            worker.submit(job(1, SpeechPriority::Export)),
            worker.submit(job(2, SpeechPriority::ReadAhead)),
            worker.submit(job(3, SpeechPriority::Now)),
            worker.submit(job(4, SpeechPriority::ReadAhead)),
        ];

        engine.release();
        first.wait().unwrap();
        for ticket in tickets {
            ticket.wait().unwrap();
        }

        assert_eq!(engine.spoken(), ["s0", "s3", "s2", "s4", "s1"]);
    }

    /// A player asks for the next sentences at once; the one it waits for is made first.
    #[test]
    fn now_jobs_run_lowest_sentence_first() {
        let (engine, worker, first) = blocked_worker();
        let tickets: Vec<SpeechTicket> = [3, 2, 1].into_iter().map(|sentence| worker.submit(job(sentence, SpeechPriority::Now))).collect();

        engine.release();
        first.wait().unwrap();
        tickets.into_iter().for_each(|ticket| drop(ticket.wait().unwrap()));

        assert_eq!(engine.spoken(), ["s0", "s1", "s2", "s3"]);
    }

    /// A paused listener holds up neither an export nor, at the gate, a phone's audio.
    #[test]
    fn while_nobody_listens_exports_go_before_read_ahead_and_read_ahead_takes_an_export_turn() {
        let engine = Arc::new(GatedEngine::new("s0"));
        let gate = Arc::new(RecordingGate::default());
        let worker = SpeechWorker::start_gated(engine.clone(), Some(gate.clone())).unwrap();
        let first = worker.submit(job(0, SpeechPriority::Export));
        engine.wait_until_blocked();
        *worker.shared.last_listener.lock().unwrap() = Instant::now().checked_sub(LISTENER_IDLE_AFTER);
        let tickets = [worker.submit(job(1, SpeechPriority::ReadAhead)), worker.submit(job(2, SpeechPriority::Export)), worker.submit(job(3, SpeechPriority::Now))];

        engine.release();
        first.wait().unwrap();
        tickets.into_iter().for_each(|ticket| drop(ticket.wait().unwrap()));

        assert_eq!(engine.spoken(), ["s0", "s3", "s2", "s1"]);
        assert_eq!(*gate.turns.lock().unwrap(), [SpeechPriority::Export, SpeechPriority::Now, SpeechPriority::Export, SpeechPriority::Export]);
        worker.note_listener();
        assert!(worker.shared.listening());
    }

    #[test]
    fn identical_keys_are_synthesized_once() {
        let (engine, worker, first) = blocked_worker();
        let a = worker.submit(job(5, SpeechPriority::ReadAhead));
        let b = worker.submit(job(5, SpeechPriority::Now));
        let running = worker.submit(job(0, SpeechPriority::Now));

        engine.release();
        let (a, b) = (a.wait().unwrap(), b.wait().unwrap());
        assert!(Arc::ptr_eq(&first.wait().unwrap(), &running.wait().unwrap()));

        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(engine.spoken(), ["s0", "s5"]);
    }

    #[test]
    fn a_merge_with_higher_priority_moves_the_job_up() {
        let (engine, worker, first) = blocked_worker();
        let read_ahead = worker.submit(job(1, SpeechPriority::ReadAhead));
        let export = worker.submit(job(2, SpeechPriority::Export));
        let upgraded = worker.submit(job(2, SpeechPriority::Now));

        engine.release();
        for ticket in [first, read_ahead, export, upgraded] {
            ticket.wait().unwrap();
        }

        assert_eq!(engine.spoken(), ["s0", "s2", "s1"]);
    }

    #[test]
    fn a_cancelled_job_is_never_synthesized() {
        let (engine, worker, first) = blocked_worker();
        let cancelled = worker.submit(job(6, SpeechPriority::ReadAhead));
        let kept = worker.submit(job(7, SpeechPriority::ReadAhead));

        assert!(cancelled.cancel_handle().cancel());
        engine.release();
        first.wait().unwrap();
        kept.wait().unwrap();

        assert_eq!(cancelled.wait().unwrap_err().primary(), ErrorName::Internal);
        assert_eq!(engine.spoken(), ["s0", "s7"]);
    }

    #[test]
    fn cancelling_one_of_two_waiters_keeps_the_job() {
        let (engine, worker, first) = blocked_worker();
        let leaving = worker.submit(job(8, SpeechPriority::ReadAhead));
        let staying = worker.submit(job(8, SpeechPriority::ReadAhead));

        assert!(leaving.cancel_handle().cancel());
        engine.release();
        first.wait().unwrap();

        assert!(staying.wait().is_ok());
        assert_eq!(engine.spoken(), ["s0", "s8"]);
    }

    #[test]
    fn callbacks_receive_the_audio_and_a_late_cancel_does_nothing() {
        let (engine, worker, first) = blocked_worker();
        let (sender, receiver) = mpsc::channel();
        let handle = worker.submit_with(job(9, SpeechPriority::ReadAhead), move |outcome| sender.send(outcome.is_ok()).unwrap());

        engine.release();
        first.wait().unwrap();

        assert!(receiver.recv().unwrap());
        assert!(!handle.cancel());
    }

    #[test]
    fn engine_errors_and_panics_reach_every_waiter() {
        let (engine, worker, first) = blocked_worker();
        let failing = [worker.submit(job(GatedEngine::FAILING, SpeechPriority::Now)), worker.submit(job(GatedEngine::FAILING, SpeechPriority::Now))];
        let panicking = worker.submit(job(GatedEngine::PANICKING, SpeechPriority::Now));
        let after = worker.submit(job(1, SpeechPriority::Now));

        engine.release();
        first.wait().unwrap();

        for ticket in failing {
            assert_eq!(ticket.wait().unwrap_err().primary(), ErrorName::VoiceUnknown);
        }
        assert_eq!(panicking.wait().unwrap_err().primary(), ErrorName::SpeechSynthesisFailed);
        assert!(after.wait().is_ok());
    }

    #[test]
    fn dropping_the_worker_stops_it_and_fails_queued_tickets() {
        let (engine, worker, first) = blocked_worker();
        let queued = worker.submit(job(1, SpeechPriority::Now));

        engine.release();
        drop(worker);

        assert!(first.wait().is_ok());
        assert!(queued.wait().is_err() || engine.spoken().len() == 2);
    }

    #[test]
    fn a_cancelled_now_waiter_lowers_a_merged_job_to_read_ahead() {
        let (engine, worker, first) = blocked_worker();
        let read_ahead = worker.submit(job(1, SpeechPriority::ReadAhead));
        let merged = worker.submit(job(2, SpeechPriority::ReadAhead));
        let now = worker.submit(job(2, SpeechPriority::Now));

        assert!(now.cancel_handle().cancel());
        engine.release();
        for ticket in [first, read_ahead, merged] {
            ticket.wait().unwrap();
        }

        assert_eq!(engine.spoken(), ["s0", "s1", "s2"]);
    }

    #[test]
    fn a_cancelled_caller_stops_waiting_and_its_job_is_dropped() {
        let (engine, worker, first) = blocked_worker();
        let abandoned = worker.submit(job(3, SpeechPriority::Now));
        let cancellation = Cancellation::default();
        cancellation.cancel();

        assert_eq!(abandoned.wait_unless_cancelled(&cancellation).unwrap_err().primary(), ErrorName::RequestCancelled);
        let wanted = worker.submit(job(4, SpeechPriority::Now));
        engine.release();
        first.wait().unwrap();
        wanted.wait_unless_cancelled(&Cancellation::default()).unwrap();

        assert_eq!(engine.spoken(), ["s0", "s4"]);
    }

    #[test]
    fn cancel_if_queued_leaves_a_running_job_alone() {
        let (engine, worker, first) = blocked_worker();
        let queued = worker.submit(job(1, SpeechPriority::ReadAhead));

        assert!(!first.cancel_handle().cancel_if_queued(), "the running job is not withdrawn");
        assert!(queued.cancel_handle().cancel_if_queued());
        engine.release();

        assert!(first.wait().is_ok());
        assert!(queued.wait().is_err());
        assert_eq!(engine.spoken(), ["s0"]);
    }

    /// C17: the median real-time factor appears after ten synthesized jobs.
    #[test]
    fn timings_report_the_median_real_time_factor_after_enough_jobs() {
        let worker = SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap();
        for sentence in 0..9 {
            worker.submit(job(sentence, SpeechPriority::Now)).wait().unwrap();
        }
        assert_eq!(worker.timings(), SynthesisTimings { rtf_median: None, samples: 9 });

        worker.submit(job(9, SpeechPriority::Now)).wait().unwrap();

        let timings = worker.timings();
        assert_eq!(timings.samples, 10);
        assert!(timings.rtf_median.is_some_and(|rtf| rtf >= 0.0));
    }

    /// Records the priority of every turn it gives.
    #[derive(Default)]
    struct RecordingGate {
        turns: Mutex<Vec<SpeechPriority>>,
    }

    impl SynthesisGate for RecordingGate {
        fn with_turn(&self, priority: SpeechPriority, synthesize: &mut dyn FnMut()) {
            self.turns.lock().unwrap().push(priority);
            synthesize();
        }
    }

    /// C48: each job takes its turn at the machine's gate with its own priority (no more "everything is Now").
    #[test]
    fn every_job_takes_its_turn_at_the_gate_with_its_priority() {
        let gate = Arc::new(RecordingGate::default());
        let worker = SpeechWorker::start_gated(Arc::new(GatedEngine::open()), Some(gate.clone())).unwrap();

        for (sentence, priority) in [(1, SpeechPriority::Now), (2, SpeechPriority::ReadAhead), (3, SpeechPriority::Export)] {
            worker.submit(job(sentence, priority)).wait().unwrap();
        }

        assert_eq!(*gate.turns.lock().unwrap(), [SpeechPriority::Now, SpeechPriority::ReadAhead, SpeechPriority::Export]);
    }

    #[test]
    fn a_timed_wait_says_when_the_audio_is_not_there_yet() {
        let (engine, worker, first) = blocked_worker();

        assert!(first.wait_timeout(Duration::from_millis(20)).is_none());
        engine.release();
        assert!(first.wait_timeout(Duration::from_secs(5)).unwrap().is_ok());
        drop(worker);
    }

    /// Review L7: a panic in a callback used to kill the worker thread, so every later wait hung.
    #[test]
    fn a_panicking_callback_does_not_stop_the_worker() {
        let (engine, worker, first) = blocked_worker();
        worker.submit_with(job(5, SpeechPriority::Now), |_| panic!("callback panic"));
        let other_waiter = worker.submit(job(5, SpeechPriority::Now));

        engine.release();
        first.wait().unwrap();

        assert!(other_waiter.wait().is_ok());
        assert!(worker.submit(job(6, SpeechPriority::Now)).wait().is_ok());
    }
}
