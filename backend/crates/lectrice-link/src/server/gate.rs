//! One synthesis at a time on the computer (desktop spec §2.1): the computer's own speech and every phone's items take
//! turns through this gate, highest priority first, first come first served within a priority.

use crate::sync::lock;
use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Highest first (file 24 §3.7): the computer's own `Now` > phone urgent > the computer's read-ahead > phone fill >
/// the computer's exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GatePriority {
    OwnExport,
    PhoneFill,
    OwnReadAhead,
    PhoneUrgent,
    OwnNow,
}

impl GatePriority {
    pub const fn is_own(self) -> bool {
        matches!(self, Self::OwnNow | Self::OwnReadAhead | Self::OwnExport)
    }
}

#[derive(Default)]
struct GateState {
    busy: bool,
    /// (priority, reversed arrival) so the largest entry is the next to go.
    waiting: BTreeSet<(GatePriority, std::cmp::Reverse<u64>)>,
    next_ticket: u64,
    last_own: Option<Instant>,
}

#[derive(Default)]
pub struct CpuGate {
    state: Mutex<GateState>,
    turn: Condvar,
}

impl CpuGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Blocks until it is this caller's turn; the turn lasts as long as the guard.
    pub fn acquire(&self, priority: GatePriority) -> GateGuard<'_> {
        let mut state = self.lock();
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        let entry = (priority, std::cmp::Reverse(ticket));
        state.waiting.insert(entry);
        while state.busy || state.waiting.last() != Some(&entry) {
            state = self.turn.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        state.waiting.remove(&entry);
        state.busy = true;
        if priority.is_own() {
            state.last_own = Some(Instant::now());
        }
        GateGuard { gate: self, priority }
    }

    /// The computer's own speech used the gate within `window` (or is using it now).
    pub fn own_active_within(&self, window: Duration) -> bool {
        let state = self.lock();
        let own_waiting = state.waiting.iter().any(|(priority, _)| priority.is_own());
        own_waiting || state.last_own.is_some_and(|at| at.elapsed() <= window)
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        lock(&self.state)
    }
}

pub struct GateGuard<'a> {
    gate: &'a CpuGate,
    priority: GatePriority,
}

impl Drop for GateGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.busy = false;
        if self.priority.is_own() {
            state.last_own = Some(Instant::now());
        }
        drop(state);
        self.gate.turn.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, mpsc};
    use std::thread;

    use super::*;

    #[test]
    fn the_highest_waiting_priority_goes_next_then_arrival_order() {
        let gate = Arc::new(CpuGate::new());
        let holder = gate.acquire(GatePriority::PhoneFill);
        let (sender, order) = mpsc::channel();
        let mut threads = Vec::new();
        for (index, priority) in [GatePriority::PhoneFill, GatePriority::OwnExport, GatePriority::PhoneUrgent, GatePriority::OwnNow, GatePriority::PhoneUrgent]
            .into_iter()
            .enumerate()
        {
            let waiter = gate.clone();
            let sender = sender.clone();
            threads.push(thread::spawn(move || {
                let _turn = waiter.acquire(priority);
                sender.send(index).unwrap();
            }));
            // Arrival order must be known for the tie between the two urgent items.
            while gate.lock().waiting.len() < index + 1 {
                thread::yield_now();
            }
        }
        assert!(gate.own_active_within(Duration::ZERO), "own speech is waiting");
        drop(holder);
        threads.into_iter().for_each(|thread| thread.join().unwrap());
        let order: Vec<usize> = order.try_iter().collect();
        assert_eq!(order, vec![3, 2, 4, 0, 1]);
    }

    #[test]
    fn remembers_recent_own_use() {
        let gate = CpuGate::new();
        assert!(!gate.own_active_within(Duration::from_secs(10)));
        drop(gate.acquire(GatePriority::PhoneUrgent));
        assert!(!gate.own_active_within(Duration::from_secs(10)));
        drop(gate.acquire(GatePriority::OwnReadAhead));
        assert!(gate.own_active_within(Duration::from_secs(10)));
    }
}
