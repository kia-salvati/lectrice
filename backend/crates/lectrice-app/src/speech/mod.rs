//! `ListVoices`, `SpeakSentence`, `SpeakVoiceSample`, the read-ahead controls, the speech worker and the read-ahead.

mod control;
#[cfg(test)]
pub(crate) mod fakes;
mod list_voices;
mod read_ahead;
mod read_ahead_plan;
mod speak_sentence;
mod voice_sample;
mod worker;

pub use control::{GetSpeechStats, SetReadAheadMinutes, SpeechStats, StopAllReadAhead, StopReadAhead};
pub use list_voices::{ListVoices, VoiceList};
pub use read_ahead::{ListenerPosition, ReadAhead, ReadAheadDeps, ReadAheadOutcome, ReadAheadSettings};
pub(crate) use read_ahead_plan::{CHARS_PER_SECOND, SECONDS_PER_MINUTE, chars_to_ms, playable_in_order};
pub use read_ahead_plan::{ReadAheadInput, ReadAheadPlan, plan_read_ahead, playback_order};
pub use speak_sentence::{SpeakSentence, SpeakSentenceArgs, SpeakSentenceDeps, SpokenSentence};
pub use voice_sample::{SpeakVoiceSample, VOICE_SAMPLE_TEXT, VOICE_SAMPLE_VERSION};
pub(crate) use worker::elapsed_ms;
pub use worker::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechTicket, SpeechWorker, SynthesisTimings};
