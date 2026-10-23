//! `ListVoices`, `SpeakSentence`, the speech worker and the read-ahead.

#[cfg(test)]
pub(crate) mod fakes;
mod list_voices;
mod read_ahead;
mod read_ahead_plan;
mod speak_sentence;
mod worker;

pub use list_voices::{ListVoices, VoiceList};
pub use read_ahead::{ListenerPosition, ReadAhead, ReadAheadOutcome, ReadAheadSettings};
pub(crate) use read_ahead_plan::{CHARS_PER_SECOND, SECONDS_PER_MINUTE, playable_in_order};
pub use read_ahead_plan::{ReadAheadInput, ReadAheadPlan, plan_read_ahead, playback_order};
pub use speak_sentence::{SpeakSentence, SpeakSentenceArgs, SpeakSentenceDeps, SpokenSentence};
pub(crate) use worker::elapsed_ms;
pub use worker::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechTicket, SpeechWorker};
