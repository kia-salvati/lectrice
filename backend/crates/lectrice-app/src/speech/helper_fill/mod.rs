//! `HelperFill` (file 24 C43–C45): while a helper computer is connected, it plans 30 listening-minutes ahead of the
//! listener and asks the helper for the sentences that aren't cached, nearest first, a few at a time; the clips that
//! come back go into the normal audio cache under the normal keys (first writer wins). The local read-ahead is paused
//! for that book meanwhile (C44) and the local engine stays unloaded (C46). When the link goes, the in-flight items
//! are forgotten and the local read-ahead resumes: the player only ever reads the cache or the local engine, so a
//! lost link can't make a gap.
//!
//! The player's thread calls [`HelperFill::follow`] and [`HelperFill::urgent`]; the helper's thread delivers events.
//! Commands to the helper are issued under this use case's lock (the port promises they return at once), so they
//! reach the helper in the order the state changed.

mod prepare;
#[cfg(test)]
mod tests;
mod urgent;

pub use prepare::HelperPrepareProgress;
use prepare::PrepareTarget;
pub use urgent::{HELPER_COLD_GRACE, HELPER_CONNECT_WAIT, HELPER_NOW_GRACE, HelperWait, UrgentWait};

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioEncoder, AudioKey, AudioRetention, DocumentId, DocumentReader, ErrorName, HelperEngine, HelperEvent,
    HelperEvents, HelperItem, HelperState, SentenceId, SpeechEngine, SpeechHelper,
};
use tracing::{debug, info, warn};

use super::read_ahead::{ListenerPosition, ReadAhead};
use super::read_ahead_plan::{ReadAheadInput, chars_to_ms, plan_read_ahead};
use crate::sync::lock;

const NAME: &str = "HelperFill";

/// How far ahead the helper fills, in listening minutes (file 24 §3.6, user 2026-11-13).
pub const HELPER_AHEAD_MINUTES: u32 = 30;
/// Flow control (file 24 §3.5): at most this many items, or this much estimated 1x audio, in flight at once.
pub const HELPER_MAX_IN_FLIGHT_ITEMS: usize = 12;
pub const HELPER_MAX_IN_FLIGHT_MS: u64 = 3 * 60 * 1000;
/// A phone whose sustained `rtf × rate` is at most this keeps up alone ("green", file 12 §3.3).
pub const GREEN_RTF_TIMES_RATE: f32 = 0.6;

/// The local read-ahead's length once the link is lost (file 24 §3.6): a green phone waits with its normal setting
/// (the 30 buffered minutes last long), any other phone (or one without stats yet) starts at the far end of the
/// buffer at once.
pub fn lost_read_ahead_minutes(rtf_median: Option<f32>, rate: f32, setting_minutes: u32) -> u32 {
    match rtf_median {
        Some(rtf) if rtf * rate <= GREEN_RTF_TIMES_RATE => setting_minutes,
        _ => HELPER_AHEAD_MINUTES,
    }
}

pub struct HelperFillDeps {
    pub helper: Arc<dyn SpeechHelper>,
    /// The phone's own engine: only its id, revision, sample rate and voices are read here, so a deferred engine
    /// stays unloaded.
    pub engine: Arc<dyn SpeechEngine>,
    /// Checked before a clip is stored, so a removed document gets no new clips.
    pub reader: Arc<dyn DocumentReader>,
    pub cache: Arc<dyn AudioCache>,
    pub retention: Arc<dyn AudioRetention>,
    pub encoder: Arc<dyn AudioEncoder>,
    pub read_ahead: Arc<ReadAhead>,
}

/// Counts for the log and the shell's status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HelperFillStats {
    pub sent: u64,
    pub stored: u64,
    /// Clips that arrived after the phone had made the sentence itself.
    pub already_cached: u64,
    /// Clips for items no longer asked for (cancelled, or the link was lost meanwhile).
    pub dropped: u64,
    pub failed: u64,
    /// Re-plans that cancelled the helper's queue.
    pub cancels: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HelperFillStatus {
    pub state: HelperState,
    pub in_flight: usize,
    pub stats: HelperFillStats,
}

pub struct HelperFill {
    deps: HelperFillDeps,
    state: Mutex<FillState>,
    /// Woken when a clip is stored or the link changes, for the callers waiting on an urgent sentence.
    changed: Condvar,
    me: Weak<HelperFill>,
}

/// Same book, cleaner version, engine and voice: the clips one plan is made of.
#[derive(Debug, Clone, PartialEq)]
struct Anchor {
    key: AudioKey,
    rate: f32,
    read_skipped: bool,
}

impl Anchor {
    fn same_scope(&self, key: &AudioKey) -> bool {
        same_scope(&self.key, key)
    }
}

fn same_scope(own: &AudioKey, key: &AudioKey) -> bool {
    own.document_id == key.document_id && own.cleaner_version == key.cleaner_version && own.engine_id == key.engine_id && own.voice_id == key.voice_id
}

struct Planned {
    sentence_id: SentenceId,
    text: String,
    estimated_ms: u64,
}

struct Target {
    anchor: Anchor,
    /// The listener's own sentence (when it has speech). Asked for first and urgent while it isn't cached: when the
    /// link becomes ready while the player waits for it, the fill must not start the computer on a later one first.
    current: Option<Planned>,
    plan: Vec<Planned>,
    /// The helper refused these; the phone makes them when the player gets there.
    failed: HashSet<SentenceId>,
}

struct InFlight {
    key: AudioKey,
    text: String,
    estimated_ms: u64,
    urgent: bool,
    /// Asked for a Prepare range, not the listener's plan.
    prepare: bool,
}

struct FillState {
    link: HelperState,
    target: Option<Target>,
    prepare: Option<PrepareTarget>,
    in_flight: HashMap<u32, InFlight>,
    /// Clips taken out of flight and being written; still counted as asked for.
    storing: HashSet<AudioKey>,
    next_item: u32,
    /// The document whose local read-ahead this use case paused.
    paused: Option<DocumentId>,
    /// Until when the first connection after `start` is waited for (`HELPER_CONNECT_WAIT`).
    first_link_until: Option<Instant>,
    stats: HelperFillStats,
}

impl FillState {
    fn first_link_left(&self) -> Option<Duration> {
        if self.link != HelperState::Connecting {
            return None;
        }
        self.first_link_until?.checked_duration_since(Instant::now()).filter(|left| !left.is_zero())
    }

    fn asked_for(&self, key: &AudioKey) -> bool {
        self.storing.contains(key) || self.in_flight.values().any(|flight| flight.key == *key)
    }

    fn item_of(&self, key: &AudioKey) -> Option<u32> {
        self.in_flight.iter().find(|(_, flight)| flight.key == *key).map(|(item_id, _)| *item_id)
    }

    fn new_item_id(&mut self) -> u32 {
        self.next_item = self.next_item.wrapping_add(1);
        self.next_item
    }

    fn in_flight_ms(&self) -> u64 {
        self.in_flight.values().map(|flight| flight.estimated_ms).sum()
    }
}

/// The helper's events reach the use case without keeping it alive.
struct Events(Weak<HelperFill>);

impl HelperEvents for Events {
    fn on_event(&self, event: HelperEvent) {
        if let Some(fill) = self.0.upgrade() {
            fill.on_event(event);
        }
    }
}

impl HelperFill {
    pub fn new(deps: HelperFillDeps) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            deps,
            state: Mutex::new(FillState {
                link: HelperState::Off,
                target: None,
                prepare: None,
                in_flight: HashMap::new(),
                storing: HashSet::new(),
                next_item: 0,
                paused: None,
                first_link_until: None,
                stats: HelperFillStats::default(),
            }),
            changed: Condvar::new(),
            me: me.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, FillState> {
        lock(&self.state)
    }

    /// Starts helping with the phone's own engine (same id and revision, or no clips at all).
    pub fn start(&self) -> AppResult<()> {
        let engine = &self.deps.engine;
        let wanted = HelperEngine { engine_id: engine.id().to_string(), revision: engine.revision().to_string(), sample_rate: engine.sample_rate() };
        info!(service = NAME, engine_id = %wanted.engine_id, revision = %wanted.revision, "helper started");
        self.lock().first_link_until = Some(Instant::now() + HELPER_CONNECT_WAIT);
        self.on_state(HelperState::Connecting);
        self.deps.helper.open(wanted, Arc::new(Events(self.me.clone())))
    }

    /// Stops helping; the local read-ahead takes over at once.
    pub fn stop(&self) {
        self.deps.helper.close();
        self.on_state(HelperState::Off);
        info!(service = NAME, "helper stopped");
    }

    pub fn status(&self) -> HelperFillStatus {
        let state = self.lock();
        HelperFillStatus { state: state.link.clone(), in_flight: state.in_flight.len(), stats: state.stats }
    }

    pub fn makes_clips(&self) -> bool {
        self.lock().link.makes_clips()
    }

    /// Re-plans from the listener's position (every audio request calls it) and asks for what is missing. Items the
    /// new plan doesn't want any more are cancelled on the helper (a jump wastes at most the item being made).
    pub fn follow(&self, position: ListenerPosition<'_>) {
        let anchor = Anchor { key: position.key.clone(), rate: position.rate, read_skipped: position.read_skipped };
        let mut state = self.lock();
        if state.target.as_ref().is_none_or(|target| target.anchor != anchor) {
            self.replan(&mut state, position, anchor);
        }
        if state.link.makes_clips() || state.first_link_left().is_some() {
            self.pause_local(&mut state);
        } else {
            self.resume_local(&mut state);
        }
        self.top_up(&mut state);
    }

    /// Stops filling `document_id` (removed, or switched to its audiobook file): the helper's queue is dropped.
    pub fn stop_document(&self, document_id: &DocumentId) {
        let mut state = self.lock();
        self.drop_prepare(&mut state, document_id);
        if state.target.as_ref().is_some_and(|target| target.anchor.key.document_id == *document_id) {
            state.target = None;
            self.cancel_on_helper(&mut state);
            state.in_flight.clear();
            self.resume_local(&mut state);
            info!(service = NAME, doc_id = %document_id, "helper fill stopped for the document");
            self.top_up(&mut state);
        }
    }

    fn replan(&self, state: &mut FillState, position: ListenerPosition<'_>, anchor: Anchor) {
        let input = ReadAheadInput {
            document: position.document,
            from: anchor.key.sentence_id,
            rate: anchor.rate,
            read_skipped: anchor.read_skipped,
            minutes: HELPER_AHEAD_MINUTES,
        };
        let plan: Vec<Planned> = plan_read_ahead(input)
            .sentences
            .into_iter()
            .map(|sentence| {
                let text = sentence.spoken_text().to_string();
                Planned { sentence_id: sentence.id, estimated_ms: estimated_ms(&text), text }
            })
            .collect();
        let wanted: HashSet<SentenceId> = plan.iter().map(|planned| planned.sentence_id).collect();
        // The sentence asked for now and urgent items stay: the player is waiting for them. A Prepare range's items
        // don't depend on where the listener is.
        let keep = |flight: &InFlight| {
            flight.prepare
                || (anchor.same_scope(&flight.key)
                    && (flight.urgent || flight.key.sentence_id == anchor.key.sentence_id || wanted.contains(&flight.key.sentence_id)))
        };
        let stale = state.in_flight.values().filter(|flight| !keep(flight)).count();
        let failed = match state.target.take() {
            Some(previous) if previous.anchor.same_scope(&anchor.key) => previous.failed,
            _ => HashSet::new(),
        };
        let planned = plan.len();
        let document_id = anchor.key.document_id.clone();
        info!(
            service = NAME,
            doc_id = %anchor.key.document_id,
            voice_id = %anchor.key.voice_id,
            sentence_id = anchor.key.sentence_id.0,
            rate = anchor.rate,
            planned,
            cancelled = stale,
            "helper fill planned"
        );
        if stale > 0 {
            self.cancel_on_helper(state);
            state.in_flight.retain(|_, flight| keep(flight));
        }
        let current = position
            .document
            .blocks
            .iter()
            .flat_map(|block| block.sentences.iter().chain(block.cue.iter()))
            .find(|sentence| sentence.id == anchor.key.sentence_id)
            .map(|sentence| sentence.spoken_text().to_string())
            .filter(|text| !text.trim().is_empty())
            .map(|text| Planned { sentence_id: anchor.key.sentence_id, estimated_ms: estimated_ms(&text), text });
        state.target = Some(Target { anchor, current, plan, failed });
        if stale > 0 {
            self.resend_in_flight(state);
        }
        if state.paused.as_ref().is_some_and(|paused| *paused != document_id) {
            self.resume_local(state);
        }
    }

    /// After the helper's queue was dropped: asks again for what is still in flight, in the order it is needed
    /// (urgent first, then the listener's plan in playback order, then the Prepare range).
    fn resend_in_flight(&self, state: &mut FillState) {
        let listener: HashMap<SentenceId, usize> =
            state.target.iter().flat_map(|target| target.plan.iter()).enumerate().map(|(index, planned)| (planned.sentence_id, index)).collect();
        let range: HashMap<SentenceId, usize> =
            state.prepare.iter().flat_map(|prepare| prepare.plan.iter()).enumerate().map(|(index, planned)| (planned.sentence_id, index)).collect();
        let mut kept: Vec<(&u32, &InFlight)> = state.in_flight.iter().collect();
        kept.sort_by_key(|(_, flight)| {
            let places = if flight.prepare { &range } else { &listener };
            (!flight.urgent, flight.prepare, places.get(&flight.key.sentence_id).copied().unwrap_or(0))
        });
        let kept: Vec<HelperItem> = kept
            .into_iter()
            .map(|(item_id, flight)| HelperItem { item_id: *item_id, voice_id: flight.key.voice_id.clone(), text: flight.text.clone(), urgent: flight.urgent })
            .collect();
        if !kept.is_empty() && self.send(state, kept).is_err() {
            state.in_flight.clear();
        }
    }

    /// Asks for the nearest planned sentences that aren't cached or asked for, within the flow-control limits: the
    /// listener's own sentence first (urgent), then the listener's plan, then the Prepare range.
    fn top_up(&self, state: &mut FillState) {
        if !state.link.makes_clips() {
            return;
        }
        let mut count = state.in_flight.len();
        let mut in_flight_ms = state.in_flight_ms();
        // (key, text, estimated ms, prepare, urgent)
        let mut wanted: Vec<(AudioKey, String, u64, bool, bool)> = Vec::new();
        if let Some(target) = &state.target
            && let Some(current) = &target.current
            && !target.failed.contains(&current.sentence_id)
        {
            let key = AudioKey { sentence_id: current.sentence_id, ..target.anchor.key.clone() };
            if !state.asked_for(&key) && !self.deps.retention.contains(&key).unwrap_or(false) {
                count += 1;
                in_flight_ms += current.estimated_ms;
                wanted.push((key, current.text.clone(), current.estimated_ms, false, true));
            }
        }
        let listener = state.target.iter().map(|target| (&target.anchor.key, &target.plan, &target.failed, false));
        let range = state.prepare.iter().map(|prepare| (&prepare.scope, &prepare.plan, &prepare.failed, true));
        'plans: for (scope, plan, failed, prepare) in listener.chain(range) {
            for planned in plan {
                if count >= HELPER_MAX_IN_FLIGHT_ITEMS || in_flight_ms >= HELPER_MAX_IN_FLIGHT_MS {
                    break 'plans;
                }
                if failed.contains(&planned.sentence_id) {
                    continue;
                }
                let key = AudioKey { sentence_id: planned.sentence_id, ..scope.clone() };
                if state.asked_for(&key) || wanted.iter().any(|(other, ..)| *other == key) || self.deps.retention.contains(&key).unwrap_or(false) {
                    continue;
                }
                count += 1;
                in_flight_ms += planned.estimated_ms;
                wanted.push((key, planned.text.clone(), planned.estimated_ms, prepare, false));
            }
        }
        if wanted.is_empty() {
            return;
        }
        let mut items = Vec::with_capacity(wanted.len());
        for (key, text, estimated_ms, prepare, urgent) in wanted {
            let item_id = state.new_item_id();
            items.push(HelperItem { item_id, voice_id: key.voice_id.clone(), text: text.clone(), urgent });
            state.in_flight.insert(item_id, InFlight { key, text, estimated_ms, urgent, prepare });
        }
        let sent: Vec<u32> = items.iter().map(|item| item.item_id).collect();
        if self.send(state, items).is_err() {
            for item_id in sent {
                state.in_flight.remove(&item_id);
            }
        }
    }

    fn send(&self, state: &mut FillState, items: Vec<HelperItem>) -> AppResult<()> {
        let count = items.len() as u64;
        self.deps.helper.make(items).inspect_err(|error| warn!(service = NAME, error = %error, "helper refused items"))?;
        state.stats.sent += count;
        Ok(())
    }

    fn cancel_on_helper(&self, state: &mut FillState) {
        if !state.link.makes_clips() || state.in_flight.is_empty() {
            return;
        }
        state.stats.cancels += 1;
        if let Err(error) = self.deps.helper.cancel_all() {
            warn!(service = NAME, error = %error, "helper cancel failed");
        }
    }

    fn pause_local(&self, state: &mut FillState) {
        let Some(document_id) = state.target.as_ref().map(|target| target.anchor.key.document_id.clone()) else { return };
        if state.paused.as_ref() == Some(&document_id) {
            return;
        }
        self.resume_local(state);
        self.deps.read_ahead.pause_for_helper(&document_id);
        state.paused = Some(document_id);
    }

    fn resume_local(&self, state: &mut FillState) {
        if let Some(document_id) = state.paused.take() {
            self.deps.read_ahead.resume(&document_id);
        }
    }

    fn on_event(&self, event: HelperEvent) {
        match event {
            HelperEvent::State(state) => self.on_state(state),
            HelperEvent::Clip { item_id, audio } => self.on_clip(item_id, audio),
            HelperEvent::ItemFailed { item_id, error } => self.on_failed(item_id, error),
        }
    }

    fn on_state(&self, next: HelperState) {
        let mut state = self.lock();
        if state.link == next {
            return;
        }
        let was_making = state.link.makes_clips();
        info!(service = NAME, from = ?state.link, to = ?next, in_flight = state.in_flight.len(), stats = ?state.stats, "helper state changed");
        state.link = next;
        if state.link != HelperState::Connecting {
            state.first_link_until = None;
        }
        if state.link.makes_clips() {
            if !was_making {
                self.pause_local(&mut state);
                self.top_up(&mut state);
            }
        } else {
            // The items in flight are gone with the session; the local read-ahead takes over.
            state.in_flight.clear();
            self.resume_local(&mut state);
        }
        drop(state);
        self.changed.notify_all();
    }

    fn on_clip(&self, item_id: u32, audio: Audio) {
        let flight = {
            let mut state = self.lock();
            let Some(flight) = state.in_flight.remove(&item_id) else {
                state.stats.dropped += 1;
                debug!(service = NAME, item_id, "clip for an item no longer asked for dropped");
                return;
            };
            state.storing.insert(flight.key.clone());
            flight
        };
        let stored = self.store(&flight.key, &audio);
        let mut state = self.lock();
        state.storing.remove(&flight.key);
        match stored {
            Ok(true) => state.stats.stored += 1,
            Ok(false) => state.stats.already_cached += 1,
            Err(error) => {
                state.stats.dropped += 1;
                warn!(service = NAME, doc_id = %flight.key.document_id, sentence_id = flight.key.sentence_id.0, error = %error, "helper clip not stored");
            }
        }
        self.top_up(&mut state);
        drop(state);
        self.changed.notify_all();
    }

    /// Writes the clip unless the phone already has one; a clip at another sample rate or for a removed book is refused.
    fn store(&self, key: &AudioKey, audio: &Audio) -> AppResult<bool> {
        let expected = self.deps.engine.sample_rate();
        if audio.sample_rate != expected {
            return Err(AppError::new(ErrorName::HelperIncompatible).with_detail(format!("clip at {} Hz, expected {expected} Hz", audio.sample_rate)));
        }
        if matches!(self.deps.reader.find_summary(&key.document_id), Ok(None)) {
            return Err(AppError::new(ErrorName::DocumentNotFound).with_detail("the document was removed while its clip was made"));
        }
        let bytes = self.deps.encoder.encode(audio)?;
        let stored = self.deps.cache.put_if_absent(key, &bytes)?;
        debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, voice_id = %key.voice_id, bytes = bytes.len(), stored, "helper clip cached");
        Ok(stored)
    }

    fn on_failed(&self, item_id: u32, error: ErrorName) {
        let mut state = self.lock();
        let Some(flight) = state.in_flight.remove(&item_id) else { return };
        state.stats.failed += 1;
        warn!(service = NAME, doc_id = %flight.key.document_id, sentence_id = flight.key.sentence_id.0, error = %error, "helper could not make an item");
        if flight.prepare {
            if let Some(prepare) = state.prepare.as_mut().filter(|prepare| same_scope(&prepare.scope, &flight.key)) {
                prepare.failed.insert(flight.key.sentence_id);
            }
        } else if let Some(target) = state.target.as_mut().filter(|target| target.anchor.same_scope(&flight.key)) {
            target.failed.insert(flight.key.sentence_id);
        }
        self.top_up(&mut state);
        drop(state);
        self.changed.notify_all();
    }
}

fn estimated_ms(text: &str) -> u64 {
    chars_to_ms(text.chars().count() as u64)
}
