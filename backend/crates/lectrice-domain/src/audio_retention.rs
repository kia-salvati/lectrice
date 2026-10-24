//! Keeping the sentence audio cache small (spec §4 "keep about an hour per book", §6 sizes).

use crate::error::AppResult;
use crate::ids::DocumentId;
use crate::speech::AudioKey;

/// What one retention pass removed, as counts only.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioEviction {
    /// Clips of the reading position's document + voice removed because they fell outside the window.
    pub window_clips: usize,
    /// Clips of other documents or voices removed by the global size cap (or by a clear).
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

/// The cache's size, in total and per document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CacheUsage {
    pub bytes: u64,
    pub clips: usize,
    /// The global size cap.
    pub cap_bytes: u64,
    /// Every document with clips, in no particular order.
    pub documents: Vec<DocumentCacheUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentCacheUsage {
    pub document_id: DocumentId,
    pub bytes: u64,
    pub clips: usize,
}

/// The cache bookkeeping the read-ahead and the storage screens need besides [`crate::AudioCache`]'s get and put.
/// Clips being read or written are never deleted.
pub trait AudioRetention: Send + Sync {
    /// True if the clip is cached, without reading it.
    fn contains(&self, key: &AudioKey) -> AppResult<bool>;

    /// Keeps about `keep_seconds` of audio for the position's document, cleaner version, engine and voice,
    /// deleting the clips farthest behind `position.sentence_id` first, then the farthest ahead.
    /// Then applies the global size cap to everything else, oldest clip first.
    fn retain_around(&self, position: &AudioKey, keep_seconds: f64) -> AppResult<AudioEviction>;

    fn usage(&self) -> AppResult<CacheUsage>;

    /// Deletes every clip.
    fn clear(&self) -> AppResult<AudioEviction>;

    /// Deletes every clip of one document (all cleaner versions, engines and voices). Clips in use go when released.
    fn remove_document(&self, id: &DocumentId) -> AppResult<AudioEviction>;

    /// Re-reads what is on disk, for a cache root the system may clear behind the app's back.
    fn refresh(&self) -> AppResult<()>;
}
