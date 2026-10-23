//! Keeping the sentence audio cache small (spec §4 "keep about an hour per book", §6 sizes).

use crate::error::AppResult;
use crate::speech::AudioKey;

/// What one retention pass removed, as counts only.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioEviction {
    /// Clips of the reading position's document + voice removed because they fell outside the window.
    pub window_clips: usize,
    /// Clips of other documents or voices removed by the global size cap.
    pub cap_clips: usize,
    pub freed_bytes: u64,
    /// Audio left for the reading position's document + voice.
    pub kept_seconds: f64,
    pub cache_bytes: u64,
}

impl AudioEviction {
    pub fn evicted_clips(&self) -> usize {
        self.window_clips + self.cap_clips
    }
}

/// The cache bookkeeping the read-ahead needs besides [`crate::AudioCache`]'s get and put.
pub trait AudioRetention: Send + Sync {
    /// True if the clip is cached, without reading it.
    fn contains(&self, key: &AudioKey) -> AppResult<bool>;

    /// Keeps about `keep_seconds` of audio for the position's document, cleaner version, engine and voice,
    /// deleting the clips farthest behind `position.sentence_id` first, then the farthest ahead.
    /// Then applies the global size cap to everything else, oldest clip first.
    /// Clips being read or written are never deleted.
    fn retain_around(&self, position: &AudioKey, keep_seconds: f64) -> AppResult<AudioEviction>;
}
