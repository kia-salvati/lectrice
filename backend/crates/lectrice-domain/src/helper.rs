//! The speech helper port: another machine that makes sentence clips for this one (the phone's paired
//! computer). No network words here: the adapter decides how items travel, the app only asks for clips and stores
//! what comes back in its normal audio cache.

use std::fmt;
use std::sync::Arc;

use crate::error::{AppResult, ErrorName};
use crate::ids::VoiceId;
use crate::speech::Audio;

/// The engine the helper must make clips with. Clips are only ever produced when the helper has exactly this engine
/// (same id and revision) at this sample rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperEngine {
    pub engine_id: String,
    pub revision: String,
    pub sample_rate: u32,
}

/// One sentence to make. `item_id` is the app's own number for it, meaningless without the book.
#[derive(Clone, PartialEq, Eq)]
pub struct HelperItem {
    pub item_id: u32,
    pub voice_id: VoiceId,
    pub text: String,
    /// The player needs it now: it goes before every fill item.
    pub urgent: bool,
}

/// Never prints the text (book content must not reach a log).
impl fmt::Debug for HelperItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperItem")
            .field("item_id", &self.item_id)
            .field("voice_id", &self.voice_id)
            .field("text_chars", &self.text.chars().count())
            .field("urgent", &self.urgent)
            .finish()
    }
}

/// Where the helper stands. Only `Connected` and `Busy` make clips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperState {
    /// Not asked to help (no engine opened, or closed).
    Off,
    /// Looking for the helper or setting up the session.
    Connecting,
    /// Ready for this engine.
    Connected,
    /// Connected, but the helper's own listener (or another device) comes first for now.
    Busy,
    /// Not reachable; the adapter keeps retrying.
    Lost,
    /// Won't work until a person acts (e.g. `HelperNotPaired`, `HelperModelMissing`, `HelperIncompatible`).
    NeedsAttention(ErrorName),
}

impl HelperState {
    /// Clips can be asked for.
    pub const fn makes_clips(&self) -> bool {
        matches!(self, Self::Connected | Self::Busy)
    }
}

/// What the helper reports, on the adapter's own thread.
#[derive(Debug, Clone, PartialEq)]
pub enum HelperEvent {
    State(HelperState),
    /// A whole clip within bounds, at the opened engine's sample rate, for an item still asked for.
    Clip {
        item_id: u32,
        audio: Audio,
    },
    /// The helper won't make this item; the app makes it itself.
    ItemFailed {
        item_id: u32,
        error: ErrorName,
    },
}

/// Receives the helper's events. Called on the adapter's thread: must be short and must not block on the helper.
pub trait HelperEvents: Send + Sync {
    fn on_event(&self, event: HelperEvent);
}

/// A producer of sentence clips on another machine. Every call returns at once (the player's thread calls some of
/// them); work and waiting happen on the adapter's threads.
pub trait SpeechHelper: Send + Sync {
    fn state(&self) -> HelperState;
    /// Starts (or restarts) helping with `engine`; events go to `events` from now on. A different engine on the
    /// helper ends in `NeedsAttention`, never in clips.
    fn open(&self, engine: HelperEngine, events: Arc<dyn HelperEvents>) -> AppResult<()>;
    /// Queues items. Fails with `HelperUnreachable` when not connected.
    fn make(&self, items: Vec<HelperItem>) -> AppResult<()>;
    /// Drops every queued item; a clip being made still arrives and is ignored by the app.
    fn cancel_all(&self) -> AppResult<()>;
    /// Stops helping (the session ends); the state becomes `Off`.
    fn close(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_never_prints_its_text() {
        let item = HelperItem { item_id: 7, voice_id: VoiceId::new("af_heart"), text: "secret words".into(), urgent: true };

        let printed = format!("{item:?}");

        assert!(!printed.contains("secret"));
        assert!(printed.contains("text_chars: 12"));
    }

    #[test]
    fn only_connected_and_busy_make_clips() {
        assert!(HelperState::Connected.makes_clips());
        assert!(HelperState::Busy.makes_clips());
        for state in [HelperState::Off, HelperState::Connecting, HelperState::Lost, HelperState::NeedsAttention(ErrorName::HelperNotPaired)] {
            assert!(!state.makes_clips(), "{state:?}");
        }
    }
}
