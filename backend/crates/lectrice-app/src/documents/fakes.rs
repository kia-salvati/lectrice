//! In-memory fakes of the domain ports for the use-case tests.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use lectrice_domain::{
    AppError, AppResult, BlockKind, CleanedBlock, CleanedDocument, Clock, ContentHasher, Document, DocumentCleaner, DocumentId, DocumentMeta, DocumentReader,
    DocumentSummary, DocumentWriter, ErrorName, ListOptions, Page, PdfContent, PdfTextSource, ReadingPosition, SentenceId, SentenceSplitter, TextSpan,
    Timestamp,
};

use super::pipeline::{DocumentPipeline, summarize};

pub(crate) const PDF_BYTES: &[u8] = b"%PDF-1.7 a small fake book";

pub(crate) fn at(seconds: i64) -> Timestamp {
    Timestamp::from_timestamp(seconds, 0).unwrap()
}

pub(crate) fn id_for(n: u8) -> DocumentId {
    DocumentId::parse(&format!("{n:064x}")).unwrap()
}

/// Same bytes, same id; different bytes almost surely a different one. Not SHA-256 (that is the adapter's job).
pub(crate) struct FakeHasher;

impl ContentHasher for FakeHasher {
    fn document_id(&self, pdf: &[u8]) -> AppResult<DocumentId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        pdf.hash(&mut hasher);
        DocumentId::parse(&format!("{:064x}", hasher.finish()))
    }
}

/// Ends a sentence after `.`, `!` or `?`; ranges exclude surrounding whitespace.
pub(crate) struct FakeSplitter;

impl SentenceSplitter for FakeSplitter {
    fn split(&self, text: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut start = 0;
        for (index, character) in text.char_indices() {
            if matches!(character, '.' | '!' | '?') {
                push_trimmed(text, start..index + 1, &mut ranges);
                start = index + 1;
            }
        }
        push_trimmed(text, start..text.len(), &mut ranges);
        ranges
    }
}

fn push_trimmed(text: &str, range: Range<usize>, ranges: &mut Vec<Range<usize>>) {
    let piece = &text[range.clone()];
    let trimmed = piece.trim();
    if trimmed.is_empty() {
        return;
    }
    let start = range.start + (piece.len() - piece.trim_start().len());
    ranges.push(start..start + trimmed.len());
}

#[derive(Default)]
pub(crate) struct FakeSource {
    calls: AtomicUsize,
}

impl FakeSource {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl PdfTextSource for FakeSource {
    fn read_runs(&self, _pdf: &[u8]) -> AppResult<PdfContent> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(PdfContent::default())
    }
}

/// Always returns a heading, a two-sentence paragraph and a code block: 5 sentence ids with the cue.
pub(crate) struct FakeCleaner {
    version: AtomicU32,
    calls: AtomicUsize,
}

impl FakeCleaner {
    pub fn new(version: u32) -> Self {
        Self { version: AtomicU32::new(version), calls: AtomicUsize::new(0) }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub fn set_version(&self, version: u32) {
        self.version.store(version, Ordering::SeqCst);
    }
}

fn cleaned_block(kind: BlockKind, page_index: u32, text: &str) -> CleanedBlock {
    CleanedBlock { kind, level: None, page_index, spans: vec![TextSpan { text: text.to_string(), page_index, spoken: true }] }
}

impl DocumentCleaner for FakeCleaner {
    fn version(&self) -> u32 {
        self.version.load(Ordering::SeqCst)
    }

    fn clean(&self, _content: PdfContent) -> AppResult<CleanedDocument> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CleanedDocument {
            title: Some("Fake Book".into()),
            page_count: 3,
            pages: Vec::new(),
            outline: Vec::new(),
            blocks: vec![
                cleaned_block(BlockKind::Heading, 1, "Intro"),
                cleaned_block(BlockKind::Paragraph, 1, "One. Two."),
                cleaned_block(BlockKind::Code, 2, "let x = 1;"),
            ],
        })
    }
}

pub(crate) struct FixedClock(Mutex<Timestamp>);

impl FixedClock {
    pub fn set(&self, now: Timestamp) {
        *self.0.lock().unwrap() = now;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

struct Stored {
    document: Document,
    pdf: Vec<u8>,
    meta: DocumentMeta,
}

#[derive(Default)]
pub(crate) struct InMemoryStore {
    documents: Mutex<HashMap<DocumentId, Stored>>,
}

impl InMemoryStore {
    pub fn original_pdf(&self, id: &DocumentId) -> Option<Vec<u8>> {
        self.documents.lock().unwrap().get(id).map(|stored| stored.pdf.clone())
    }

    pub fn document(&self, id: &DocumentId) -> Option<Document> {
        self.documents.lock().unwrap().get(id).map(|stored| stored.document.clone())
    }

    pub fn summary(&self, id: &DocumentId) -> Option<DocumentSummary> {
        self.documents.lock().unwrap().get(id).map(|stored| summarize(&stored.document, &stored.meta))
    }

    pub fn set_position(&self, id: &DocumentId, sentence_id: u32) {
        self.save_position(id, ReadingPosition { sentence_id: SentenceId(sentence_id) }).unwrap();
    }

    fn update_meta(&self, id: &DocumentId, change: impl FnOnce(&mut DocumentMeta)) -> AppResult<()> {
        let mut documents = self.documents.lock().unwrap();
        let stored = documents.get_mut(id).ok_or_else(|| AppError::new(ErrorName::DocumentNotFound))?;
        change(&mut stored.meta);
        Ok(())
    }
}

impl DocumentReader for InMemoryStore {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>> {
        Ok(self.document(id))
    }

    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
        Ok(self.summary(id))
    }

    fn find_original_pdf(&self, id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
        Ok(self.original_pdf(id))
    }

    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
        let limit = options.limit_or_default();
        let offset = options.offset.unwrap_or(0);
        let mut items: Vec<DocumentSummary> = self.documents.lock().unwrap().values().map(|stored| summarize(&stored.document, &stored.meta)).collect();
        items.sort_by_key(|item| std::cmp::Reverse(item.last_opened_at));
        let total = items.len();
        Ok(Page { items: items.into_iter().skip(offset).take(limit).collect(), total, limit, offset })
    }
}

impl DocumentWriter for InMemoryStore {
    fn save(&self, document: &Document, original_pdf: &[u8], meta: &DocumentMeta) -> AppResult<()> {
        let mut documents = self.documents.lock().unwrap();
        let meta = match documents.get(&document.id) {
            Some(saved) => meta.clone().merged_with_saved(&saved.meta, document.sentence_count()),
            None => meta.clone(),
        };
        documents.insert(document.id.clone(), Stored { document: document.clone(), pdf: original_pdf.to_vec(), meta });
        Ok(())
    }

    fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> AppResult<()> {
        self.update_meta(id, |meta| meta.last_sentence_id = Some(position.sentence_id))
    }

    fn mark_opened(&self, id: &DocumentId, opened_at: Timestamp) -> AppResult<()> {
        self.update_meta(id, |meta| meta.last_opened_at = opened_at)
    }
}

/// All fakes wired together; the clock starts at `at(100)`.
pub(crate) struct Library {
    pub store: Arc<InMemoryStore>,
    pub source: Arc<FakeSource>,
    pub cleaner: Arc<FakeCleaner>,
    pub clock: Arc<FixedClock>,
}

impl Library {
    pub fn new(cleaner_version: u32) -> Self {
        Self {
            store: Arc::default(),
            source: Arc::default(),
            cleaner: Arc::new(FakeCleaner::new(cleaner_version)),
            clock: Arc::new(FixedClock(Mutex::new(at(100)))),
        }
    }

    pub fn pipeline(&self) -> Arc<DocumentPipeline> {
        Arc::new(DocumentPipeline::new(self.source.clone(), self.cleaner.clone(), Arc::new(FakeSplitter)))
    }

    pub fn reader(&self) -> Arc<dyn DocumentReader> {
        self.store.clone()
    }

    pub fn writer(&self) -> Arc<dyn DocumentWriter> {
        self.store.clone()
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    /// Imports `PDF_BYTES` through the real pipeline and returns its id.
    pub fn add_book(&self) -> DocumentId {
        let id = id_for(1);
        let document = self.pipeline().process(&id, "Book.pdf", PDF_BYTES).unwrap();
        let now = self.clock.now();
        self.store.save(&document, PDF_BYTES, &DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None }).unwrap();
        id
    }
}
