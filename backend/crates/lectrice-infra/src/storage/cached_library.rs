use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lectrice_domain::{
    AppResult, Document, DocumentBytes, DocumentId, DocumentMeta, DocumentReader, DocumentSummary, DocumentWriter, ListOptions, Page, ReadingPosition,
    Timestamp,
};

/// Keeps the last few parsed documents in memory in front of a library, because every audio request
/// (and so every read-ahead step) looks its document up. Saving a document drops its cached copy.
/// Positions and opened times live outside `document.json`, so they don't touch the cache.
pub struct CachedLibrary<L> {
    inner: Arc<L>,
    capacity: usize,
    state: Mutex<CacheState>,
}

#[derive(Default)]
struct CacheState {
    /// Most recently used first.
    recent: VecDeque<Arc<Document>>,
    /// Bumped by every save, so a read that raced with a save isn't cached.
    saves: u64,
}

impl<L> CachedLibrary<L> {
    pub fn new(inner: Arc<L>, capacity: usize) -> Self {
        Self { inner, capacity, state: Mutex::default() }
    }

    /// Only whole `Arc`s are pushed and removed under the lock, so a poisoned lock still holds a valid list.
    fn lock(&self) -> MutexGuard<'_, CacheState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn cached(&self, id: &DocumentId) -> (Option<Arc<Document>>, u64) {
        let mut state = self.lock();
        let saves = state.saves;
        let Some(index) = state.recent.iter().position(|document| document.id == *id) else {
            return (None, saves);
        };
        let document = state.recent.remove(index);
        if let Some(document) = &document {
            state.recent.push_front(document.clone());
        }
        (document, saves)
    }

    fn remember(&self, document: Arc<Document>, saves_before_read: u64) {
        let mut state = self.lock();
        if self.capacity == 0 || state.saves != saves_before_read {
            return;
        }
        state.recent.retain(|cached| cached.id != document.id);
        state.recent.push_front(document);
        state.recent.truncate(self.capacity);
    }

    fn forget(&self, id: &DocumentId) {
        let mut state = self.lock();
        state.saves += 1;
        state.recent.retain(|cached| cached.id != *id);
    }
}

impl<L: DocumentReader> DocumentReader for CachedLibrary<L> {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>> {
        let (cached, saves) = self.cached(id);
        if let Some(document) = cached {
            return Ok(Some(Document::clone(&document)));
        }
        let found = self.inner.find(id)?;
        if let Some(document) = &found {
            self.remember(Arc::new(document.clone()), saves);
        }
        Ok(found)
    }

    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
        self.inner.find_summary(id)
    }

    fn find_original_pdf(&self, id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
        self.inner.find_original_pdf(id)
    }

    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
        self.inner.list_recent(options)
    }

    fn list_removed(&self) -> AppResult<Vec<DocumentId>> {
        self.inner.list_removed()
    }

    fn library_usage(&self) -> AppResult<Vec<DocumentBytes>> {
        self.inner.library_usage()
    }
}

impl<L: DocumentWriter> DocumentWriter for CachedLibrary<L> {
    /// Forgets before and after writing: a read during the write can't be cached either way (see `saves`).
    fn save(&self, document: &Document, original_pdf: &[u8], meta: &DocumentMeta) -> AppResult<()> {
        self.forget(&document.id);
        let saved = self.inner.save(document, original_pdf, meta);
        self.forget(&document.id);
        saved
    }

    fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> AppResult<()> {
        self.inner.save_position(id, position)
    }

    fn mark_opened(&self, id: &DocumentId, opened_at: Timestamp) -> AppResult<()> {
        self.inner.mark_opened(id, opened_at)
    }

    /// A removed document must not be served from memory.
    fn set_removed(&self, id: &DocumentId, removed_at: Option<Timestamp>) -> AppResult<()> {
        self.forget(id);
        let changed = self.inner.set_removed(id, removed_at);
        self.forget(id);
        changed
    }

    fn delete(&self, id: &DocumentId) -> AppResult<()> {
        self.forget(id);
        let deleted = self.inner.delete(id);
        self.forget(id);
        deleted
    }
}
