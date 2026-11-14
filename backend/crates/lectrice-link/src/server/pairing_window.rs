//! The computer's pairing window: opened only by the computer's user,
//! 2 minutes, up to `MAX_PARALLEL_PAIRINGS` attempts at once. Wrong codes are counted per window across every
//! address: the third closes the window and only the user opens a new one. On top, more than
//! `PAIR_FAILURES_PER_HOUR` wrong codes in an hour make "Pair a phone" wait `PAIR_COOLDOWN`. Attempts that stop
//! before the code is checked aren't wrong codes (nothing was learned) and just free their slot.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crate::error::LinkResult;
use crate::identity::PeerRecord;
use crate::limits::{MAX_PARALLEL_PAIRINGS, PAIR_CODE_LIFETIME, PAIR_COOLDOWN, PAIR_FAILURE_WINDOW, PAIR_FAILURES_PER_HOUR, PAIR_MAX_FAILURES};
use crate::pairing::PairingCode;

/// What the computer's UI shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingStatus {
    Closed,
    Showing {
        code: PairingCode,
        expires_in: Duration,
        /// Wrong codes typed in this window so far.
        wrong_codes: u32,
        /// Addresses with an attempt under way now ("who's trying": a stranger's address shows here too).
        trying: Vec<IpAddr>,
    },
    Paired {
        peer: PeerRecord,
    },
    Expired,
    /// The third wrong code closed the window.
    TooManyWrongCodes {
        last_from: Option<IpAddr>,
    },
    /// Too many wrong codes this hour: "Pair a phone" works again after `wait`.
    CoolingDown {
        wait: Duration,
    },
}

enum Window {
    Closed,
    Open { code: PairingCode, opened: Instant, wrong_codes: u32, trying: Vec<IpAddr> },
    Paired(PeerRecord),
    Expired,
    TooManyWrongCodes(Option<IpAddr>),
}

pub(crate) struct PairingWindow {
    window: Window,
    /// When each recent wrong code happened (all windows).
    failures: VecDeque<Instant>,
    cooldown_until: Option<Instant>,
}

/// A pairing attempt allowed to run.
pub(crate) struct Attempt {
    pub code: PairingCode,
    pub from: IpAddr,
}

/// How an attempt ended.
pub(crate) enum AttemptEnd {
    Paired(PeerRecord),
    WrongCode,
    /// Stopped before the code was checked (timeout, disconnect, garbage).
    Abandoned,
}

impl PairingWindow {
    pub fn new() -> Self {
        Self { window: Window::Closed, failures: VecDeque::new(), cooldown_until: None }
    }

    pub fn open(&mut self, now: Instant) -> LinkResult<PairingStatus> {
        if self.cooldown_until.is_none_or(|until| now >= until) {
            self.cooldown_until = None;
            self.window = Window::Open { code: PairingCode::generate()?, opened: now, wrong_codes: 0, trying: Vec::new() };
        } else {
            self.window = Window::Closed;
        }
        Ok(self.status(now))
    }

    pub fn close(&mut self) {
        self.window = Window::Closed;
    }

    /// Starts an attempt if the window is open, a slot is free and this address has no attempt under way.
    pub fn begin(&mut self, from: IpAddr, now: Instant) -> Option<Attempt> {
        self.expire(now);
        let Window::Open { code, trying, .. } = &mut self.window else { return None };
        if trying.len() >= MAX_PARALLEL_PAIRINGS || trying.contains(&from) {
            return None;
        }
        trying.push(from);
        Some(Attempt { code: code.clone(), from })
    }

    /// True while the window this attempt began in is still open (the caller saves a paired phone only then).
    pub fn is_open_for(&self, attempt: &Attempt) -> bool {
        matches!(&self.window, Window::Open { code, .. } if *code == attempt.code)
    }

    pub fn end(&mut self, attempt: &Attempt, end: AttemptEnd, now: Instant) {
        // A wrong code is a guess made, also when its window closed while the attempt ran (attempts in the other
        // slots outlive the third wrong code): it always counts towards the hourly limit.
        if matches!(end, AttemptEnd::WrongCode) && self.count_wrong_code(now) {
            self.cooldown_until = Some(now + PAIR_COOLDOWN);
            self.window = Window::TooManyWrongCodes(Some(attempt.from));
            return;
        }
        let Window::Open { code, wrong_codes, trying, .. } = &mut self.window else { return };
        if *code != attempt.code {
            return;
        }
        if let Some(index) = trying.iter().position(|address| *address == attempt.from) {
            trying.remove(index);
        }
        match end {
            AttemptEnd::Paired(peer) => self.window = Window::Paired(peer),
            AttemptEnd::Abandoned => {}
            AttemptEnd::WrongCode => {
                *wrong_codes += 1;
                if *wrong_codes >= PAIR_MAX_FAILURES {
                    self.window = Window::TooManyWrongCodes(Some(attempt.from));
                }
            }
        }
    }

    /// Notes a wrong code; true when this hour's limit is reached.
    fn count_wrong_code(&mut self, now: Instant) -> bool {
        self.failures.push_back(now);
        while self.failures.front().is_some_and(|at| now.duration_since(*at) > PAIR_FAILURE_WINDOW) {
            self.failures.pop_front();
        }
        self.failures.len() >= PAIR_FAILURES_PER_HOUR
    }

    /// Closes an open window whose code ran out; true when it just expired.
    pub fn expire(&mut self, now: Instant) -> bool {
        if let Window::Open { opened, .. } = &self.window
            && now.duration_since(*opened) >= PAIR_CODE_LIFETIME
        {
            self.window = Window::Expired;
            return true;
        }
        false
    }

    pub fn status(&self, now: Instant) -> PairingStatus {
        if let Some(until) = self.cooldown_until
            && now < until
            && !matches!(self.window, Window::TooManyWrongCodes(_))
        {
            return PairingStatus::CoolingDown { wait: until - now };
        }
        match &self.window {
            Window::Closed => PairingStatus::Closed,
            Window::Open { code, opened, wrong_codes, trying } => PairingStatus::Showing {
                code: code.clone(),
                expires_in: PAIR_CODE_LIFETIME.saturating_sub(now.duration_since(*opened)),
                wrong_codes: *wrong_codes,
                trying: trying.clone(),
            },
            Window::Paired(peer) => PairingStatus::Paired { peer: peer.clone() },
            Window::Expired => PairingStatus::Expired,
            Window::TooManyWrongCodes(from) => PairingStatus::TooManyWrongCodes { last_from: *from },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(last: u8) -> IpAddr {
        IpAddr::from([192, 168, 1, last])
    }

    #[test]
    fn three_wrong_codes_from_any_addresses_close_the_window_without_a_new_code() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        assert!(window.begin(address(1), now).is_none(), "closed until the user opens it");
        window.open(now).unwrap();
        for last in 1..=3 {
            let attempt = window.begin(address(last), now).unwrap();
            window.end(&attempt, AttemptEnd::WrongCode, now);
        }
        assert_eq!(window.status(now), PairingStatus::TooManyWrongCodes { last_from: Some(address(3)) });
        assert!(window.begin(address(4), now).is_none());
    }

    #[test]
    fn stalled_attempts_dont_count_and_slots_are_limited() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        let attempts: Vec<Attempt> = (1..=3).map(|last| window.begin(address(last), now).unwrap()).collect();
        assert!(window.begin(address(9), now).is_none(), "all slots busy");
        let PairingStatus::Showing { trying, .. } = window.status(now) else { panic!() };
        assert_eq!(trying.len(), 3);
        for attempt in &attempts {
            window.end(attempt, AttemptEnd::Abandoned, now);
        }
        let PairingStatus::Showing { wrong_codes, trying, .. } = window.status(now) else { panic!() };
        assert_eq!((wrong_codes, trying.len()), (0, 0));
    }

    #[test]
    fn the_code_expires_after_two_minutes() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        assert!(!window.expire(now + Duration::from_secs(119)));
        assert!(window.expire(now + PAIR_CODE_LIFETIME));
        assert_eq!(window.status(now + PAIR_CODE_LIFETIME), PairingStatus::Expired);
    }

    #[test]
    fn ten_wrong_codes_in_an_hour_make_pairing_wait() {
        let mut now = Instant::now();
        let mut window = PairingWindow::new();
        let mut wrong = 0;
        while wrong < PAIR_FAILURES_PER_HOUR {
            window.open(now).unwrap();
            let attempt = window.begin(address(1), now).unwrap();
            window.end(&attempt, AttemptEnd::WrongCode, now);
            wrong += 1;
            now += Duration::from_secs(60);
        }
        assert!(matches!(window.open(now).unwrap(), PairingStatus::CoolingDown { .. }));
        assert!(window.begin(address(1), now).is_none());
        let later = now + PAIR_COOLDOWN;
        assert!(matches!(window.open(later).unwrap(), PairingStatus::Showing { .. }));
    }

    #[test]
    fn success_shows_the_new_peer() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        let attempt = window.begin(address(5), now).unwrap();
        let peer = PeerRecord::new("Phone".into(), [7; 32]);
        window.end(&attempt, AttemptEnd::Paired(peer.clone()), now);
        assert_eq!(window.status(now), PairingStatus::Paired { peer });
    }

    #[test]
    fn one_address_gets_one_slot() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        let first = window.begin(address(1), now).unwrap();
        assert!(window.begin(address(1), now).is_none(), "the same address again");
        assert!(window.begin(address(2), now).is_some());
        window.end(&first, AttemptEnd::Abandoned, now);
        assert!(window.begin(address(1), now).is_some());
    }

    #[test]
    fn attempts_that_outlive_their_window_pair_nobody_but_their_wrong_codes_count() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        // Three attempts run; as each wrong one ends, another starts in its slot.
        let mut running: Vec<Attempt> = (1..=3).map(|last| window.begin(address(last), now).unwrap()).collect();
        for last in 4..=5 {
            window.end(&running.remove(0), AttemptEnd::WrongCode, now);
            running.push(window.begin(address(last), now).unwrap());
        }
        window.end(&running.remove(0), AttemptEnd::WrongCode, now);
        assert!(matches!(window.status(now), PairingStatus::TooManyWrongCodes { .. }));
        // Two attempts are still running. One had the right code: too late, its window is closed.
        assert!(!window.is_open_for(&running[0]));
        window.end(&running[0], AttemptEnd::Paired(PeerRecord::new("Late".into(), [9; 32])), now);
        assert!(matches!(window.status(now), PairingStatus::TooManyWrongCodes { .. }));
        // The other was wrong: it still counts towards the hour's limit (4 so far).
        window.end(&running[1], AttemptEnd::WrongCode, now);
        assert_eq!(window.failures.len(), 4);
        // A new window: the old attempts don't belong to it.
        window.open(now).unwrap();
        assert!(!window.is_open_for(&running[0]));
    }

    #[test]
    fn the_hourly_limit_closes_whatever_window_is_open() {
        let now = Instant::now();
        let mut window = PairingWindow::new();
        window.open(now).unwrap();
        let late = window.begin(address(9), now).unwrap();
        for _ in 0..PAIR_FAILURES_PER_HOUR - 1 {
            window.open(now).unwrap();
            let attempt = window.begin(address(1), now).unwrap();
            window.end(&attempt, AttemptEnd::WrongCode, now);
        }
        window.open(now).unwrap();
        // The tenth wrong code comes from an attempt of an earlier window.
        window.end(&late, AttemptEnd::WrongCode, now);
        assert!(matches!(window.status(now), PairingStatus::TooManyWrongCodes { .. }));
        assert!(window.begin(address(2), now).is_none());
        assert!(matches!(window.open(now).unwrap(), PairingStatus::CoolingDown { .. }));
    }
}
