use std::sync::Arc;
use std::time::Instant;

use lectrice_domain::{AppResult, Document, DocumentCleaner, DocumentId, DocumentMeta, DocumentSummary, DocumentWriter, PdfTextSource, SentenceSplitter};
use tracing::info;

use super::build::{DocumentOrigin, build_document};

const NAME: &str = "DocumentPipeline";

/// PDF bytes → text runs → cleaned blocks → numbered sentences. Saves nothing.
pub struct DocumentPipeline {
    source: Arc<dyn PdfTextSource>,
    cleaner: Arc<dyn DocumentCleaner>,
    splitter: Arc<dyn SentenceSplitter>,
}

impl DocumentPipeline {
    pub fn new(source: Arc<dyn PdfTextSource>, cleaner: Arc<dyn DocumentCleaner>, splitter: Arc<dyn SentenceSplitter>) -> Self {
        Self { source, cleaner, splitter }
    }

    /// Saved with an older cleaner, so it should be re-cleaned from its original PDF.
    pub fn is_outdated(&self, document: &Document) -> bool {
        document.cleaner_version < self.cleaner.version()
    }

    pub fn process(&self, id: &DocumentId, source_file_name: &str, pdf: &[u8]) -> AppResult<Document> {
        let started = Instant::now();

        let content = self.source.read_runs(pdf)?;
        info!(service = NAME, doc_id = %id, pages = content.pages.len(), runs = content.runs.len(), "pdf text read");
        let cleaned = self.cleaner.clean(content)?;
        info!(service = NAME, doc_id = %id, blocks = cleaned.blocks.len(), "document cleaned");

        let origin = DocumentOrigin { id, source_file_name, cleaner_version: self.cleaner.version() };
        let document = build_document(origin, cleaned, self.splitter.as_ref());
        let duration_ms = started.elapsed().as_millis();
        info!(service = NAME, doc_id = %id, blocks = document.blocks.len(), sentences = document.sentence_count(), duration_ms, "sentences split");
        Ok(document)
    }

    /// Re-cleans `stale` from its original PDF and saves it with `meta`. The reading position is kept
    /// only if that sentence id still exists.
    pub(crate) fn reclean(&self, writer: &dyn DocumentWriter, stale: &Document, pdf: &[u8], meta: DocumentMeta) -> AppResult<(Document, DocumentMeta)> {
        let document = self.process(&stale.id, &stale.source_file_name, pdf)?;
        let sentences = document.sentence_count();
        let last_sentence_id = meta.last_sentence_id.filter(|id| id.0 < sentences);
        let meta = DocumentMeta { last_sentence_id, ..meta };

        writer.save(&document, pdf, &meta)?;
        info!(service = NAME, doc_id = %document.id, from_version = stale.cleaner_version, to_version = document.cleaner_version, "document re-cleaned and saved");
        Ok((document, meta))
    }
}

pub(crate) fn summarize(document: &Document, meta: &DocumentMeta) -> DocumentSummary {
    DocumentSummary {
        id: document.id.clone(),
        title: document.title.clone(),
        source_file_name: document.source_file_name.clone(),
        page_count: document.page_count,
        sentence_count: document.sentence_count(),
        added_at: meta.added_at,
        last_opened_at: meta.last_opened_at,
        last_sentence_id: meta.last_sentence_id,
    }
}

pub(crate) fn meta_of(summary: &DocumentSummary) -> DocumentMeta {
    DocumentMeta { added_at: summary.added_at, last_opened_at: summary.last_opened_at, last_sentence_id: summary.last_sentence_id }
}
