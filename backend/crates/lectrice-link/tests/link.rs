//! The whole link in one process: pairing, sessions, clips, refusals, priorities (file 24 §13).

mod support;

use std::thread;
use std::time::Duration;

use lectrice_link::client::{ClientEvent, ClosedReason, Session, pair};
use lectrice_link::server::{GatePriority, PairingStatus, ServerEvent};
use lectrice_link::{ByeReason, CancelScope, LinkError, Make, Open, PairingCode, PairingFailure, decode_clip, encode_clip, pcm16_from_f32};
use support::*;

const WAIT: Duration = Duration::from_secs(5);

fn expected_clip(item_id: u32) -> Vec<i16> {
    fake_samples(&sentence(item_id)).into_iter().map(pcm16_from_f32).collect()
}

#[test]
fn pairs_then_makes_clips_identical_to_local_pcm() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    assert_eq!(computer.peers.peers.lock().unwrap().len(), 1);
    assert!(matches!(computer.server().pairing_status(), PairingStatus::Paired { .. }));

    let session = phone.connect(computer.address).unwrap();
    assert_eq!(session.welcome().computer_name, "Test laptop");
    let ready = session.open(open_request(), WAIT).unwrap();
    assert_eq!((ready.sample_rate, ready.engine_id.as_str()), (SAMPLE_RATE, ENGINE_ID));

    session.make(Make { batch_id: 1, items: (1..=20).map(|id| item(id, false)).collect() }).unwrap();
    let mut seen = Vec::new();
    while seen.len() < 20 {
        match next_result(&session, WAIT).expect("a clip") {
            ClientEvent::Clip { item_id, samples, sample_rate, .. } => {
                assert_eq!(sample_rate, SAMPLE_RATE);
                assert_eq!(samples, expected_clip(item_id), "item {item_id} is bit-exact");
                seen.push(item_id);
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(seen, (1..=20).collect::<Vec<_>>(), "in order, each once");
    assert_eq!(session.in_flight(), 0);
    assert!(computer.events.any(|event| matches!(event, ServerEvent::SessionStarted { .. })));
}

/// Clips that arrive one right after the other, for longer than `dead_after`: the phone's read never times out, so
/// it has to send its pings between clips too, or the computer takes it for gone in the middle of the stream.
#[test]
fn a_phone_that_only_listens_through_a_long_stream_stays_connected() {
    let computer = Computer::start(Duration::from_millis(60));
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);

    // 32 items at 60 ms each: about 2 s of clips without a 200 ms pause, with `dead_after` at 1 s.
    session.make(Make { batch_id: 1, items: (1..=32).map(|id| item(id, false)).collect() }).unwrap();
    for expected in 1..=32 {
        match next_result(&session, WAIT) {
            Some(ClientEvent::Clip { item_id, .. }) => assert_eq!(item_id, expected),
            other => panic!("clip {expected}: {other:?}"),
        }
    }
    assert_eq!(computer.server().sessions().len(), 1, "still the same session");
}

#[test]
fn a_long_clip_crosses_several_chunks() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    let long = lectrice_link::Item { item_id: 9, voice_id: VOICE.into(), text: "word ".repeat(300), urgent: true };
    let expected: Vec<i16> = fake_samples(&long.text).into_iter().map(pcm16_from_f32).collect();
    assert!(encode_clip(&expected, SAMPLE_RATE).unwrap().len() > 2 * 60_000, "the test clip needs 3+ chunks");
    session.make(Make { batch_id: 1, items: vec![long] }).unwrap();
    match next_result(&session, WAIT) {
        Some(ClientEvent::Clip { item_id: 9, samples, .. }) => assert_eq!(samples, expected),
        other => panic!("{other:?}"),
    }
}

#[test]
fn three_wrong_codes_close_the_window_and_it_stays_closed() {
    let computer = Computer::start(Duration::ZERO);
    let phone = Phone::new("Test phone");
    let code = computer.open_pairing();
    let wrong = PairingCode::parse(&format!("{:06}", (code.as_str().parse::<u32>().unwrap() + 1) % 1_000_000)).unwrap();
    for _ in 0..3 {
        assert!(matches!(pair(computer.address, &wrong, &phone.keys, &phone.config), Err(LinkError::PairingFailed(PairingFailure::WrongCode))));
    }
    assert!(wait_until(WAIT, || matches!(computer.server().pairing_status(), PairingStatus::TooManyWrongCodes { .. })));
    // Even the right code is refused now: only the computer's user opens a new window.
    assert!(matches!(pair(computer.address, &code, &phone.keys, &phone.config), Err(LinkError::PairingFailed(PairingFailure::NoCodeShowing))));
    assert!(computer.peers.peers.lock().unwrap().is_empty());
}

#[test]
fn pairing_is_refused_while_no_code_is_showing() {
    let computer = Computer::start(Duration::ZERO);
    let phone = Phone::new("Test phone");
    let result = pair(computer.address, &PairingCode::parse("123456").unwrap(), &phone.keys, &phone.config);
    assert!(matches!(result, Err(LinkError::PairingFailed(PairingFailure::NoCodeShowing))), "{result:?}");
}

#[test]
fn an_unpaired_phone_is_refused_and_a_forgotten_one_is_told() {
    let computer = Computer::start(Duration::ZERO);
    let mut stranger = Phone::new("Stranger");
    stranger.computer_key = Some(*computer.keys.public());
    assert!(matches!(stranger.connect(computer.address), Err(LinkError::NotPaired)));

    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    let peer_id = computer.peers.peers.lock().unwrap().keys().next().cloned().unwrap();
    assert!(computer.server().forget(&peer_id).unwrap());
    assert_eq!(closed_reason(&session), ClosedReason::Bye(ByeReason::Unpaired));
    assert!(matches!(phone.connect(computer.address), Err(LinkError::NotPaired)));
}

#[test]
fn a_session_with_the_wrong_computer_key_fails_the_handshake() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    phone.computer_key = Some(*lectrice_link::StaticKeys::generate().unwrap().public());
    assert!(phone.connect(computer.address).is_err());
}

#[test]
fn another_model_or_revision_is_refused_with_the_reason() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.connect(computer.address).unwrap();
    let other_model = Open { engine_id: "kokoro-v1.0".into(), ..open_request() };
    assert!(matches!(session.open(other_model, WAIT), Err(LinkError::ModelMissing { bundle_id }) if bundle_id == "kokoro-en-v1_0-fp32"));
    let other_revision = Open { revision: "sherpa-onnx-1.12/kokoro-1".into(), ..open_request() };
    assert!(matches!(session.open(other_revision, WAIT), Err(LinkError::Incompatible(_))));
    assert!(computer.events.any(|event| matches!(event, ServerEvent::Refused { .. })));
    // Items before a successful Open fail, so the phone makes them itself.
    session.make(Make { batch_id: 1, items: vec![item(1, false)] }).unwrap();
    assert!(matches!(next_result(&session, WAIT), Some(ClientEvent::ItemFailed { item_id: 1, .. })));
}

#[test]
fn unknown_voices_and_engine_failures_come_back_as_item_failures() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    let unknown_voice = lectrice_link::Item { voice_id: "zz_nobody".into(), ..item(1, false) };
    let failing = lectrice_link::Item { text: "FAIL here".into(), ..item(2, false) };
    session.make(Make { batch_id: 1, items: vec![unknown_voice, failing, item(3, false)] }).unwrap();
    let mut results = Vec::new();
    for _ in 0..3 {
        results.push(match next_result(&session, WAIT).unwrap() {
            ClientEvent::ItemFailed { item_id, error } => (item_id, error.name),
            ClientEvent::Clip { item_id, .. } => (item_id, "clip".into()),
            other => panic!("{other:?}"),
        });
    }
    results.sort();
    assert_eq!(results, vec![(1, "VoiceUnknown".into()), (2, "SpeechSynthesisFailed".into()), (3, "clip".into())]);
}

#[test]
fn cancel_drops_queued_items_and_the_one_being_made() {
    let computer = Computer::start(Duration::from_millis(60));
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    session.make(Make { batch_id: 1, items: (1..=10).map(|id| item(id, false)).collect() }).unwrap();
    thread::sleep(Duration::from_millis(30));
    session.cancel(CancelScope::Batch(1)).unwrap();
    session.make(Make { batch_id: 2, items: vec![item(100, true)] }).unwrap();
    match next_result(&session, WAIT) {
        Some(ClientEvent::Clip { item_id: 100, .. }) => {}
        other => panic!("{other:?}"),
    }
    assert!(next_result(&session, Duration::from_millis(300)).is_none(), "nothing from the cancelled batch");
    assert!(computer.engine.made.load(std::sync::atomic::Ordering::SeqCst) <= 3, "at most the item being made was wasted");
}

#[test]
fn urgent_items_jump_the_queue() {
    let computer = Computer::start(Duration::from_millis(20));
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    session.make(Make { batch_id: 1, items: (1..=8).map(|id| item(id, false)).collect() }).unwrap();
    session.make(Make { batch_id: 2, items: vec![item(50, true)] }).unwrap();
    let order: Vec<u32> = (0..9)
        .map(|_| match next_result(&session, WAIT).unwrap() {
            ClientEvent::Clip { item_id, .. } => item_id,
            other => panic!("{other:?}"),
        })
        .collect();
    let urgent_at = order.iter().position(|id| *id == 50).unwrap();
    assert!(urgent_at <= 2, "urgent came out at {urgent_at}: {order:?}");
}

#[test]
fn the_computers_own_speech_goes_before_phone_items_and_phones_hear_load() {
    let computer = Computer::start(Duration::from_millis(30));
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    let gate = computer.gate.clone();
    let own = thread::spawn(move || {
        for _ in 0..5 {
            let _turn = gate.acquire(GatePriority::OwnNow);
            thread::sleep(Duration::from_millis(30));
        }
    });
    session.make(Make { batch_id: 1, items: (1..=5).map(|id| item(id, false)).collect() }).unwrap();
    let mut load = None;
    let mut clips = 0;
    while clips < 5 {
        match session.next_event(WAIT).unwrap() {
            ClientEvent::Load(seen) => load = Some(seen),
            ClientEvent::Clip { .. } => clips += 1,
            other => panic!("{other:?}"),
        }
    }
    own.join().unwrap();
    assert!(load.is_some_and(|load| load.own_listener), "the phone was told the computer is busy");
}

#[test]
fn two_phones_take_turns() {
    let computer = Computer::start(Duration::from_millis(15));
    let mut first = Phone::new("First");
    let mut second = Phone::new("Second");
    first.pair_with(&computer, computer.address);
    second.pair_with(&computer, computer.address);
    let a = first.open_session(computer.address);
    let b = second.open_session(computer.address);
    a.make(Make { batch_id: 1, items: (1..=6).map(|id| item(id, false)).collect() }).unwrap();
    b.make(Make { batch_id: 1, items: (101..=106).map(|id| item(id, false)).collect() }).unwrap();
    for session in [&a, &b] {
        for _ in 0..6 {
            assert!(matches!(next_result(session, WAIT), Some(ClientEvent::Clip { .. })));
        }
    }
    let log = computer.engine.log.lock().unwrap().clone();
    let first_six: Vec<bool> = log.iter().take(6).map(|text| text.contains("number 1") && !text.contains("number 10")).collect();
    assert!(first_six.iter().any(|from_a| *from_a) && first_six.iter().any(|from_a| !*from_a), "both phones served early: {log:?}");
}

#[test]
fn engine_switch_and_shutdown_say_why() {
    let mut computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let session = phone.open_session(computer.address);
    computer.server().close_sessions_using(ENGINE_ID, ByeReason::EngineSwitch);
    assert_eq!(closed_reason(&session), ClosedReason::Bye(ByeReason::EngineSwitch));

    let session = phone.open_session(computer.address);
    computer.server.take().unwrap().shutdown(ByeReason::HelperOff);
    assert_eq!(closed_reason(&session), ClosedReason::Bye(ByeReason::HelperOff));
    assert!(phone.connect(computer.address).is_err(), "nothing listens any more");
}

#[test]
fn the_phone_says_bye_and_the_computer_sees_it() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    phone.open_session(computer.address).close(ByeReason::Idle);
    assert!(wait_until(WAIT, || computer
        .events
        .any(|event| matches!(event, ServerEvent::SessionEnded { end: lectrice_link::server::SessionEnd::Bye(ByeReason::Idle), .. }))));
}

#[test]
fn public_addresses_are_refused_by_the_client() {
    let phone = Phone::new("Test phone");
    let result = Session::connect("8.8.8.8:47878".parse().unwrap(), &phone.keys, &[0; 32], &phone.config);
    assert!(matches!(result, Err(LinkError::NotLocal)));
}

/// The phone's shell opens the socket (to bind it to the Wi-Fi network); the link still checks where it leads.
struct CountingSockets {
    calls: std::sync::atomic::AtomicUsize,
    /// Connects here instead of the address asked for (a broken or hostile factory).
    redirect: Option<std::net::SocketAddr>,
}

impl lectrice_link::client::SocketFactory for CountingSockets {
    fn connect(&self, address: std::net::SocketAddr, timeout: Duration) -> std::io::Result<std::net::TcpStream> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::net::TcpStream::connect_timeout(&self.redirect.unwrap_or(address), timeout)
    }
}

#[test]
fn sessions_use_the_shells_socket_factory_and_check_where_it_connected() {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    let key = phone.computer_key.unwrap();
    let sockets = CountingSockets { calls: Default::default(), redirect: None };

    let session = Session::connect_via(&sockets, computer.address, &phone.keys, &key, &phone.config).unwrap();
    session.open(open_request(), WAIT).unwrap();
    session.make(Make { batch_id: 1, items: vec![item(1, false)] }).unwrap();
    assert!(matches!(next_result(&session, WAIT), Some(ClientEvent::Clip { item_id: 1, .. })));
    assert_eq!(sockets.calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    let other = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let redirecting = CountingSockets { calls: Default::default(), redirect: Some(other.local_addr().unwrap()) };
    let result = Session::connect_via(&redirecting, computer.address, &phone.keys, &key, &phone.config);
    assert!(matches!(result, Err(LinkError::NotLocal)));

    let refused = Session::connect_via(&sockets, "8.8.8.8:47878".parse().unwrap(), &phone.keys, &key, &phone.config);
    assert!(matches!(refused, Err(LinkError::NotLocal)));
    assert_eq!(sockets.calls.load(std::sync::atomic::Ordering::SeqCst), 1, "a public address never reaches the factory");
}

#[test]
fn reconnects_follow_the_schedule() {
    let seconds: Vec<u64> = (1..=8).map(|failures| lectrice_link::limits::reconnect_delay(failures).as_secs()).collect();
    assert_eq!(seconds, [1, 2, 5, 10, 30, 60, 60, 60]);
    assert_eq!(lectrice_link::limits::reconnect_delay(0).as_secs(), 1);
}

#[test]
fn clip_helpers_are_lossless() {
    let pcm = expected_clip(1);
    assert_eq!(decode_clip(&encode_clip(&pcm, SAMPLE_RATE).unwrap(), SAMPLE_RATE, pcm.len() as u64).unwrap(), pcm);
}

fn closed_reason(session: &Session) -> ClosedReason {
    loop {
        if let ClientEvent::Closed(reason) = session.next_event(WAIT).expect("the session closes") {
            return reason;
        }
    }
}
