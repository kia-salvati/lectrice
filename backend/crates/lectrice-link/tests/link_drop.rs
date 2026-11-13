//! Link-drop tests through `FaultyLink` (file 24 §13): cuts, stalls, cuts mid-chunk, a slow and jittery link. The
//! phone side here is a small filler loop like `HelperFill`: keep a few items in flight, reconnect after a drop, ask
//! again for what didn't arrive. The player and virtual-clock assertions (1 000 runs) are in `lectrice-app`'s
//! `speech/helper_sim.rs`; the real `HelperFill` over this link is `lectrice-infra/tests/helper_link.rs`.

mod support;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use lectrice_link::client::{ClientEvent, ClosedReason, Session};
use lectrice_link::testing::{Direction, Faults, FaultyLink};
use lectrice_link::{Make, pcm16_from_f32};
use support::*;

const IN_FLIGHT: usize = 6;

/// What the phone's cache would hold: the first clip per item wins; a second must be identical.
#[derive(Default)]
struct Cache {
    clips: HashMap<u32, Vec<i16>>,
    duplicates: usize,
}

impl Cache {
    fn put(&mut self, item_id: u32, samples: Vec<i16>) {
        let expected: Vec<i16> = fake_samples(&sentence(item_id)).into_iter().map(pcm16_from_f32).collect();
        assert_eq!(samples, expected, "item {item_id}: never a partial or wrong clip");
        if let Some(existing) = self.clips.get(&item_id) {
            assert_eq!(existing, &samples);
            self.duplicates += 1;
        } else {
            self.clips.insert(item_id, samples);
        }
    }
}

/// Fills items `1..=count` through `address`, reconnecting after drops, until all are cached or `budget` runs out.
fn fill(phone: &Phone, address: std::net::SocketAddr, count: u32, budget: Duration, mut on_tick: impl FnMut(&Cache)) -> (Cache, usize) {
    let deadline = Instant::now() + budget;
    let mut cache = Cache::default();
    let mut reconnects = 0;
    let mut batch = 0;
    while cache.clips.len() < count as usize && Instant::now() < deadline {
        on_tick(&cache);
        let Ok(session) = phone.connect(address) else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        if session.open(open_request(), Duration::from_secs(2)).is_err() {
            continue;
        }
        reconnects += 1;
        run_session(&session, &mut cache, count, deadline, &mut batch, &mut on_tick);
    }
    (cache, reconnects)
}

fn run_session(session: &Session, cache: &mut Cache, count: u32, deadline: Instant, batch: &mut u32, on_tick: &mut impl FnMut(&Cache)) {
    let mut asked: Vec<u32> = Vec::new();
    loop {
        on_tick(cache);
        if cache.clips.len() == count as usize || Instant::now() >= deadline {
            return;
        }
        asked.retain(|id| !cache.clips.contains_key(id));
        if asked.len() < IN_FLIGHT / 2 {
            let wanted: Vec<u32> = (1..=count).filter(|id| !cache.clips.contains_key(id) && !asked.contains(id)).take(IN_FLIGHT - asked.len()).collect();
            if !wanted.is_empty() {
                *batch += 1;
                if session.make(Make { batch_id: *batch, items: wanted.iter().map(|id| item(*id, false)).collect() }).is_err() {
                    return;
                }
                asked.extend(wanted);
            }
        }
        match session.next_event(Duration::from_millis(100)) {
            Some(ClientEvent::Clip { item_id, samples, .. }) => cache.put(item_id, samples),
            Some(ClientEvent::Closed(_)) => return,
            Some(ClientEvent::ItemFailed { item_id, .. }) => asked.retain(|id| *id != item_id),
            Some(ClientEvent::Load(_)) | None => {}
        }
    }
}

fn paired_through_proxy(delay: Duration) -> (Computer, Phone, FaultyLink) {
    let computer = Computer::start(delay);
    let link = FaultyLink::start(computer.address).unwrap();
    let mut phone = Phone::new("Test phone");
    phone.pair_with(&computer, link.address());
    (computer, phone, link)
}

#[test]
fn random_cuts_never_lose_corrupt_or_split_a_clip() {
    let mut seed = 0x1234_5678_u64;
    for run in 0..12 {
        let (_computer, phone, link) = paired_through_proxy(Duration::from_millis(2));
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        // Cut once the phone has 1–30 clips, for 100–400 ms.
        let cut_after_clips = 1 + (seed % 30) as usize;
        let cut_for = Duration::from_millis(100 + (seed >> 8) % 300);
        let mut cut_since: Option<Instant> = None;
        let mut done = false;
        let (cache, reconnects) = fill(&phone, link.address(), 40, Duration::from_secs(20), |cache| {
            if !done && cut_since.is_none() && cache.clips.len() >= cut_after_clips {
                link.cut();
                cut_since = Some(Instant::now());
            } else if !done && cut_since.is_some_and(|since| since.elapsed() >= cut_for) {
                link.heal();
                done = true;
            }
        });
        assert_eq!(cache.clips.len(), 40, "run {run}: everything arrived after the cut");
        assert!(reconnects >= 2, "run {run}: the cut ended a session");
    }
}

#[test]
fn a_cut_in_the_middle_of_a_clip_yields_no_partial_clip() {
    let (_computer, phone, link) = paired_through_proxy(Duration::ZERO);
    let session = phone.open_session(link.address());
    link.set(Faults { cut_after_bytes_to_phone: Some(3_000), ..Faults::default() });
    let long = lectrice_link::Item { item_id: 7, voice_id: VOICE.into(), text: "word ".repeat(200), urgent: false };
    session.make(Make { batch_id: 1, items: vec![long] }).unwrap();
    loop {
        match session.next_event(Duration::from_secs(5)).expect("the session ends") {
            ClientEvent::Clip { .. } => panic!("a clip cut mid-way must never be handed over"),
            ClientEvent::Closed(_) => break,
            _ => {}
        }
    }
    link.heal();
    let (cache, _) = fill(&phone, link.address(), 5, Duration::from_secs(10), |_| {});
    assert_eq!(cache.clips.len(), 5, "after the cut the link works again");
}

#[test]
fn a_stalled_link_is_noticed_within_the_dead_time() {
    let (_computer, phone, link) = paired_through_proxy(Duration::ZERO);
    let session = phone.open_session(link.address());
    link.set(Faults { stall: true, ..Faults::default() });
    let stalled_at = Instant::now();
    let reason = loop {
        if let ClientEvent::Closed(reason) = session.next_event(Duration::from_secs(5)).expect("the session ends") {
            break reason;
        }
    };
    assert_eq!(reason, ClosedReason::Silent);
    let noticed = stalled_at.elapsed();
    assert!(noticed <= timing().dead_after + Duration::from_millis(600), "noticed after {noticed:?}");
}

#[test]
fn a_one_way_cut_towards_the_phone_ends_the_session() {
    let (_computer, phone, link) = paired_through_proxy(Duration::ZERO);
    let session = phone.open_session(link.address());
    link.set(Faults { cut: Some(Direction::ToPhone), ..Faults::default() });
    session.make(Make { batch_id: 1, items: vec![item(1, false)] }).ok();
    let ended = (0..60).any(|_| matches!(session.next_event(Duration::from_millis(100)), Some(ClientEvent::Closed(_))));
    assert!(ended);
}

#[test]
fn a_slow_jittery_link_still_delivers_every_clip_intact() {
    let (_computer, phone, link) = paired_through_proxy(Duration::ZERO);
    // 128 kbit/s with 300 ± 200 ms per chunk: slower than the dead time allows, so pings must keep it alive.
    link.set(Faults { throttle: Some(16_000), latency: Duration::from_millis(30), jitter: Duration::from_millis(20), ..Faults::default() });
    let (cache, _) = fill(&phone, link.address(), 6, Duration::from_secs(30), |_| {});
    assert_eq!(cache.clips.len(), 6);
    assert_eq!(cache.duplicates, 0);
}
