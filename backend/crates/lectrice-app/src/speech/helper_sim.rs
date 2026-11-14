//! The link-drop simulator on a virtual clock, so a thousand long listening sessions run in seconds.
//!
//! Real code under test: `HelperFill` (plan, flow control, urgent items, cancel on jumps, first writer wins, the
//! first connection after `start`) with its `ReadAhead` pause and resume, `HelperWait` (how long a sentence the player
//! needs waits for the computer) and `lost_read_ahead_minutes`. Simulated: the computer (one item at a time at its RTF,
//! the first one slower while its engine is cold), the link (first connection, cut, noticed at once or after the dead
//! time, reconnect on the 1-2-5-10-30-60 s schedule once healed), the phone's engine (loads before its first sentence),
//! its local read-ahead (runs only while `ReadAhead` isn't paused for the book, at the phone's RTF, up to the horizon
//! the shell sets for a lost link) and the player (plays at `rate`; a sentence that isn't cached when its turn comes is
//! a gap; a sentence it waits for goes local when `HelperWait` says so, like `SpeakSentence`).
//!
//! The same story over real sockets (fewer runs, real time) is `lectrice-infra/tests/helper_link.rs`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lectrice_domain::{
    AppError, AppResult, AudioCache, AudioKey, AudioRetention, Block, BlockKind, Document, DocumentId, ErrorName, HelperEngine, HelperEvent, HelperEvents,
    HelperItem, HelperState, Sentence, SentenceId, SpeechHelper, VoiceId,
};

use super::fakes::{CountingEncoder, ENGINE_ID, GatedEngine, ListedReader, MemoryCache, SAMPLE_RATE};
use super::helper_fill::{
    GREEN_RTF_TIMES_RATE, HELPER_AHEAD_MINUTES, HELPER_COLD_GRACE, HELPER_CONNECT_WAIT, HelperFill, HelperFillDeps, HelperWait, lost_read_ahead_minutes,
};
use super::read_ahead::{ListenerPosition, ReadAhead, ReadAheadDeps, ReadAheadSettings};
use super::read_ahead_plan::{CHARS_PER_SECOND, playable_in_order};
use super::worker::SpeechWorker;
use crate::units::SECONDS_PER_MINUTE;

const RUNS: u64 = 1_000;
/// Each sentence is 15 s of estimated 1x audio, so the book (420 sentences) is 105 minutes long.
const SENTENCE_CHARS: usize = 270;
const SENTENCES: u32 = 420;
/// Each run listens this long (virtual time).
const LISTEN_SECONDS: f64 = 75.0 * 60.0;
const RECONNECT_DELAYS: [f64; 6] = [1.0, 2.0, 5.0, 10.0, 30.0, 60.0];
/// A reconnect attempt succeeds once the link has been back this long (TCP + Noise IK on a LAN).
const CONNECT_SECONDS: f64 = 0.2;
/// The phone's model load before its first sentence (measured on a Galaxy S10+).
const PHONE_LOAD_SECONDS: f64 = 2.2;
/// A first connection to a computer that isn't there fails after this long (TCP timeout on the LAN).
const FIRST_FAIL_SECONDS: f64 = 3.0;
/// A sentence of about ten words (63 characters, 3.5 s of speech) for the first-sentence runs.
const SHORT_SENTENCE_CHARS: usize = 63;
/// The listener's own read-ahead setting.
const LOCAL_SETTING_MINUTES: u32 = 10;
const DEAD_AFTER_SECONDS: f64 = 15.0;
/// One sentence of slack for "the buffer was under the horizon when the phone started".
const SLACK_SECONDS: f64 = SENTENCE_CHARS as f64 / CHARS_PER_SECOND;

/// The simulated computer's side of the port: it queues what the app asks for; the simulation makes it.
#[derive(Default)]
struct SimHelper {
    inner: Mutex<SimHelperInner>,
}

#[derive(Default)]
struct SimHelperInner {
    state: Option<HelperState>,
    events: Option<Arc<dyn HelperEvents>>,
    queue: VecDeque<HelperItem>,
}

impl SimHelper {
    fn emit(&self, event: HelperEvent) {
        let events = {
            let mut inner = self.inner.lock().unwrap();
            if let HelperEvent::State(state) = &event {
                inner.state = Some(state.clone());
                if !state.makes_clips() {
                    inner.queue.clear();
                }
            }
            inner.events.clone()
        };
        if let Some(events) = events {
            events.on_event(event);
        }
    }

    fn next_item(&self) -> Option<HelperItem> {
        self.inner.lock().unwrap().queue.pop_front()
    }
}

impl SpeechHelper for SimHelper {
    fn state(&self) -> HelperState {
        self.inner.lock().unwrap().state.clone().unwrap_or(HelperState::Off)
    }

    fn open(&self, _engine: HelperEngine, events: Arc<dyn HelperEvents>) -> AppResult<()> {
        self.inner.lock().unwrap().events = Some(events);
        Ok(())
    }

    /// Urgent items go before every fill item; an item asked for again moves (as the real server's queue does).
    fn make(&self, items: Vec<HelperItem>) -> AppResult<()> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.state.as_ref().is_some_and(HelperState::makes_clips) {
            return Err(AppError::new(ErrorName::HelperUnreachable));
        }
        for item in items {
            inner.queue.retain(|queued| queued.item_id != item.item_id);
            if item.urgent {
                let at = inner.queue.iter().position(|queued| !queued.urgent).unwrap_or(inner.queue.len());
                inner.queue.insert(at, item);
            } else {
                inner.queue.push_back(item);
            }
        }
        Ok(())
    }

    fn cancel_all(&self) -> AppResult<()> {
        self.inner.lock().unwrap().queue.clear();
        Ok(())
    }

    fn close(&self) {
        self.inner.lock().unwrap().state = Some(HelperState::Off);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in [low, high).
    fn between(&mut self, low: f64, high: f64) -> f64 {
        low + (self.next() % 1_000_000) as f64 / 1_000_000.0 * (high - low)
    }
}

#[derive(Debug, Clone, Copy)]
struct Scenario {
    rate: f32,
    phone_rtf: f32,
    computer_rtf: f64,
    cut_at: f64,
    outage: f64,
    /// A TCP error is noticed at once, a silent stall after the dead time.
    noticed_after: f64,
    jump_at: Option<f64>,
    /// When the first connection after `start` is ready; `None`: the computer isn't there (the attempt fails after
    /// `FIRST_FAIL_SECONDS` and it never comes).
    first_link_after: Option<f64>,
    /// Extra time the computer's first item takes (its engine is cold).
    cold_start: f64,
}

/// How the player decides to make a sentence itself. `current` is the core's rule; `without_link_wait` is the earlier
/// rule measured on a Galaxy S10+ (1.5 s from the request, no wait for a link that is still connecting).
#[derive(Debug, Clone, Copy)]
struct Policy {
    wait: HelperWait,
    connect_wait: bool,
}

impl Policy {
    fn current() -> Self {
        Self { wait: HelperWait::default(), connect_wait: true }
    }

    fn without_link_wait() -> Self {
        let grace = Duration::from_millis(1500);
        Self { wait: HelperWait { grace, cold_grace: grace }, connect_wait: false }
    }
}

impl Scenario {
    /// Green phones at any speed; red phones (Galaxy S10+ class) at 1x, which is what the helper promises them. Cuts start
    /// once a little is buffered (a cut in the first two minutes is a cold start, not a drop) and last up to 25 min.
    fn draw(rng: &mut Rng) -> Self {
        let red = rng.next().is_multiple_of(3);
        let (rate, phone_rtf) = if red { (1.0, 1.11) } else { ([1.0, 1.5, 2.0][(rng.next() % 3) as usize], 0.25) };
        let jump = !red && rng.next().is_multiple_of(2);
        Self {
            rate,
            phone_rtf,
            computer_rtf: [0.186, 0.254][(rng.next() % 2) as usize],
            cut_at: rng.between(120.0, 40.0 * 60.0),
            outage: rng.between(1.0, 25.0 * 60.0),
            noticed_after: if rng.next().is_multiple_of(2) { 0.0 } else { DEAD_AFTER_SECONDS },
            jump_at: jump.then(|| rng.between(60.0, 60.0 * 60.0)),
            first_link_after: Some(rng.between(0.02, 1.2)),
            cold_start: rng.between(0.0, 1.5),
        }
    }

    /// A session start with nothing else happening: the link takes up to 1.2 s, the computer's first item up to
    /// 1.5 s more (a cold desktop took 1.95–2.06 s for its first sentence).
    fn start(rng: &mut Rng) -> Self {
        Self {
            rate: 1.0,
            phone_rtf: 1.11,
            computer_rtf: [0.186, 0.254][(rng.next() % 2) as usize],
            cut_at: f64::INFINITY,
            outage: 0.0,
            noticed_after: 0.0,
            jump_at: None,
            first_link_after: Some(rng.between(0.02, 1.2)),
            cold_start: rng.between(0.5, 1.5),
        }
    }

    fn is_red(&self) -> bool {
        self.phone_rtf * self.rate > GREEN_RTF_TIMES_RATE
    }
}

/// What one run saw, for the assertions.
#[derive(Debug, Default)]
struct Outcome {
    gaps: u32,
    /// When, and at which playlist index, for the failure message.
    first_gap: Option<(f64, usize)>,
    lost_at: Option<f64>,
    reconnected_at: Option<f64>,
    first_local_start: Option<f64>,
    /// Listening seconds cached ahead of the player when the phone first made audio itself.
    buffered_at_first_local: Option<f64>,
    local_starts_after_reconnect: u32,
    paused_after_reconnect: bool,
    /// Helper clips dropped after the jump (cancelled or stale items).
    jump_waste: Option<u64>,
    puts: usize,
    cached: usize,
    /// Sentences the player was waiting for that the phone made itself, and when the first one started.
    player_local_starts: u32,
    first_player_local: Option<f64>,
    /// When the phone's engine was first needed (it loads then).
    engine_loaded_at: Option<f64>,
    first_sound_at: Option<f64>,
}

struct World {
    scenario: Scenario,
    policy: Policy,
    listen_seconds: f64,
    document: Arc<Document>,
    order: Vec<SentenceId>,
    keys: Vec<AudioKey>,
    cache: Arc<MemoryCache>,
    helper: Arc<SimHelper>,
    read_ahead: Arc<ReadAhead>,
    fill: Arc<HelperFill>,
    now: f64,
    /// The computer's item being made, and when it is done.
    making: Option<(f64, HelperItem)>,
    /// The first connection after `start` is done (or failed).
    first_link_done: bool,
    /// What is left of the computer's cold start.
    cold_left: f64,
    link_up: bool,
    lost_noticed_at: Option<f64>,
    healed_at: f64,
    next_attempt: Option<f64>,
    failures: usize,
    /// The phone's own engine: when it is done, and with which playlist index.
    local: Option<(f64, usize)>,
    engine_loaded: bool,
    /// The player: its playlist index, when the sentence playing ends, or since when it waits (start and jumps).
    playing: usize,
    play_end: Option<f64>,
    waiting_since: Option<f64>,
    /// When the sentence the player waits for went to the computer as urgent.
    urgent_sent_at: Option<f64>,
    outcome: Outcome,
}

fn seconds_of(sentence: &Sentence) -> f64 {
    sentence.spoken_text().chars().count() as f64 / CHARS_PER_SECOND
}

fn book(sentence_chars: usize) -> Document {
    let blocks = (0..SENTENCES)
        .map(|id| Block {
            id,
            kind: BlockKind::Paragraph,
            level: None,
            page_index: 1,
            spoken: true,
            text: None,
            sentences: vec![Sentence { id: SentenceId(id), text: format!("{id:04}{}.", "a".repeat(sentence_chars - 5)), page_index: 1, speech_text: None }],
            cue: None,
        })
        .collect();
    Document {
        id: DocumentId::parse(&format!("{:064x}", 7)).unwrap(),
        title: "T".into(),
        source_file_name: "t.pdf".into(),
        page_count: 1,
        cleaner_version: 1,
        pages: Vec::new(),
        toc: Vec::new(),
        blocks,
    }
}

impl World {
    fn new(scenario: Scenario, policy: Policy, listen_seconds: f64, document: Arc<Document>, worker: &Arc<SpeechWorker>) -> Self {
        let cache = Arc::new(MemoryCache::default());
        let helper = Arc::new(SimHelper::default());
        let reader = Arc::new(ListedReader::default());
        let encoder = Arc::new(CountingEncoder);
        let settings = ReadAheadSettings { read_ahead_minutes: LOCAL_SETTING_MINUTES, keep_minutes_per_document: 60 };
        let deps = ReadAheadDeps { worker: worker.clone(), reader: reader.clone(), cache: cache.clone(), retention: cache.clone(), encoder: encoder.clone() };
        let read_ahead = Arc::new(ReadAhead::new(deps, settings).unwrap());
        let fill = HelperFill::new(HelperFillDeps {
            helper: helper.clone(),
            engine: Arc::new(GatedEngine::open()),
            reader,
            cache: cache.clone(),
            retention: cache.clone(),
            encoder,
            read_ahead: read_ahead.clone(),
        });
        fill.start().unwrap();
        helper.emit(HelperEvent::State(HelperState::Connecting));
        let order: Vec<SentenceId> = playable_in_order(&document, false).into_iter().map(|(_, sentence)| sentence.id).collect();
        let keys = order
            .iter()
            .map(|sentence_id| AudioKey {
                document_id: document.id.clone(),
                cleaner_version: 1,
                engine_id: ENGINE_ID.into(),
                voice_id: VoiceId::new("af_heart"),
                sentence_id: *sentence_id,
            })
            .collect();
        Self {
            scenario,
            policy,
            listen_seconds,
            document,
            order,
            keys,
            cache,
            helper,
            read_ahead,
            fill,
            now: 0.0,
            making: None,
            first_link_done: false,
            cold_left: scenario.cold_start,
            link_up: scenario.first_link_after.is_some(),
            lost_noticed_at: None,
            healed_at: f64::INFINITY,
            next_attempt: None,
            failures: 0,
            local: None,
            engine_loaded: false,
            playing: 0,
            play_end: None,
            waiting_since: Some(0.0),
            urgent_sent_at: None,
            outcome: Outcome::default(),
        }
    }

    /// `HelperFill::first_link_left` on the virtual clock (the real one counts on the wall clock from `start`).
    fn first_link_left(&self) -> Option<Duration> {
        let left = HELPER_CONNECT_WAIT.as_secs_f64() - self.now;
        (self.policy.connect_wait && !self.first_link_done && left > 0.0).then(|| Duration::from_secs_f64(left))
    }

    /// The phone's engine starts on a sentence, loading first if it has to.
    fn start_local(&mut self, index: usize) {
        let load = if self.engine_loaded { 0.0 } else { PHONE_LOAD_SECONDS };
        self.engine_loaded = true;
        self.outcome.engine_loaded_at.get_or_insert(self.now);
        let seconds = seconds_of(self.sentence(index));
        self.local = Some((self.now + load + seconds * f64::from(self.scenario.phone_rtf), index));
    }

    fn key(&self, index: usize) -> &AudioKey {
        &self.keys[index]
    }

    /// Sentence ids are their playlist index in this book.
    fn sentence(&self, index: usize) -> &Sentence {
        &self.document.blocks[index].sentences[0]
    }

    fn cached(&self, index: usize) -> bool {
        self.cache.contains(self.key(index)).unwrap()
    }

    /// What every audio request does: the helper (and the paused local read-ahead) follow the listener.
    fn request(&mut self, index: usize) {
        self.fill.follow(ListenerPosition { document: &self.document, key: self.key(index), rate: self.scenario.rate, read_skipped: false });
    }

    fn ask_urgently(&self, index: usize) {
        self.fill.urgent(self.key(index), self.sentence(index).spoken_text());
    }

    /// Listening seconds cached right after the player, in order.
    fn buffered_ahead(&self) -> f64 {
        (self.playing + 1..self.order.len())
            .take_while(|index| self.cached(*index))
            .map(|index| seconds_of(self.sentence(index)) / f64::from(self.scenario.rate))
            .sum()
    }

    fn run(mut self) -> Outcome {
        self.request(0);
        let mut dropped_before_jump = None;
        while self.now < self.listen_seconds && self.playing + 1 < self.order.len() {
            self.step();
            if let Some(at) = self.scenario.jump_at
                && dropped_before_jump.is_none()
                && self.now >= at
                && self.link_up
                && self.fill.makes_clips()
                && self.waiting_since.is_none()
            {
                dropped_before_jump = Some(self.fill.status().stats.dropped);
                self.jump();
            }
        }
        self.outcome.jump_waste = dropped_before_jump.map(|before| self.fill.status().stats.dropped - before);
        self.outcome.paused_after_reconnect = self.read_ahead.is_paused(&self.document.id);
        self.outcome.puts = self.cache.puts();
        self.outcome.cached = self.cache.keys().len();
        self.outcome
    }

    /// The listener jumps far ahead: the sentence there is asked for urgently, like a cache miss after a seek.
    fn jump(&mut self) {
        let target = (self.playing + 100).min(self.order.len() - 2);
        self.playing = target;
        self.play_end = None;
        self.waiting_since = Some(self.now);
        self.urgent_sent_at = None;
        self.request(target);
    }

    /// Moves the clock to the next thing that happens and handles everything due then.
    fn step(&mut self) {
        let scenario = self.scenario;
        let candidates = [
            self.play_end,
            self.making.as_ref().map(|(at, _)| *at),
            self.local.map(|(at, _)| at),
            (self.link_up && self.outcome.lost_at.is_none()).then_some(scenario.cut_at),
            (!self.link_up && self.healed_at.is_infinite()).then_some(scenario.cut_at + scenario.outage),
            self.lost_noticed_at,
            self.next_attempt,
            (!self.first_link_done).then(|| scenario.first_link_after.unwrap_or(FIRST_FAIL_SECONDS)),
        ];
        let next = candidates.into_iter().flatten().filter(|at| *at > self.now).fold(f64::INFINITY, f64::min);
        // A waiting player looks at the cache at least every 100 ms.
        let poll = if self.waiting_since.is_some() { 0.1 } else { f64::INFINITY };
        self.now = next.min(self.now + poll);

        self.link();
        self.computer();
        self.phone();
        self.player();
    }

    fn link(&mut self) {
        let scenario = self.scenario;
        if !self.first_link_done {
            match scenario.first_link_after {
                Some(at) if self.now >= at => {
                    self.first_link_done = true;
                    self.helper.emit(HelperEvent::State(HelperState::Connected));
                }
                None if self.now >= FIRST_FAIL_SECONDS => {
                    self.first_link_done = true;
                    self.outcome.lost_at = Some(self.now);
                    self.helper.emit(HelperEvent::State(HelperState::Lost));
                    self.failures = 1;
                    self.next_attempt = Some(self.now + RECONNECT_DELAYS[0]);
                }
                _ => {}
            }
        }
        if self.link_up && self.outcome.lost_at.is_none() && self.lost_noticed_at.is_none() && self.now >= scenario.cut_at {
            self.link_up = false;
            self.making = None;
            self.lost_noticed_at = Some(scenario.cut_at + scenario.noticed_after);
        }
        if self.outcome.lost_at.is_none()
            && let Some(at) = self.lost_noticed_at
            && self.now >= at
        {
            self.outcome.lost_at = Some(self.now);
            self.helper.emit(HelperEvent::State(HelperState::Lost));
            self.failures = 1;
            self.next_attempt = Some(self.now + RECONNECT_DELAYS[0]);
        }
        if !self.link_up && self.healed_at.is_infinite() && self.now >= scenario.cut_at + scenario.outage {
            self.healed_at = self.now;
        }
        if let Some(at) = self.next_attempt
            && self.now >= at
        {
            if self.now >= self.healed_at + CONNECT_SECONDS {
                self.next_attempt = None;
                self.link_up = true;
                self.outcome.reconnected_at = Some(self.now);
                self.helper.emit(HelperEvent::State(HelperState::Connected));
            } else {
                self.failures += 1;
                self.next_attempt = Some(self.now + RECONNECT_DELAYS[(self.failures - 1).min(RECONNECT_DELAYS.len() - 1)]);
            }
        }
    }

    /// One item at a time; a finished clip reaches the phone only while the link is up.
    fn computer(&mut self) {
        if self.making.as_ref().is_some_and(|(at, _)| self.now >= *at)
            && let Some((_, item)) = self.making.take()
            && self.link_up
        {
            let audio = lectrice_domain::Audio { samples: vec![0.0; 10], sample_rate: SAMPLE_RATE };
            self.helper.emit(HelperEvent::Clip { item_id: item.item_id, audio });
        }
        if self.making.is_none()
            && self.link_up
            && let Some(item) = self.helper.next_item()
        {
            let seconds = item.text.chars().count() as f64 / CHARS_PER_SECOND;
            let cold = std::mem::take(&mut self.cold_left);
            self.making = Some((self.now + cold + seconds * self.scenario.computer_rtf, item));
        }
    }

    /// The phone's own read-ahead: only while it isn't paused for the helper, up to the shell's horizon for a lost link.
    fn phone(&mut self) {
        if let Some((at, index)) = self.local
            && self.now >= at
        {
            self.local = None;
            self.cache.put_if_absent(self.key(index), format!("{SAMPLE_RATE}:11").as_bytes()).unwrap();
        }
        if self.local.is_some() || self.read_ahead.is_paused(&self.document.id) {
            return;
        }
        let minutes = lost_read_ahead_minutes(Some(self.scenario.phone_rtf), self.scenario.rate, LOCAL_SETTING_MINUTES);
        let horizon = f64::from(minutes) * SECONDS_PER_MINUTE * f64::from(self.scenario.rate);
        let mut planned = 0.0;
        for index in self.playing + 1..self.order.len() {
            if planned >= horizon {
                return;
            }
            let seconds = seconds_of(self.sentence(index));
            planned += seconds;
            if self.cached(index) {
                continue;
            }
            if self.outcome.first_local_start.is_none() {
                self.outcome.first_local_start = Some(self.now);
                self.outcome.buffered_at_first_local = Some(self.buffered_ahead());
            }
            if self.outcome.reconnected_at.is_some() {
                self.outcome.local_starts_after_reconnect += 1;
            }
            self.start_local(index);
            return;
        }
    }

    fn player(&mut self) {
        if self.play_end.is_some_and(|end| self.now >= end) {
            self.playing += 1;
            self.play_end = None;
            if self.cached(self.playing) {
                self.start_playing();
            } else {
                self.outcome.gaps += 1;
                self.outcome.first_gap.get_or_insert((self.now, self.playing));
                self.waiting_since = Some(self.now);
                self.urgent_sent_at = None;
                self.request(self.playing);
            }
        }
        if self.waiting_since.is_none() {
            return;
        }
        if self.cached(self.playing) {
            self.start_playing();
            return;
        }
        // What `SpeakSentence` does for a miss: urgent once the link is ready, then the grace from that moment.
        let ready = self.fill.makes_clips();
        if ready && self.urgent_sent_at.is_none() {
            self.ask_urgently(self.playing);
            self.urgent_sent_at = Some(self.now);
        }
        if self.local.is_some_and(|(_, index)| index == self.playing) {
            return;
        }
        let ready_for = self.urgent_sent_at.filter(|_| ready).map(|sent| Duration::from_secs_f64(self.now - sent));
        if self.policy.wait.left(self.first_link_left(), ready_for, self.engine_loaded).is_none() {
            // The phone makes it now (a read-ahead job it was on gives way).
            self.outcome.player_local_starts += 1;
            self.outcome.first_player_local.get_or_insert(self.now);
            self.start_local(self.playing);
        }
    }

    fn start_playing(&mut self) {
        self.outcome.first_sound_at.get_or_insert(self.now);
        self.urgent_sent_at = None;
        self.waiting_since = None;
        let seconds = seconds_of(self.sentence(self.playing)) / f64::from(self.scenario.rate);
        self.play_end = Some(self.now + seconds);
        self.request(self.playing);
    }
}

/// No gap, the cache, the local engine and the read-ahead behave across a thousand random runs.
#[test]
fn a_thousand_link_drops_never_make_a_gap() {
    let document = Arc::new(book(SENTENCE_CHARS));
    let worker = Arc::new(SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap());
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let (mut lost, mut jumps, mut reds) = (0, 0, 0);
    for run in 0..RUNS {
        let scenario = Scenario::draw(&mut rng);
        let outcome = World::new(scenario, Policy::current(), LISTEN_SECONDS, document.clone(), &worker).run();
        let context = format!("run {run}: {scenario:?} → {outcome:?}");

        assert_eq!(outcome.gaps, 0, "(1) no gap: {context}");
        assert_eq!(outcome.puts, outcome.cached, "(4) no sentence stored twice: {context}");
        if let Some(waste) = outcome.jump_waste {
            jumps += 1;
            assert!(waste <= 1, "(6) at most one item wasted per jump: {context}");
        }
        let Some(lost_at) = outcome.lost_at else { continue };
        lost += 1;
        reds += u32::from(scenario.is_red());
        if let Some(started) = outcome.first_local_start {
            assert!(started >= lost_at, "the phone's engine stays idle while connected: {context}");
            let buffered = outcome.buffered_at_first_local.unwrap();
            if scenario.is_red() {
                let full = f64::from(HELPER_AHEAD_MINUTES) * 60.0 - SLACK_SECONDS;
                assert!(started - lost_at < 1.0 || buffered >= full, "(3) a red phone starts at once: {context}");
            } else {
                assert!(buffered < f64::from(LOCAL_SETTING_MINUTES) * 60.0 + SLACK_SECONDS, "(3) a green phone waits for the 10-minute mark: {context}");
            }
        }
        if outcome.reconnected_at.is_some() {
            assert!(outcome.paused_after_reconnect, "(5) the helper has the book again: {context}");
            assert_eq!(outcome.local_starts_after_reconnect, 0, "(5) the local worker stops: {context}");
        }
    }
    assert!(lost > 900 && jumps > 200 && reds > 250, "enough of each case ran: lost {lost}, jumps {jumps}, red {reds}");
}

/// A red phone loses the computer right after a full fill and never gets it back: starting at the far end of the 30
/// buffered minutes at once, it plays the rest of the session without a gap (~5 h at 1x).
#[test]
fn a_red_phone_plays_on_without_the_computer() {
    let document = Arc::new(book(SENTENCE_CHARS));
    let worker = Arc::new(SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap());
    let scenario = Scenario {
        rate: 1.0,
        phone_rtf: 1.11,
        computer_rtf: 0.254,
        cut_at: 12.0 * 60.0,
        outage: 10.0 * 3600.0,
        noticed_after: DEAD_AFTER_SECONDS,
        jump_at: None,
        first_link_after: Some(CONNECT_SECONDS),
        cold_start: 0.0,
    };

    let outcome = World::new(scenario, Policy::current(), LISTEN_SECONDS, document, &worker).run();

    assert_eq!(outcome.gaps, 0, "{outcome:?}");
    assert!(outcome.reconnected_at.is_none());
    assert!(outcome.first_local_start.unwrap() - outcome.lost_at.unwrap() < 1.0, "{outcome:?}");
}

const START_RUNS: u64 = 300;
const START_LISTEN_SECONDS: f64 = 60.0;

fn start_runs(policy: Policy) -> Vec<(Scenario, Outcome)> {
    let document = Arc::new(book(SHORT_SENTENCE_CHARS));
    let worker = Arc::new(SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap());
    let mut rng = Rng(0x5eed_1234_abcd_0005);
    (0..START_RUNS)
        .map(|_| {
            let scenario = Scenario::start(&mut rng);
            (scenario, World::new(scenario, policy, START_LISTEN_SECONDS, document.clone(), &worker).run())
        })
        .collect()
}

/// Play pressed while the link is still connecting, and a cold computer that answers the first
/// sentence in up to ~2.6 s. The phone waits for both and never loads its own engine; nothing gaps afterwards.
#[test]
fn the_first_sentence_waits_for_the_link_and_a_cold_computer() {
    for (run, (scenario, outcome)) in start_runs(Policy::current()).into_iter().enumerate() {
        let context = format!("run {run}: {scenario:?} → {outcome:?}");
        assert_eq!(outcome.engine_loaded_at, None, "the phone's engine stays unloaded: {context}");
        assert_eq!(outcome.player_local_starts, 0, "{context}");
        assert_eq!(outcome.gaps, 0, "{context}");
        let first_item = f64::from(SHORT_SENTENCE_CHARS as u32) / CHARS_PER_SECOND * scenario.computer_rtf;
        let expected = scenario.first_link_after.unwrap() + scenario.cold_start + first_item;
        assert!(outcome.first_sound_at.unwrap() <= expected + 0.15, "the first sentence plays as soon as the computer has it: {context}");
    }
}

/// The same starts with the earlier rule: most of them load the phone's engine (so the test above tells the two rules
/// apart).
#[test]
fn the_rule_without_a_link_wait_loaded_the_engine_at_most_starts() {
    let loaded = start_runs(Policy::without_link_wait()).iter().filter(|(_, outcome)| outcome.engine_loaded_at.is_some()).count();
    assert!(loaded * 4 >= START_RUNS as usize * 3, "{loaded} of {START_RUNS}");
}

/// The computer isn't there: the first sentence waits only the connect wait, then the phone makes it.
#[test]
fn an_absent_computer_costs_at_most_the_connect_wait() {
    let document = Arc::new(book(SHORT_SENTENCE_CHARS));
    let worker = Arc::new(SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap());
    let scenario = Scenario { first_link_after: None, ..Scenario::start(&mut Rng(3)) };

    let outcome = World::new(scenario, Policy::current(), START_LISTEN_SECONDS, document, &worker).run();

    let started = outcome.first_player_local.unwrap();
    let connect_wait = HELPER_CONNECT_WAIT.as_secs_f64();
    assert!((connect_wait..connect_wait + 0.15).contains(&started), "{outcome:?}");
    assert!(outcome.first_local_start.is_some_and(|at| at >= FIRST_FAIL_SECONDS), "the read-ahead follows once the link failed: {outcome:?}");
}

/// Connected, but the computer never answers: the cold grace, counted from the urgent item, then the phone.
#[test]
fn a_computer_that_never_answers_costs_the_cold_grace() {
    let document = Arc::new(book(SHORT_SENTENCE_CHARS));
    let worker = Arc::new(SpeechWorker::start(Arc::new(GatedEngine::open())).unwrap());
    let scenario = Scenario { first_link_after: Some(0.4), cold_start: 3600.0, ..Scenario::start(&mut Rng(4)) };

    let outcome = World::new(scenario, Policy::current(), START_LISTEN_SECONDS, document, &worker).run();

    let started = outcome.first_player_local.unwrap();
    let expected = 0.4 + HELPER_COLD_GRACE.as_secs_f64();
    assert!((expected..expected + 0.15).contains(&started), "{outcome:?}");
}
