//! A sentence the player needs now: it goes to the computer as an urgent item, and the caller waits for it (or races
//! the phone's engine) for a grace that depends on the link and on whether the phone's engine is loaded.

use std::sync::{Arc, PoisonError};
use std::time::Duration;

use lectrice_domain::{AppError, AppResult, AudioKey, ErrorName, HelperItem};
use tracing::debug;

use super::{HelperFill, InFlight, NAME, estimated_ms};
use crate::Cancellation;

/// How long a sentence the player needs now waits for the helper before the phone makes it itself (the
/// laptop's p95 for a 10-word sentence is 1.09 s). Counted from the moment the link is ready.
pub const HELPER_NOW_GRACE: Duration = Duration::from_millis(1500);
/// The grace while the phone's engine isn't loaded. The local path then pays the load first (2.2 s on a Galaxy S10+),
/// so waiting longer for the computer costs little and keeps the model out of memory (a cold desktop engine or a
/// voice change during an item answered in 1.95–2.06 s).
pub const HELPER_COLD_GRACE: Duration = Duration::from_millis(3000);
/// How long after `start` the first connection may take before the phone stops waiting for it: no local speech or
/// read-ahead meanwhile (the link was Connecting ~100 ms at play; TCP + Noise IK took 37 ms).
pub const HELPER_CONNECT_WAIT: Duration = Duration::from_millis(1500);
/// How often a waiting caller checks whether it was cancelled.
const WAIT_POLL: Duration = Duration::from_millis(100);

/// How long a sentence the player needs now waits for the helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperWait {
    pub grace: Duration,
    pub cold_grace: Duration,
}

impl Default for HelperWait {
    fn default() -> Self {
        Self { grace: HELPER_NOW_GRACE, cold_grace: HELPER_COLD_GRACE }
    }
}

impl HelperWait {
    /// How much longer the caller may wait for the helper; `None` once the phone should make the sentence itself.
    /// `first_link_left`: what is left of the first connection's wait ([`HelperFill::first_link_left`]);
    /// `ready_for`: how long the link has been ready since the sentence was asked for (`None` while it isn't).
    pub fn left(&self, first_link_left: Option<Duration>, ready_for: Option<Duration>, engine_loaded: bool) -> Option<Duration> {
        match ready_for {
            Some(waited) => {
                let grace = if engine_loaded { self.grace } else { self.cold_grace };
                grace.checked_sub(waited).filter(|left| !left.is_zero())
            }
            None => first_link_left,
        }
    }
}

impl HelperFill {
    /// While the first connection after `start` is still being set up: how much longer it is waited for. Meanwhile
    /// the local read-ahead stays paused and a sentence the player needs waits ([`HelperWait`]), so a link that is
    /// about to be ready never makes the phone load its own engine.
    pub fn first_link_left(&self) -> Option<Duration> {
        self.lock().first_link_left()
    }

    /// Waits up to `timeout` (at most a poll interval) for a clip to be stored or the link to change.
    pub fn wait_for_change(&self, timeout: Duration, cancellation: &Cancellation) -> AppResult<()> {
        if cancellation.is_cancelled() {
            return Err(AppError::new(ErrorName::RequestCancelled).with_detail("the caller stopped waiting for the helper"));
        }
        let state = self.lock();
        let _ = self.changed.wait_timeout(state, timeout.min(WAIT_POLL)).unwrap_or_else(PoisonError::into_inner);
        Ok(())
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
                state.in_flight.insert(item_id, InFlight { key: key.clone(), text: text.to_string(), estimated_ms, urgent: true, prepare: false });
                item_id
            }
        };
        let flight = state.in_flight.get_mut(&item_id)?;
        if flight.urgent {
            // Already first in the helper's queue (or being made): asking again could only make it twice.
            debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, item_id, "urgent item already asked for");
            return Some(UrgentWait { fill, key: key.clone() });
        }
        flight.urgent = true;
        let item = HelperItem { item_id, voice_id: key.voice_id.clone(), text: flight.text.clone(), urgent: true };
        if let Err(error) = self.send(&mut state, vec![item]) {
            debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, error = %error, "urgent item not sent");
            return None;
        }
        debug!(service = NAME, doc_id = %key.document_id, sentence_id = key.sentence_id.0, item_id, "urgent item sent");
        Some(UrgentWait { fill, key: key.clone() })
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
}
