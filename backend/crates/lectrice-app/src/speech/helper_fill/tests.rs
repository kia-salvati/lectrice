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
    worker: Arc<SpeechWorker>,
    speak: SpeakSentence,
}

fn setup_with(engine: GatedEngine, grace: Duration) -> Setup {
    setup_waiting(engine, HelperWait { grace, cold_grace: grace })
}

fn setup_waiting(engine: GatedEngine, wait: HelperWait) -> Setup {
    let HelperWait { grace, cold_grace } = wait;
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
        worker: worker.clone(),
        reader: library.reader(),
        cache: cache.clone(),
        encoder,
        read_ahead: read_ahead.clone(),
        helper: Some(fill.clone()),
        helper_wait: HelperWait { grace, cold_grace },
    });
    fill.start().unwrap();
    Setup { library, engine, cache, helper, read_ahead, fill, worker, speak }
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

/// The listener plays `sentence` (so it is cached, as when a player follows along).
fn follow(setup: &Setup, document: &Document, sentence: u32) {
    let key = key(document, sentence);
    if !setup.cache.contains(&key).unwrap() {
        setup.cache.put(&key, b"playing").unwrap();
    }
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

/// Nearest first, at most 12 items in flight; each clip lets one more go; the local engine never runs.
#[test]
fn fills_nearest_first_within_the_flow_control_and_keeps_the_local_engine_idle() {
    let setup = connected();
    let document = book(&setup, 2, 100, 1);
    setup.cache.put(&key(&document, 3), b"x").unwrap();

    follow(&setup, &document, 0);

    let first = setup.helper.take_made();
    assert_eq!(sentences(&first), [1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    assert!(setup.read_ahead.is_paused(&document.id), "the local read-ahead is paused for this book");
    for item in &first[..3] {
        setup.helper.deliver(item);
    }
    assert_eq!(sentences(&setup.helper.take_made()), [14, 15, 16]);
    assert_eq!(setup.fill.status().stats.stored, 3);
    assert!(setup.cache.contains(&key(&document, 2)).unwrap());
    assert!(setup.engine.spoken().is_empty(), "the phone's engine never speaks while the helper fills");
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

/// Lost gives the book back to the local read-ahead; reconnecting takes it again and asks anew.
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

/// A book switched to its audiobook file, or removed, stops being filled.
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

/// A miss while connected is sent urgent and played from the helper within the grace.
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
    assert!(setup.engine.spoken().is_empty(), "the phone's engine was never needed");
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
    setup.worker.wait_until_idle();
    assert_eq!(setup.engine.spoken().len(), 1);
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

/// Play while the first connection is still under way. The miss waits for the link instead of
/// starting the phone's engine, and the local read-ahead stays paused meanwhile.
#[test]
fn a_miss_while_the_first_link_connects_waits_for_it() {
    let setup = Arc::new(setup_waiting(GatedEngine::cold(), HelperWait::default()));
    let document = book(&setup, 2, 100, 1);
    let computer = {
        let setup = setup.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            setup.helper.connect();
            wait_until(|| setup.helper.made().iter().any(|item| item.urgent));
            let urgent = setup.helper.made().into_iter().find(|item| item.urgent).unwrap();
            setup.helper.deliver(&urgent);
            urgent
        })
    };

    let bytes = speak(&setup, &document, 50).unwrap();

    let urgent = computer.join().unwrap();
    assert_eq!(sentence_of(&urgent), 50);
    assert_eq!(bytes, format!("{SAMPLE_RATE}:{}", urgent.text.len() * 100).into_bytes());
    assert!(setup.engine.spoken().is_empty(), "the phone's engine never ran");
    assert!(!setup.engine.is_loaded());
    assert_eq!(setup.helper.made().iter().filter(|item| item.urgent).count(), 1, "the listener's sentence is asked for once");
}

#[test]
fn the_local_read_ahead_waits_for_the_first_link_but_not_for_a_failed_one() {
    let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
    let document = book(&setup, 2, 100, 1);

    follow(&setup, &document, 0);
    assert!(setup.read_ahead.is_paused(&document.id));
    assert!(setup.fill.first_link_left().is_some());

    setup.helper.emit(HelperEvent::State(HelperState::Lost));
    assert!(!setup.read_ahead.is_paused(&document.id));
    setup.helper.emit(HelperEvent::State(HelperState::Connecting));
    assert_eq!(setup.fill.first_link_left(), None, "a reconnect is not the first connection");
    follow(&setup, &document, 1);
    assert!(!setup.read_ahead.is_paused(&document.id));
}

/// A first connection that takes too long: after the connect wait the phone makes the sentence itself.
#[test]
fn the_connect_wait_ends() {
    let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
    let document = book(&setup, 2, 100, 1);

    let started = Instant::now();
    speak(&setup, &document, 50).unwrap();

    let waited = started.elapsed();
    assert!(waited >= HELPER_CONNECT_WAIT - Duration::from_millis(50) && waited < WAIT_LIMIT, "{waited:?}");
    assert!(setup.engine.spoken()[0].starts_with("0050"));
    assert!(!setup.read_ahead.is_paused(&document.id));
}

/// With the phone's engine still unloaded, a computer that answers after the normal grace still
/// wins (the cold grace), and the model is never loaded.
#[test]
fn an_unloaded_engine_waits_the_cold_grace() {
    let wait = HelperWait { grace: Duration::from_millis(20), cold_grace: WAIT_LIMIT };
    let setup = Arc::new(setup_waiting(GatedEngine::cold(), wait));
    setup.helper.connect();
    let document = book(&setup, 2, 100, 1);
    let computer = {
        let setup = setup.clone();
        std::thread::spawn(move || {
            wait_until(|| setup.helper.made().iter().any(|item| item.urgent));
            std::thread::sleep(Duration::from_millis(300));
            let urgent = setup.helper.made().into_iter().find(|item| item.urgent).unwrap();
            setup.helper.deliver(&urgent);
        })
    };

    speak(&setup, &document, 50).unwrap();

    computer.join().unwrap();
    assert!(setup.engine.spoken().is_empty());
}

/// The link becomes ready while the player waits: the listener's sentence is the first item, urgent, ahead of
/// the fill (so a cold computer doesn't start on a later sentence first).
#[test]
fn the_listeners_sentence_goes_first_when_the_link_becomes_ready() {
    let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
    let document = book(&setup, 2, 100, 1);
    let waited_for = key(&document, 40);
    setup.fill.follow(ListenerPosition { document: &document, key: &waited_for, rate: 1.0, read_skipped: false });

    setup.helper.connect();

    let made = setup.helper.take_made();
    assert_eq!(sentence_of(&made[0]), 40);
    assert!(made[0].urgent && made[1..].iter().all(|item| !item.urgent));
    assert_eq!(made.len(), HELPER_MAX_IN_FLIGHT_ITEMS);
}

fn range(document: &Document, from: u32, to: u32) -> Vec<(SentenceId, String)> {
    document.blocks.iter().flat_map(|block| &block.sentences).filter(|sentence| (from..=to).contains(&sentence.id.0)).map(|s| (s.id, s.text.clone())).collect()
}

/// A Prepare range goes to the helper after the listener's plan, survives a lost link, and is
/// counted from the cache.
#[test]
fn a_prepare_range_is_made_by_the_helper_after_the_listeners_plan() {
    let setup = connected();
    let document = book(&setup, 2, 15, 1);
    let other = book(&setup, 3, 40, 1);

    follow(&setup, &document, 0);
    let listener = setup.helper.take_made();
    assert_eq!(listener.len(), HELPER_MAX_IN_FLIGHT_ITEMS);
    let progress = setup.fill.prepare(key(&other, 0), range(&other, 20, 29)).unwrap();

    assert_eq!((progress.ready, progress.total, progress.running), (0, 10, true));
    assert!(setup.helper.take_made().is_empty(), "the listener's plan fills the flow control first");
    for item in &listener {
        setup.helper.deliver(item);
    }
    let made = setup.helper.take_made();
    assert_eq!(sentences(&made), [13, 14, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29], "the rest of the listener's plan, then the range");
    assert!(setup.fill.prepare_progress(&other.id).unwrap().running);

    setup.helper.emit(HelperEvent::State(HelperState::Lost));
    assert!(setup.fill.prepare_progress(&other.id).is_some(), "the range waits for the helper");
    setup.helper.connect();
    assert!(setup.fill.stop_prepare(&other.id));
    assert!(setup.fill.prepare_progress(&other.id).is_none());
    assert!(setup.engine.spoken().is_empty(), "the phone's engine never ran");
}

#[test]
fn a_prepare_range_alone_fills_and_finishes() {
    let setup = connected();
    let document = book(&setup, 3, 40, 1);
    setup.cache.put(&key(&document, 12), b"x").unwrap();

    let progress = setup.fill.prepare(key(&document, 0), range(&document, 10, 19)).unwrap();
    assert_eq!(progress.ready, 1);
    let made = setup.helper.take_made();
    assert_eq!(sentences(&made), [10, 11, 13, 14, 15, 16, 17, 18, 19]);
    for item in &made {
        setup.helper.deliver(item);
    }

    let progress = setup.fill.prepare_progress(&document.id).unwrap();
    assert_eq!((progress.ready, progress.total, progress.running), (10, 10, false));
}

#[test]
fn a_jump_keeps_the_prepare_items_and_a_refused_one_ends_the_range() {
    let setup = connected();
    let document = book(&setup, 3, 40, 1);
    setup.fill.prepare(key(&document, 0), range(&document, 10, 12)).unwrap();
    let made = setup.helper.take_made();

    let listener_book = book(&setup, 2, 400, 1);
    follow(&setup, &listener_book, 0);
    follow(&setup, &listener_book, 200);
    let resent = setup.helper.take_made();
    assert!(made.iter().all(|item| resent.iter().any(|other| other.item_id == item.item_id)), "prepare items asked again after the cancel");

    setup.helper.deliver(&made[0]);
    setup.helper.deliver(&made[1]);
    setup.helper.emit(HelperEvent::ItemFailed { item_id: made[2].item_id, error: ErrorName::HelperBusy });

    let progress = setup.fill.prepare_progress(&document.id).unwrap();
    assert_eq!((progress.ready, progress.total, progress.running), (2, 3, false));
}

#[test]
fn prepare_jobs_send_the_range_to_the_helper_and_pin_it() {
    use crate::speech::prepare::{PrepareArgs, PrepareDeps, PrepareJobs};
    let setup = connected();
    let document = book(&setup, 3, 40, 1);
    let jobs = PrepareJobs::new(PrepareDeps {
        engine: setup.engine.clone(),
        worker: Arc::new(SpeechWorker::start(setup.engine.clone()).unwrap()),
        reader: setup.library.reader(),
        cache: setup.cache.clone(),
        retention: setup.cache.clone(),
        encoder: Arc::new(CountingEncoder),
        helper: Some(setup.fill.clone()),
    });
    let args =
        || PrepareArgs { document_id: document.id.clone(), voice_id: VoiceId::new("af_heart"), read_skipped: false, from: SentenceId(5), to: SentenceId(7) };

    let status = jobs.start_on_helper(args()).unwrap();

    assert_eq!((status.total, status.running), (3, true));
    assert!(jobs.on_helper(&document.id));
    assert_eq!(setup.cache.pins(), [(document.id.clone(), 5, 7)]);
    for item in &setup.helper.take_made() {
        setup.helper.deliver(item);
    }
    assert_eq!(jobs.status(&document.id).ready, 3);
    jobs.cancel(&document.id).unwrap();
    assert!(!jobs.on_helper(&document.id));
    assert!(setup.cache.pins().is_empty());
    assert!(setup.engine.spoken().is_empty());

    setup.helper.emit(HelperEvent::State(HelperState::Lost));
    assert_eq!(jobs.start_on_helper(args()).unwrap_err().primary(), ErrorName::HelperUnreachable);
}

#[test]
fn prepare_needs_a_connected_helper() {
    let setup = setup_with(GatedEngine::open(), WAIT_LIMIT);
    let document = book(&setup, 3, 40, 1);

    let refused = setup.fill.prepare(key(&document, 0), range(&document, 0, 3)).unwrap_err();

    assert_eq!(refused.primary(), ErrorName::HelperUnreachable);
}

#[test]
fn the_lost_horizon_depends_on_the_phone() {
    assert_eq!(lost_read_ahead_minutes(Some(0.25), 2.0, 10), 10, "green: the buffer lasts, wait with the setting");
    assert_eq!(lost_read_ahead_minutes(Some(0.38), 2.0, 10), 30, "amber at 2x");
    assert_eq!(lost_read_ahead_minutes(Some(1.11), 1.0, 10), 30, "red");
    assert_eq!(lost_read_ahead_minutes(None, 1.0, 10), 30, "no stats yet");
}
