//! The phones' items waiting for the gate: urgent before fill, phones served in turn, each phone's items in order.
//! A phone that keeps sending urgent items gets fill priority after `PHONE_URGENT_IN_A_ROW` of them, so it can't
//! starve the computer's own read-ahead.

use std::collections::{HashMap, VecDeque};

use crate::limits::PHONE_URGENT_IN_A_ROW;
use crate::messages::{CancelScope, Item};

pub(crate) type SessionKey = u64;

#[derive(Debug, Clone)]
pub(crate) struct Queued {
    pub session: SessionKey,
    pub batch_id: u32,
    pub item: Item,
    /// Urgent after the budget check.
    pub urgent: bool,
}

#[derive(Default)]
struct PhoneQueue {
    urgent: VecDeque<(u32, Item)>,
    fill: VecDeque<(u32, Item)>,
    urgent_in_a_row: u32,
}

#[derive(Default)]
pub(crate) struct Queue {
    phones: HashMap<SessionKey, PhoneQueue>,
    /// The last phone served; the next pick starts after it.
    last_served: Option<SessionKey>,
}

impl Queue {
    pub fn push(&mut self, session: SessionKey, batch_id: u32, items: Vec<Item>) {
        let phone = self.phones.entry(session).or_default();
        for item in items {
            // A sentence asked for again (now urgent, or re-planned) replaces its queued copy.
            phone.urgent.retain(|(_, queued)| queued.item_id != item.item_id);
            phone.fill.retain(|(_, queued)| queued.item_id != item.item_id);
            if item.urgent {
                phone.urgent.push_back((batch_id, item));
            } else {
                phone.fill.push_back((batch_id, item));
            }
        }
    }

    /// Removes what the scope names; returns how many were removed.
    pub fn cancel(&mut self, session: SessionKey, scope: CancelScope) -> usize {
        let Some(phone) = self.phones.get_mut(&session) else { return 0 };
        let before = phone.urgent.len() + phone.fill.len();
        match scope {
            CancelScope::All => {
                phone.urgent.clear();
                phone.fill.clear();
            }
            CancelScope::Batch(batch) => {
                phone.urgent.retain(|(queued_batch, _)| *queued_batch != batch);
                phone.fill.retain(|(queued_batch, _)| *queued_batch != batch);
            }
        }
        before - phone.urgent.len() - phone.fill.len()
    }

    pub fn remove_session(&mut self, session: SessionKey) {
        self.phones.remove(&session);
    }

    pub fn len_of(&self, session: SessionKey) -> usize {
        self.phones.get(&session).map_or(0, |phone| phone.urgent.len() + phone.fill.len())
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.phones.values().all(|phone| phone.urgent.is_empty() && phone.fill.is_empty())
    }

    /// The next item: an urgent one from a phone within its urgent budget if any, else the next fill (or
    /// over-budget urgent) item; phones in turn after the last one served.
    pub fn pop(&mut self) -> Option<Queued> {
        let order = self.turn_order();
        let urgent_phone = order.iter().copied().find(|key| {
            let phone = &self.phones[key];
            !phone.urgent.is_empty() && phone.urgent_in_a_row < PHONE_URGENT_IN_A_ROW
        });
        let (session, urgent) = match urgent_phone {
            Some(session) => (session, true),
            None => (order.into_iter().find(|key| !self.phones[key].urgent.is_empty() || !self.phones[key].fill.is_empty())?, false),
        };
        let phone = self.phones.get_mut(&session)?;
        let (batch_id, item) = if urgent {
            phone.urgent_in_a_row += 1;
            phone.urgent.pop_front()?
        } else {
            phone.urgent_in_a_row = 0;
            // Over budget, an urgent item still goes before this phone's own fill items.
            phone.urgent.pop_front().or_else(|| phone.fill.pop_front())?
        };
        self.last_served = Some(session);
        Some(Queued { session, batch_id, item, urgent })
    }

    fn turn_order(&self) -> Vec<SessionKey> {
        let mut keys: Vec<SessionKey> = self.phones.keys().copied().collect();
        keys.sort_unstable();
        if let Some(last) = self.last_served {
            let split = keys.iter().position(|key| *key > last).unwrap_or(keys.len());
            keys.rotate_left(split);
        }
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(item_id: u32, urgent: bool) -> Item {
        Item { item_id, voice_id: "af_heart".into(), text: "Invented.".into(), urgent }
    }

    fn drain(queue: &mut Queue) -> Vec<(SessionKey, u32, bool)> {
        std::iter::from_fn(|| queue.pop()).map(|queued| (queued.session, queued.item.item_id, queued.urgent)).collect()
    }

    #[test]
    fn urgent_first_then_phones_take_turns() {
        let mut queue = Queue::default();
        queue.push(1, 1, vec![item(10, false), item(11, false)]);
        queue.push(2, 1, vec![item(20, false), item(21, false), item(22, true)]);
        assert_eq!(drain(&mut queue), vec![(2, 22, true), (1, 10, false), (2, 20, false), (1, 11, false), (2, 21, false)]);
    }

    #[test]
    fn an_item_asked_again_moves_and_cancel_drops() {
        let mut queue = Queue::default();
        queue.push(1, 1, vec![item(1, false), item(2, false), item(3, false)]);
        queue.push(1, 2, vec![item(3, true)]);
        assert_eq!(queue.cancel(1, CancelScope::Batch(1)), 2);
        assert_eq!(drain(&mut queue), vec![(1, 3, true)]);
        queue.push(1, 3, vec![item(4, false)]);
        assert_eq!(queue.cancel(1, CancelScope::All), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn a_phone_sending_only_urgent_items_is_demoted_after_its_budget() {
        let mut queue = Queue::default();
        queue.push(1, 1, (0..6).map(|id| item(id, true)).collect());
        let urgency: Vec<bool> = drain(&mut queue).into_iter().map(|(_, _, urgent)| urgent).collect();
        assert_eq!(urgency, vec![true, true, true, true, false, true]);
    }
}
