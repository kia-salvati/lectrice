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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioCache, AudioEncoder, AudioKey, AudioRetention, DocumentId, DocumentReader, ErrorName, HelperEngine, HelperEvent,
    HelperEvents, HelperItem, HelperState, SentenceId, SpeechEngine, SpeechHelper,
};
use tracing::{debug, info, warn};

use super::read_ahead::{ListenerPosition, ReadAhead};
use super::read_ahead_plan::{ReadAheadInput, chars_to_ms, plan_read_ahead};
use crate::Cancellation;

const NAME: &str = "HelperFill";

/// How far ahead the helper fills, in listening minutes (file 24 §3.6, user 2026-11-13).
pub const HELPER_AHEAD_MINUTES: u32 = 30;
/// Flow control (file 24 §3.5): at most this many items, or this much estimated 1x audio, in flight at once.
pub const HELPER_MAX_IN_FLIGHT_ITEMS: usize = 12;
pub const HELPER_MAX_IN_FLIGHT_MS: u64 = 3 * 60 * 1000;
/// How long a sentence the player needs now waits for the helper before the phone makes it itself (H0: the
/// laptop's p95 for a 10-word sentence is 1.09 s).
pub const HELPER_NOW_GRACE: Duration = Duration::from_millis(1500);
/// A phone whose sustained `rtf × rate` is at most this keeps up alone ("green", file 12 §3.3).
pub const GREEN_RTF_TIMES_RATE: f32 = 0.6;
/// How often a waiting caller checks whether it was cancelled.
const WAIT_POLL: Duration = Duration::from_millis(100);

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
        let own = &self.key;
        own.document_id == key.document_id && own.cleaner_version == key.cleaner_version && own.engine_id == key.engine_id && own.voice_id == key.voice_id
    }

    fn key_for(&self, sentence_id: SentenceId) -> AudioKey {
        AudioKey { sentence_id, ..self.key.clone() }
    }
}

struct Planned {
    sentence_id: SentenceId,
    text: String,
    estimated_ms: u64,
}

struct Target {
    anchor: Anchor,
    plan: Vec<Planned>,
    /// The helper refused these; the phone makes them when the player gets there.
    failed: HashSet<SentenceId>,
}

struct InFlight {
    key: AudioKey,
    text: String,
    estimated_ms: u64,
    urgent: bool,
}

struct FillState {
    link: HelperState,
    target: Option<Target>,
    in_flight: HashMap<u32, InFlight>,
    /// Clips taken out of flight and being written; still counted as asked for.
    storing: HashSet<AudioKey>,
    next_item: u32,
    /// The document whose local read-ahead this use case paused.
    paused: Option<DocumentId>,
    stats: HelperFillStats,
}

impl FillState {
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
                in_flight: HashMap::new(),
                storing: HashSet::new(),
                next_item: 0,
                paused: None,
                stats: HelperFillStats::default(),
            }),
            changed: Condvar::new(),
            me: me.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, FillState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts helping with the phone's own engine (same id and revision, or no clips at all).
    pub fn start(&self) -> AppResult<()> {
        let engine = &self.deps.engine;
        let wanted = HelperEngine { engine_id: engine.id().to_string(), revision: engine.revision().to_string(), sample_rate: engine.sample_rate() };
        info!(service = NAME, engine_id = %wanted.engine_id, revision = %wanted.revision, "helper started");
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
        if state.link.makes_clips() {
            self.pause_local(&mut state);
        }
        self.top_up(&mut state);
    }

    /// The player needs this sentence now: it goes to the front of the helper's queue. `None` when the helper can't
    /// make clips; then the caller makes it locally at once.
    pub fn urgent(&self, key: &AudioKey, text: &str) -> Option<UrgentWait> {
        let fill = self.me.upgrade()?;
        let mut state = self.lock();
        if !state.link.makes_clips() {
            return None;
        }
        if state.storing.contains(key) {
            return Some(UrgentWait { fill, key: key.clone() });
        }
        let item_id = match state.item_of(key) {
            Some(item_id) => item_id,
            None => {
                let item_id = state.new_item_id();
                let estimated_ms = estimated_ms(text);
                state.in_flight.insert(item_id, InFlight { key: key.clone(), text: text.to_string(), estimated_ms, urgent: true });
                item_id
            }
        };
        let flight = state.in_flight.get_mut(&item_id)?;
        flight.urgent = true;
        let item = HelperItem { item_id, voice_id: key.voice_id.clone(), text: flight.text.clone(), urgent: true };
        if let Err(error) = self.send(&mut state, vec![item]) {
            debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "urgent item not sent");
            return None;
        }
        debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, item_id, "urgent item sent");
        Some(UrgentWait { fill, key: key.clone() })
    }

    /// Stops filling `document_id` (removed, or switched to its audiobook file): the helper's queue is dropped.
    pub fn stop_document(&self, document_id: &DocumentId) {
        let mut state = self.lock();
        if state.target.as_ref().is_some_and(|target| target.anchor.key.document_id == *document_id) {
            state.target = None;
            self.cancel_on_helper(&mut state);
            state.in_flight.clear();
            self.resume_local(&mut state);
            info!(service = NAME, doc_id = %document_id, "helper fill stopped for the document");
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
        // The sentence asked for now and urgent items stay: the player is waiting for them.
        let keep = |flight: &InFlight| {
            anchor.same_scope(&flight.key) && (flight.urgent || flight.key.sentence_id == anchor.key.sentence_id || wanted.contains(&flight.key.sentence_id))
        };
        let stale = state.in_flight.values().filter(|flight| !keep(flight)).count();
        let failed = match state.target.take() {
            Some(previous) if previous.anchor.same_scope(&anchor.key) => previous.failed,
            _ => HashSet::new(),
        };
        if stale > 0 {
            self.cancel_on_helper(state);
            state.in_flight.retain(|_, flight| keep(flight));
            // Asked again in the order they are needed: urgent first, then in playback order.
            let place: HashMap<SentenceId, usize> = plan.iter().enumerate().map(|(index, planned)| (planned.sentence_id, index)).collect();
            let mut kept: Vec<(&u32, &InFlight)> = state.in_flight.iter().collect();
            kept.sort_by_key(|(_, flight)| (!flight.urgent, place.get(&flight.key.sentence_id).copied().unwrap_or(0)));
            let kept: Vec<HelperItem> = kept
                .into_iter()
                .map(|(item_id, flight)| HelperItem {
                    item_id: *item_id,
                    voice_id: flight.key.voice_id.clone(),
                    text: flight.text.clone(),
                    urgent: flight.urgent,
                })
                .collect();
            if !kept.is_empty() && self.send(state, kept).is_err() {
                state.in_flight.clear();
            }
        }
        if state.paused.as_ref().is_some_and(|paused| *paused != anchor.key.document_id) {
            self.resume_local(state);
        }
        info!(
            service = NAME,
            doc_id = %anchor.key.document_id,
            voice_id = %anchor.key.voice_id,
            sentence_id = anchor.key.sentence_id.0,
            rate = anchor.rate,
            planned = plan.len(),
            cancelled = stale,
            in_flight = state.in_flight.len(),
            "helper fill planned"
        );
        state.target = Some(Target { anchor, plan, failed });
    }

    /// Asks for the nearest planned sentences that aren't cached or asked for, within the flow-control limits.
    fn top_up(&self, state: &mut FillState) {
        if !state.link.makes_clips() {
            return;
        }
        let Some(target) = state.target.as_ref() else { return };
        let mut count = state.in_flight.len();
        let mut in_flight_ms = state.in_flight_ms();
        let mut wanted = Vec::new();
        for planned in &target.plan {
            if count >= HELPER_MAX_IN_FLIGHT_ITEMS || in_flight_ms >= HELPER_MAX_IN_FLIGHT_MS {
                break;
            }
            if target.failed.contains(&planned.sentence_id) {
                continue;
            }
            let key = target.anchor.key_for(planned.sentence_id);
            if state.asked_for(&key) || self.deps.retention.contains(&key).unwrap_or(false) {
                continue;
            }
            count += 1;
            in_flight_ms += planned.estimated_ms;
            wanted.push((key, planned.text.clone(), planned.estimated_ms));
        }
        if wanted.is_empty() {
            return;
        }
        let mut items = Vec::with_capacity(wanted.len());
        for (key, text, estimated_ms) in wanted {
            let item_id = state.new_item_id();
            items.push(HelperItem { item_id, voice_id: key.voice_id.clone(), text: text.clone(), urgent: false });
            state.in_flight.insert(item_id, InFlight { key, text, estimated_ms, urgent: false });
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
        if let Some(target) = state.target.as_mut().filter(|target| target.anchor.same_scope(&flight.key)) {
            target.failed.insert(flight.key.sentence_id);
        }
        self.top_up(&mut state);
        drop(state);
        self.changed.notify_all();
    }
}

/// A sentence asked for urgently; the caller waits for it, or races it against the local engine.
pub struct UrgentWait {
    fill: Arc<HelperFill>,
    key: AudioKey,
}

impl UrgentWait {
    /// The clip, if the helper's (or anyone's) is cached now.
    pub fn ready(&self) -> AppResult<Option<Vec<u8>>> {
        if self.fill.lock().storing.contains(&self.key) || !self.fill.deps.retention.contains(&self.key)? {
            return Ok(None);
        }
        self.fill.deps.cache.get(&self.key)
    }

    /// Waits up to `grace` for the clip. `None` when it didn't come in time or the link went; `RequestCancelled` once
    /// the caller gives up.
    pub fn wait(&self, grace: Duration, cancellation: &Cancellation) -> AppResult<Option<Vec<u8>>> {
        let deadline = Instant::now() + grace;
        loop {
            if let Some(bytes) = self.ready()? {
                return Ok(Some(bytes));
            }
            if cancellation.is_cancelled() {
                return Err(AppError::new(ErrorName::RequestCancelled).with_detail("the caller stopped waiting for the helper"));
            }
            let state = self.fill.lock();
            if !state.link.makes_clips() {
                return Ok(None);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let _ = self.fill.changed.wait_timeout(state, (deadline - now).min(WAIT_POLL)).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

fn estimated_ms(text: &str) -> u64 {
    chars_to_ms(text.chars().count() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RequestContext;
    use crate::UseCase;
    use crate::documents::fakes::{Library, id_for};
    use crate::speech::fakes::{CountingEncoder, ENGINE_ID, FakeHelper, GatedEngine, MemoryCache, REVISION, SAMPLE_RATE};
    use crate::speech::read_ahead::{ReadAheadDeps, ReadAheadSettings};
    use crate::speech::speak_sentence::{SpeakSentence, SpeakSentenceArgs, SpeakSentenceDeps};
    use crate::speech::worker::SpeechWorker;
    use lectrice_domain::{Block, BlockKind, Clock, Document, DocumentMeta, DocumentWriter, Sentence, VoiceId};

    const WAIT_LIMIT: Duration = Duration::from_secs(5);

    struct Setup {
        library: Library,
        engine: Arc<GatedEngine>,
        cache: Arc<MemoryCache>,
        helper: Arc<FakeHelper>,
        read_ahead: Arc<ReadAhead>,
        fill: Arc<HelperFill>,
        speak: SpeakSentence,
    }

    fn setup_with(engine: GatedEngine, grace: Duration) -> Setup {
        let library = Library::new(1);
        let engine = Arc::new(engine);
        let cache = Arc::new(MemoryCache::default());
        let helper = Arc::new(FakeHelper::default());
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
        let encoder = Arc::new(CountingEncoder);
        let settings = ReadAheadSettings { read_ahead_minutes: 10, keep_minutes_per_document: 60 };
        let read_ahead_deps =
            ReadAheadDeps { worker: worker.clone(), reader: library.reader(), cache: cache.clone(), retention: cache.clone(), encoder: encoder.clone() };
        let read_ahead = Arc::new(ReadAhead::new(read_ahead_deps, settings).unwrap());
        let fill = HelperFill::new(HelperFillDeps {
            helper: helper.clone(),
            engine: engine.clone(),
            reader: library.reader(),
            cache: cache.clone(),
            retention: cache.clone(),
            encoder: encoder.clone(),
            read_ahead: read_ahead.clone(),
        });
        let speak = SpeakSentence::new(SpeakSentenceDeps {
            engine: engine.clone(),
            worker,
            reader: library.reader(),
            cache: cache.clone(),
            encoder,
            read_ahead: read_ahead.clone(),
            helper: Some(fill.clone()),
            helper_grace: grace,
        });
        fill.start().unwrap();
        Setup { library, engine, cache, helper, read_ahead, fill, speak }
    }

    fn connected() -> Setup {
        let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
        setup.helper.connect();
        setup
    }

    /// `sentences` sentences of `seconds` estimated seconds each (18 characters per second).
    fn book(setup: &Setup, number: u8, sentences: u32, seconds: usize) -> Document {
        let blocks = (0..sentences)
            .map(|id| Block {
                id,
                kind: BlockKind::Paragraph,
                level: None,
                page_index: 1,
                spoken: true,
                text: None,
                sentences: vec![Sentence { id: SentenceId(id), text: format!("{id:04}{}.", "a".repeat(18 * seconds - 5)), page_index: 1, speech_text: None }],
                cue: None,
            })
            .collect();
        let document = Document {
            id: id_for(number),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: Vec::new(),
            toc: Vec::new(),
            blocks,
        };
        let now = setup.library.clock.now();
        setup.library.store.save(&document, b"%PDF-", &DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None }).unwrap();
        document
    }

    fn key(document: &Document, sentence: u32) -> AudioKey {
        AudioKey {
            document_id: document.id.clone(),
            cleaner_version: 1,
            engine_id: ENGINE_ID.into(),
            voice_id: VoiceId::new("af_heart"),
            sentence_id: SentenceId(sentence),
        }
    }

    fn follow(setup: &Setup, document: &Document, sentence: u32) {
        let key = key(document, sentence);
        setup.fill.follow(ListenerPosition { document, key: &key, rate: 1.0, read_skipped: false });
    }

    fn sentence_of(item: &HelperItem) -> u32 {
        item.text[..4].parse().unwrap()
    }

    fn sentences(items: &[HelperItem]) -> Vec<u32> {
        items.iter().map(sentence_of).collect()
    }

    fn speak(setup: &Setup, document: &Document, sentence: u32) -> AppResult<Vec<u8>> {
        let args = SpeakSentenceArgs {
            document_id: document.id.clone(),
            sentence_id: SentenceId(sentence),
            voice_id: VoiceId::new("af_heart"),
            rate: 1.0,
            read_skipped: false,
            plan_ahead: true,
        };
        setup.speak.run(&RequestContext::new("test"), args).map(|spoken| spoken.bytes)
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(started.elapsed() < WAIT_LIMIT, "condition never became true");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn the_helper_is_opened_with_the_phones_own_engine_and_revision() {
        let setup = connected();

        let opened = setup.helper.opened().unwrap();

        assert_eq!(opened, HelperEngine { engine_id: ENGINE_ID.into(), revision: REVISION.into(), sample_rate: SAMPLE_RATE });
    }

    /// C43: nearest first, at most 12 items in flight; each clip lets one more go; the local engine never runs.
    #[test]
    fn fills_nearest_first_within_the_flow_control_and_keeps_the_local_engine_idle() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        setup.cache.put(&key(&document, 3), b"x").unwrap();

        follow(&setup, &document, 0);

        let first = setup.helper.take_made();
        assert_eq!(sentences(&first), [1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
        assert!(setup.read_ahead.is_paused(&document.id), "C44: the local read-ahead is paused for this book");
        for item in &first[..3] {
            setup.helper.deliver(item);
        }
        assert_eq!(sentences(&setup.helper.take_made()), [14, 15, 16]);
        assert_eq!(setup.fill.status().stats.stored, 3);
        assert!(setup.cache.contains(&key(&document, 2)).unwrap());
        assert!(setup.engine.spoken().is_empty(), "C46: the phone's engine never speaks while the helper fills");
    }

    #[test]
    fn three_minutes_of_audio_is_the_other_limit() {
        let setup = connected();
        let document = book(&setup, 2, 200, 30);

        follow(&setup, &document, 0);

        assert_eq!(setup.helper.made().len(), 6);
    }

    #[test]
    fn a_clip_is_stored_once_and_stale_or_odd_clips_are_dropped() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        follow(&setup, &document, 0);
        let made = setup.helper.take_made();

        setup.helper.deliver(&made[0]);
        setup.helper.deliver(&made[0]);
        setup.helper.emit(HelperEvent::Clip { item_id: 999, audio: Audio { samples: vec![0.0; 10], sample_rate: SAMPLE_RATE } });
        setup.helper.emit(HelperEvent::Clip { item_id: made[1].item_id, audio: Audio { samples: vec![0.0; 10], sample_rate: 16_000 } });

        let stats = setup.fill.status().stats;
        assert_eq!((stats.stored, stats.dropped), (1, 3));
        assert_eq!(setup.cache.get(&key(&document, 1)).unwrap().unwrap(), format!("{SAMPLE_RATE}:{}", made[0].text.len() * 100).into_bytes());
        assert!(!setup.cache.contains(&key(&document, 2)).unwrap(), "a clip at another sample rate is never cached");
    }

    /// A jump cancels the helper's queue once (at most the item being made is wasted) and asks from the new place.
    #[test]
    fn a_jump_cancels_the_helpers_queue_and_asks_from_the_new_place() {
        let setup = connected();
        let document = book(&setup, 2, 400, 1);
        follow(&setup, &document, 0);
        let before = setup.helper.take_made();

        follow(&setup, &document, 300);

        assert_eq!(setup.helper.cancels(), 1);
        assert_eq!(sentences(&setup.helper.take_made()), (301..=312).collect::<Vec<_>>());
        setup.helper.deliver(&before[0]);
        assert_eq!(setup.fill.status().stats.dropped, 1, "a clip of the old place is not stored");
        assert!(!setup.cache.contains(&key(&document, 1)).unwrap());
    }

    #[test]
    fn a_small_step_keeps_what_is_in_flight() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        follow(&setup, &document, 0);
        setup.helper.take_made();

        follow(&setup, &document, 1);

        assert_eq!(setup.helper.cancels(), 0);
        assert!(setup.helper.made().is_empty());
        assert_eq!(setup.fill.status().in_flight, 12);
    }

    /// C44: Lost gives the book back to the local read-ahead; reconnecting takes it again and asks anew.
    #[test]
    fn losing_the_link_hands_the_book_to_the_phone_and_reconnecting_takes_it_back() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        follow(&setup, &document, 0);
        setup.helper.take_made();

        setup.helper.emit(HelperEvent::State(HelperState::Lost));

        assert!(!setup.read_ahead.is_paused(&document.id));
        assert_eq!(setup.fill.status().in_flight, 0);
        follow(&setup, &document, 1);
        assert!(setup.helper.made().is_empty(), "nothing is asked while lost");

        setup.helper.connect();

        assert!(setup.read_ahead.is_paused(&document.id));
        assert_eq!(sentences(&setup.helper.take_made()), (2..=13).collect::<Vec<_>>());
    }

    /// File 24 §3.12 / file 21: a book switched to its audiobook file, or removed, stops being filled.
    #[test]
    fn stopping_a_document_drops_the_helpers_queue_and_gives_the_book_back() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        follow(&setup, &document, 0);
        let made = setup.helper.take_made();

        setup.fill.stop_document(&id_for(9));
        assert_eq!(setup.helper.cancels(), 0, "another book changes nothing");
        setup.fill.stop_document(&document.id);

        assert_eq!(setup.helper.cancels(), 1);
        assert_eq!(setup.fill.status().in_flight, 0);
        assert!(!setup.read_ahead.is_paused(&document.id));
        setup.helper.deliver(&made[0]);
        assert!(!setup.cache.contains(&key(&document, 1)).unwrap(), "a late clip is not stored");
    }

    #[test]
    fn items_the_helper_refuses_are_left_to_the_phone() {
        let setup = connected();
        let document = book(&setup, 2, 100, 1);
        follow(&setup, &document, 0);
        let made = setup.helper.take_made();

        setup.helper.emit(HelperEvent::ItemFailed { item_id: made[0].item_id, error: ErrorName::HelperBusy });

        let again = setup.helper.take_made();
        assert_eq!(sentences(&again), [13], "the next sentence goes instead; the refused one is not asked again");
        assert_eq!(setup.fill.status().stats.failed, 1);
    }

    /// C45: a miss while connected is sent urgent and played from the helper within the grace.
    #[test]
    fn a_miss_while_connected_is_made_by_the_helper_within_the_grace() {
        let setup = Arc::new(connected());
        let document = book(&setup, 2, 100, 1);
        let delivering = {
            let setup = setup.clone();
            std::thread::spawn(move || {
                wait_until(|| setup.helper.made().iter().any(|item| item.urgent));
                let urgent = setup.helper.made().into_iter().find(|item| item.urgent).unwrap();
                setup.helper.deliver(&urgent);
                urgent
            })
        };

        let bytes = speak(&setup, &document, 50).unwrap();

        let urgent = delivering.join().unwrap();
        assert_eq!(sentence_of(&urgent), 50);
        assert_eq!(bytes, format!("{SAMPLE_RATE}:{}", urgent.text.len() * 100).into_bytes());
        assert!(setup.engine.spoken().is_empty(), "C46: the phone's engine was never needed");
        assert_eq!(setup.helper.made().iter().filter(|item| item.urgent).count(), 1);
    }

    /// After the grace the phone makes it; the helper's late clip changes nothing (first writer wins).
    #[test]
    fn after_the_grace_the_phone_makes_it_and_a_late_helper_clip_changes_nothing() {
        let setup = setup_with(GatedEngine::open(), Duration::from_millis(20));
        setup.helper.connect();
        let document = book(&setup, 2, 100, 1);

        let bytes = speak(&setup, &document, 50).unwrap();

        assert_eq!(setup.engine.spoken().len(), 1);
        let urgent = setup.helper.made().into_iter().find(|item| item.urgent).unwrap();
        setup.helper.emit(HelperEvent::Clip { item_id: urgent.item_id, audio: Audio { samples: vec![0.0; 7], sample_rate: SAMPLE_RATE } });
        assert_eq!(setup.fill.status().stats.already_cached, 1);
        assert_eq!(setup.cache.get(&key(&document, 50)).unwrap().unwrap(), bytes);
    }

    /// While the phone's engine works on it, a helper clip that arrives first is played and kept.
    #[test]
    fn a_helper_clip_that_beats_the_local_engine_wins() {
        let document_text = format!("0050{}.", "a".repeat(13));
        let setup = Arc::new(setup_with(GatedEngine::new(&document_text), Duration::from_millis(20)));
        setup.helper.connect();
        let document = book(&setup, 2, 100, 1);
        let speaking = {
            let (setup, document) = (setup.clone(), document.clone());
            std::thread::spawn(move || speak(&setup, &document, 50))
        };

        setup.engine.wait_until_blocked();
        let urgent = setup.helper.made().into_iter().find(|item| item.urgent).unwrap();
        setup.helper.emit(HelperEvent::Clip { item_id: urgent.item_id, audio: Audio { samples: vec![0.0; 7], sample_rate: SAMPLE_RATE } });
        let bytes = speaking.join().unwrap().unwrap();
        setup.engine.release();

        assert_eq!(bytes, format!("{SAMPLE_RATE}:7").into_bytes());
        wait_until(|| setup.engine.spoken().len() == 1);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(setup.cache.get(&key(&document, 50)).unwrap().unwrap(), bytes, "the local clip that finished later was refused");
    }

    #[test]
    fn a_miss_while_lost_goes_to_the_phones_engine_at_once() {
        let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
        setup.helper.emit(HelperEvent::State(HelperState::Lost));
        let document = book(&setup, 2, 100, 1);

        let started = Instant::now();
        speak(&setup, &document, 50).unwrap();

        assert!(started.elapsed() < Duration::from_secs(1), "no grace without a helper");
        assert!(setup.engine.spoken()[0].starts_with("0050"), "made locally first; the local read-ahead follows");
        assert!(!setup.read_ahead.is_paused(&document.id));
        assert!(setup.helper.made().is_empty());
    }

    #[test]
    fn the_lost_horizon_depends_on_the_phone() {
        assert_eq!(lost_read_ahead_minutes(Some(0.25), 2.0, 10), 10, "green: the buffer lasts, wait with the setting");
        assert_eq!(lost_read_ahead_minutes(Some(0.38), 2.0, 10), 30, "amber at 2x");
        assert_eq!(lost_read_ahead_minutes(Some(1.11), 1.0, 10), 30, "red");
        assert_eq!(lost_read_ahead_minutes(None, 1.0, 10), 30, "no stats yet");
    }
}
