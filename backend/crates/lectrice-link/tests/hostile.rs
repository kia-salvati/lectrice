//! Malicious peers (security review pass 3): a paired phone, a stranger on the network, and a paired computer that
//! misbehave must not crash the other side, grow its memory, hold its slots or get more than the protocol gives.

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lectrice_link::client::{ClientEvent, ClosedReason, Session};
use lectrice_link::limits::{MAX_HALF_OPEN_PER_ADDRESS, MAX_ITEMS_PER_BATCH, MAX_TEXT_BYTES, PHONE_ITEMS_PER_MINUTE, SESSION_PROLOGUE};
use lectrice_link::server::{PairingStatus, ServerEvent};
use lectrice_link::testing::{RawComputer, RawPhone};
use lectrice_link::{
    ByeReason, ClientMessage, ClipChunk, EngineOffer, Item, Load, Make, Open, Ready, ServerMessage, StaticKeys, Welcome, decode_client_message, encode_clip,
};
use support::*;

const WAIT: Duration = Duration::from_secs(10);

fn paired() -> (Computer, Phone) {
    let computer = Computer::start(Duration::ZERO);
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, computer.address);
    (computer, phone)
}

fn raw_session(computer: &Computer, phone: &Phone) -> RawPhone {
    let (raw, answer) = RawPhone::connect(computer.address, &phone.keys, phone.computer_key.as_ref().unwrap(), "Test phone").unwrap();
    assert!(matches!(answer, ServerMessage::Welcome(_)), "{answer:?}");
    raw
}

fn session_ended(computer: &Computer) -> bool {
    computer.events.any(|event| matches!(event, ServerEvent::SessionEnded { .. }))
}

fn long_item(item_id: u32) -> Item {
    Item { item_id, voice_id: VOICE.into(), text: format!("{item_id} {}", "word ".repeat(MAX_TEXT_BYTES / 5 - 4)), urgent: false }
}

/// Someone on the path between the phone and the computer: passes bytes on and can send the phone's last chunk again.
struct Recorder {
    address: SocketAddr,
    last_to_computer: Arc<Mutex<Vec<u8>>>,
    to_computer: Arc<Mutex<Option<TcpStream>>>,
}

impl Recorder {
    fn between(computer: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let last_to_computer = Arc::new(Mutex::new(Vec::new()));
        let to_computer = Arc::new(Mutex::new(None));
        let (last, upstream_slot) = (last_to_computer.clone(), to_computer.clone());
        thread::spawn(move || {
            let (mut phone_side, _) = listener.accept().unwrap();
            let mut computer_side = TcpStream::connect(computer).unwrap();
            *upstream_slot.lock().unwrap() = Some(computer_side.try_clone().unwrap());
            let (mut back_from, mut back_to) = (computer_side.try_clone().unwrap(), phone_side.try_clone().unwrap());
            thread::spawn(move || {
                let _ = std::io::copy(&mut back_from, &mut back_to);
                let _ = back_to.shutdown(std::net::Shutdown::Both);
            });
            let mut chunk = [0u8; 4096];
            while let Ok(read @ 1..) = phone_side.read(&mut chunk) {
                *last.lock().unwrap() = chunk[..read].to_vec();
                if computer_side.write_all(&chunk[..read]).is_err() {
                    break;
                }
            }
        });
        Self { address, last_to_computer, to_computer }
    }

    fn replay_last_frame_to_computer(&mut self) {
        let frame = self.last_to_computer.lock().unwrap().clone();
        assert!(!frame.is_empty());
        self.to_computer.lock().unwrap().as_mut().unwrap().write_all(&frame).unwrap();
    }
}

// ---- A paired phone that misbehaves ----

#[test]
fn a_phone_that_floods_pings_and_never_reads_is_dropped() {
    let (computer, phone) = paired();
    let mut raw = raw_session(&computer, &phone);
    let started = Instant::now();
    let mut sent = 0u64;
    // Every ping gets a pong the phone never reads: the computer must end the session, not queue pongs for ever.
    while !session_ended(&computer) && started.elapsed() < WAIT {
        if raw.send(&ClientMessage::Ping { t_ms: sent }).is_err() {
            break;
        }
        sent += 1;
    }
    assert!(wait_until(WAIT, || session_ended(&computer)), "the session ended after {sent} unread pongs");
    assert!(computer.server().sessions().is_empty());
}

#[test]
fn a_phone_that_never_reads_its_clips_stops_getting_them_made() {
    let (computer, phone) = paired();
    let mut raw = raw_session(&computer, &phone);
    raw.send(&ClientMessage::Open(open_request())).unwrap();
    let items = (MAX_ITEMS_PER_BATCH * 3) as u32;
    for batch in 0..3u32 {
        let first = batch * MAX_ITEMS_PER_BATCH as u32;
        raw.send(&ClientMessage::Make(Make { batch_id: batch, items: (first..first + MAX_ITEMS_PER_BATCH as u32).map(long_item).collect() })).unwrap();
    }
    // Each clip is about 1 MB of FLAC. Unread, they fill the socket and then the backlog; the rest are refused
    // without being made, or the session ends because writes stall. Either way the engine stops early.
    let made = || computer.engine.made.load(Ordering::SeqCst);
    assert!(wait_until(WAIT, || computer.server().sessions().first().is_none_or(|session| session.queued == 0 && !session.making)));
    assert!(made() < items as usize / 2, "{} of {items} clips were made for a phone that reads nothing", made());
}

#[test]
fn items_past_the_minute_budget_are_refused_as_helper_busy() {
    let (computer, phone) = paired();
    let mut raw = raw_session(&computer, &phone);
    raw.send(&ClientMessage::Open(open_request())).unwrap();
    assert!(matches!(raw.receive(WAIT).unwrap(), ServerMessage::Ready(_)));
    let asked = (PHONE_ITEMS_PER_MINUTE as u32).next_multiple_of(MAX_ITEMS_PER_BATCH as u32) + 2 * MAX_ITEMS_PER_BATCH as u32;
    for first in (0..asked).step_by(MAX_ITEMS_PER_BATCH) {
        raw.send(&ClientMessage::Make(Make { batch_id: first, items: (first..first + MAX_ITEMS_PER_BATCH as u32).map(|id| item(id, true)).collect() }))
            .unwrap();
    }
    let (mut clips, mut busy) = (0u32, 0u32);
    while clips + busy < asked {
        match raw.receive(WAIT).unwrap() {
            ServerMessage::Clip(ClipChunk { last: true, .. }) => clips += 1,
            ServerMessage::ItemFailed { error, .. } => {
                assert_eq!((error.name.as_str(), error.code), ("HelperBusy", 5006));
                busy += 1;
            }
            _ => {}
        }
        // A hand-driven phone has to show it is alive itself, or the computer takes it for gone.
        raw.send(&ClientMessage::Ping { t_ms: 0 }).unwrap();
    }
    assert_eq!((clips, busy), (PHONE_ITEMS_PER_MINUTE as u32, asked - PHONE_ITEMS_PER_MINUTE as u32));
}

#[test]
fn garbage_inside_the_channel_ends_only_that_session() {
    let (computer, phone) = paired();
    let mut other = Phone::new("Other phone");
    other.pair_with(&computer, computer.address);
    let healthy = other.open_session(computer.address);

    let oversized_name = ClientMessage::Hello(lectrice_link::Hello { proto_minor: 0, app_version: "1".into(), device_name: "x".repeat(500) });
    let second_hello = ClientMessage::Hello(lectrice_link::Hello { proto_minor: 0, app_version: "1".into(), device_name: "Phone".into() });
    let empty_make = ClientMessage::Make(Make { batch_id: 1, items: Vec::new() });
    let control_name = ClientMessage::Hello(lectrice_link::Hello { proto_minor: 0, app_version: "1".into(), device_name: "Pho\u{202e}ne\n".into() });
    assert!(decode_client_message(&[0xff; 64]).is_err());
    for attack in 0..6 {
        let mut raw = raw_session(&computer, &phone);
        match attack {
            0 => raw.send_plaintext(&[0xff; 64]).unwrap(),
            1 => raw.send(&oversized_name).unwrap(),
            2 => raw.send(&second_hello).unwrap(),
            3 => raw.send(&empty_make).unwrap(),
            4 => raw.send(&control_name).unwrap(),
            // A forged frame: not encrypted with the session keys.
            _ => raw.send_raw_frame(&[7u8; 48]).unwrap(),
        }
        assert!(raw.is_closed_within(WAIT), "attack {attack} ends the session");
    }
    healthy.make(Make { batch_id: 1, items: vec![item(1, true)] }).unwrap();
    assert!(matches!(next_result(&healthy, WAIT), Some(ClientEvent::Clip { item_id: 1, .. })), "the other phone is still served");
}

#[test]
fn a_replayed_or_reordered_frame_ends_the_session() {
    let (computer, phone) = paired();
    let mut recorder = Recorder::between(computer.address);
    let (mut raw, _) = RawPhone::connect(recorder.address, &phone.keys, phone.computer_key.as_ref().unwrap(), "Test phone").unwrap();
    raw.send(&ClientMessage::Ping { t_ms: 1 }).unwrap();
    assert!(matches!(raw.receive(WAIT).unwrap(), ServerMessage::Pong { t_ms: 1 }));
    // The attacker on the path sends the phone's last frame (the ping) once more.
    recorder.replay_last_frame_to_computer();
    assert!(raw.is_closed_within(WAIT), "a replayed frame fails to decrypt and the session ends");
    assert!(wait_until(WAIT, || computer.server().sessions().is_empty()));
}

// ---- A stranger on the network ----

#[test]
fn a_replayed_handshake_gets_no_session_and_frees_its_slot() {
    let (computer, phone) = paired();
    let original = RawPhone::handshake(computer.address, &phone.keys, phone.computer_key.as_ref().unwrap()).unwrap();
    let mut replay = TcpStream::connect(computer.address).unwrap();
    replay.write_all(SESSION_PROLOGUE).unwrap();
    replay.write_all(original.first_handshake_frame()).unwrap();
    // The computer answers the handshake (it can't tell yet), but without the phone's key no `Hello` can follow.
    replay.write_all(&[0, 20]).unwrap();
    replay.write_all(&[9u8; 20]).unwrap();
    replay.set_read_timeout(Some(WAIT)).unwrap();
    let mut rest = Vec::new();
    let _ = replay.read_to_end(&mut rest);
    assert!(computer.server().sessions().is_empty());
    assert!(!computer.events.any(|event| matches!(event, ServerEvent::SessionStarted { .. })));
    drop(original);
    // The slots are free again: the real phone connects.
    assert!(wait_until(WAIT, || phone.connect(computer.address).is_ok()));
}

#[test]
fn one_address_cannot_hold_every_half_open_slot() {
    let (computer, _phone) = paired();
    let stallers: Vec<TcpStream> = (0..MAX_HALF_OPEN_PER_ADDRESS + 3).map(|_| TcpStream::connect(computer.address).unwrap()).collect();
    thread::sleep(Duration::from_millis(300));
    let refused = stallers
        .iter()
        .filter(|stream| {
            stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
            let mut byte = [0u8; 1];
            // Closed by the computer at once: end of stream (or a reset).
            !matches!((&**stream).read(&mut byte), Err(ref error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
        })
        .count();
    assert_eq!(refused, 3, "connections past the per-address limit are closed at once");
}

#[test]
fn stalled_and_garbage_pairing_attempts_are_not_wrong_codes_and_leave_no_phone() {
    let computer = Computer::start(Duration::ZERO);
    let code = computer.open_pairing();
    for garbage in [&b""[..], &[0, 3, 1, 2, 3][..], &[0xff; 40][..]] {
        let mut stream = TcpStream::connect(computer.address).unwrap();
        stream.write_all(lectrice_link::limits::PAIR_PROLOGUE).unwrap();
        stream.write_all(garbage).unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
    }
    assert!(wait_until(WAIT, || matches!(computer.server().pairing_status(), PairingStatus::Showing { wrong_codes: 0, ref trying, .. } if trying.is_empty())));
    assert!(computer.peers.peers.lock().unwrap().is_empty());
    // The real phone still pairs with the same code.
    let phone = Phone::new("Test phone");
    assert!(lectrice_link::client::pair(computer.address, &code, &phone.keys, &phone.config).is_ok());
}

#[test]
fn an_unpaired_key_is_told_so_and_then_backed_off() {
    let (computer, phone) = paired();
    let stranger = StaticKeys::generate().unwrap();
    for _ in 0..3 {
        let (_, answer) = RawPhone::connect(computer.address, &stranger, phone.computer_key.as_ref().unwrap(), "Stranger").unwrap();
        assert!(matches!(answer, ServerMessage::Bye(ByeReason::Unpaired)), "{answer:?}");
    }
    // The fourth try from this address is closed before the handshake.
    assert!(RawPhone::connect(computer.address, &stranger, phone.computer_key.as_ref().unwrap(), "Stranger").is_err());
    assert!(computer.server().sessions().is_empty());
}

// ---- A paired computer that misbehaves ----

fn welcome() -> Welcome {
    let offer = EngineOffer { engine_id: ENGINE_ID.into(), revision: REVISION.into(), installed: true, sample_rate: SAMPLE_RATE };
    Welcome { proto_minor: 0, app_version: "0.2.0".into(), computer_name: "Test laptop".into(), engines: vec![offer] }
}

fn ready(sample_rate: u32) -> ServerMessage {
    ServerMessage::Ready(Ready { engine_id: ENGINE_ID.into(), revision: REVISION.into(), sample_rate, voices: vec![VOICE.into()] })
}

/// A phone session against a hand-driven computer; `drive` runs on the computer's side.
fn against_raw_computer(drive: impl FnOnce(lectrice_link::testing::RawComputerSession) + Send + 'static) -> (Session, thread::JoinHandle<()>) {
    let keys = StaticKeys::generate().unwrap();
    let computer = RawComputer::bind(keys.clone()).unwrap();
    let address = computer.address().unwrap();
    let driver = thread::spawn(move || drive(computer.accept(welcome()).unwrap()));
    let phone = Phone::new("Test phone");
    let session = Session::connect(address, &phone.keys, keys.public(), &phone.config).unwrap();
    (session, driver)
}

fn closed_reason(session: &Session) -> ClosedReason {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if let Some(ClientEvent::Closed(reason)) = session.next_event(Duration::from_millis(100)) {
            return reason;
        }
    }
    panic!("the session didn't close");
}

#[test]
fn a_made_up_sample_rate_is_refused_before_any_clip_is_sized_by_it() {
    for rate in [0, 7_999, 48_001, u32::MAX] {
        let (session, driver) = against_raw_computer(move |mut computer| {
            let _ = computer.receive(WAIT);
            let _ = computer.send(&ready(rate));
            let _ = computer.receive(WAIT);
        });
        assert!(session.open(open_request(), Duration::from_secs(2)).is_err(), "{rate} Hz");
        assert!(matches!(closed_reason(&session), ClosedReason::Broken(_)));
        drop(session);
        driver.join().unwrap();
    }
}

#[test]
fn a_clip_longer_than_its_text_allows_is_refused() {
    let (session, driver) = against_raw_computer(|mut computer| {
        let _ = computer.receive(WAIT);
        computer.send(&ready(SAMPLE_RATE)).unwrap();
        let _ = computer.receive(WAIT);
        // A short sentence may be 10 s long; this clip claims 11 s.
        let samples = SAMPLE_RATE * 11;
        let flac = encode_clip(&vec![0i16; samples as usize], SAMPLE_RATE).unwrap();
        let _ = computer.send(&ServerMessage::Clip(ClipChunk { item_id: 1, sample_rate: SAMPLE_RATE, samples, make_ms: 1, seq: 0, last: true, flac }));
        let _ = computer.receive(WAIT);
    });
    session.open(open_request(), WAIT).unwrap();
    session.make(Make { batch_id: 1, items: vec![item(1, true)] }).unwrap();
    assert!(matches!(closed_reason(&session), ClosedReason::Broken(_)));
    drop(session);
    driver.join().unwrap();
}

#[test]
fn a_clip_that_lies_about_its_length_or_never_ends_is_refused() {
    for attack in 0..3 {
        let (session, driver) = against_raw_computer(move |mut computer| {
            let _ = computer.receive(WAIT);
            computer.send(&ready(SAMPLE_RATE)).unwrap();
            let _ = computer.receive(WAIT);
            let flac = encode_clip(&vec![100i16; 24_000], SAMPLE_RATE).unwrap();
            let chunk = |seq: u16, samples: u32, last: bool, flac: Vec<u8>| {
                ServerMessage::Clip(ClipChunk { item_id: 1, sample_rate: SAMPLE_RATE, samples, make_ms: 1, seq, last, flac })
            };
            match attack {
                // The header says 1 000 samples, the FLAC holds 24 000.
                0 => drop(computer.send(&chunk(0, 1_000, true, flac))),
                // Chunks out of order.
                1 => drop(computer.send(&chunk(1, 24_000, true, flac))),
                // Never-ending chunks: more bytes than a 10 s clip can need.
                _ => {
                    for seq in 0..40u16 {
                        if computer.send(&chunk(seq, 24_000, false, vec![0u8; 60_000])).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = computer.receive(WAIT);
        });
        session.open(open_request(), WAIT).unwrap();
        session.make(Make { batch_id: 1, items: vec![item(1, true)] }).unwrap();
        assert!(matches!(closed_reason(&session), ClosedReason::Broken(_)), "attack {attack}");
        drop(session);
        driver.join().unwrap();
    }
}

#[test]
fn unasked_clips_and_failures_never_reach_the_app() {
    let (session, driver) = against_raw_computer(|mut computer| {
        let _ = computer.receive(WAIT);
        computer.send(&ready(SAMPLE_RATE)).unwrap();
        let flac = encode_clip(&vec![100i16; 2_400], SAMPLE_RATE).unwrap();
        for item_id in 100..120 {
            let clip = ClipChunk { item_id, sample_rate: SAMPLE_RATE, samples: 2_400, make_ms: 1, seq: 0, last: true, flac: flac.clone() };
            computer.send(&ServerMessage::Clip(clip)).unwrap();
            computer.send(&ServerMessage::ItemFailed { item_id, error: lectrice_link::ItemError { name: "Internal".into(), code: 9999 } }).unwrap();
        }
        computer.send(&ServerMessage::Bye(ByeReason::Quit)).unwrap();
    });
    session.open(open_request(), WAIT).unwrap();
    let mut events = Vec::new();
    while let Some(event) = session.next_event(WAIT) {
        let closed = matches!(event, ClientEvent::Closed(_));
        events.push(event);
        if closed {
            break;
        }
    }
    assert_eq!(events, vec![ClientEvent::Closed(ClosedReason::Bye(ByeReason::Quit))]);
    driver.join().unwrap();
}

#[test]
fn floods_of_open_replies_and_load_messages_are_not_queued() {
    let (session, driver) = against_raw_computer(|mut computer| {
        for _ in 0..2_000 {
            if computer.send(&ServerMessage::Load(Load { queue_items: 1, own_listener: true })).is_err() {
                return;
            }
        }
        for _ in 0..50 {
            if computer.send(&ready(SAMPLE_RATE)).is_err() {
                return;
            }
        }
        let _ = computer.receive(WAIT);
    });
    // The app is slow to look: meanwhile only a few `Load`s are kept, and the unasked `Open` answers end the session
    // instead of piling up.
    thread::sleep(Duration::from_secs(1));
    let mut loads = 0;
    let reason = loop {
        match session.next_event(WAIT) {
            Some(ClientEvent::Load(_)) => loads += 1,
            Some(ClientEvent::Closed(reason)) => break reason,
            Some(other) => panic!("{other:?}"),
            None => panic!("the session didn't close"),
        }
    };
    assert!(matches!(reason, ClosedReason::Broken(_)), "{reason:?}");
    assert!((1..=8).contains(&loads), "{loads} of 2000 Load messages were queued");
    drop(session);
    driver.join().unwrap();
}

#[test]
fn an_open_answer_for_another_engine_is_the_callers_to_refuse() {
    // The link hands the answer over; `LinkSpeechHelper` (lectrice-infra) compares it with the phone's engine.
    let (session, driver) = against_raw_computer(|mut computer| {
        let _ = computer.receive(WAIT);
        let other = Ready { engine_id: "another-engine".into(), revision: "r9".into(), sample_rate: 22_050, voices: Vec::new() };
        let _ = computer.send(&ServerMessage::Ready(other));
        let _ = computer.receive(WAIT);
    });
    let answer = session.open(Open { engine_id: ENGINE_ID.into(), revision: REVISION.into(), voice_ids: Vec::new() }, WAIT).unwrap();
    assert_eq!(answer.engine_id, "another-engine");
    drop(session);
    driver.join().unwrap();
}
