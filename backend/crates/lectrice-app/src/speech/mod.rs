//! `ListVoices`, `SpeakSentence`, `SpeakVoiceSample`, the read-ahead controls, prepare jobs, the speech worker, the
//! read-ahead and the helper fill (a paired computer making the audio, file 24).

mod control;
#[cfg(test)]
pub(crate) mod fakes;
mod helper_fill;
#[cfg(test)]
mod helper_sim;
mod list_voices;
mod prepare;
mod read_ahead;
mod read_ahead_plan;
mod speak_sentence;
mod voice_sample;
mod worker;

pub use control::{GetSpeechStats, SetReadAheadMinutes, SpeechStats, StopAllReadAhead, StopReadAhead};
pub use helper_fill::{
    GREEN_RTF_TIMES_RATE, HELPER_AHEAD_MINUTES, HELPER_MAX_IN_FLIGHT_ITEMS, HELPER_MAX_IN_FLIGHT_MS, HELPER_NOW_GRACE, HelperFill, HelperFillDeps,
    HelperFillStats, HelperFillStatus, HelperPrepareProgress, UrgentWait, lost_read_ahead_minutes,
};
pub use list_voices::{ListVoices, VoiceList};
pub use prepare::{PrepareArgs, PrepareDeps, PrepareJobs, PrepareStatus};
pub use read_ahead::{ListenerPosition, ReadAhead, ReadAheadDeps, ReadAheadOutcome, ReadAheadSettings};
pub(crate) use read_ahead_plan::{CHARS_PER_SECOND, SECONDS_PER_MINUTE, chars_to_ms, playable_in_order};
pub use read_ahead_plan::{ReadAheadInput, ReadAheadPlan, plan_read_ahead, playback_order};
pub use speak_sentence::{SpeakSentence, SpeakSentenceArgs, SpeakSentenceDeps, SpokenSentence};
pub use voice_sample::{SpeakVoiceSample, VOICE_SAMPLE_TEXT, VOICE_SAMPLE_VERSION};
pub(crate) use worker::elapsed_ms;
pub use worker::{CancelHandle, SpeechJob, SpeechOutcome, SpeechPriority, SpeechTicket, SpeechWorker, SynthesisGate, SynthesisTimings};
