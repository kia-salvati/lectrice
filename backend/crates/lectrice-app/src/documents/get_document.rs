use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, Clock, Document, DocumentId, DocumentMeta, DocumentReader, DocumentWriter, ErrorName, SentenceId};
use tracing::info;

use super::pipeline::{DocumentPipeline, meta_of};
use crate::{RequestContext, UseCase};

#[derive(Debug, Clone)]
pub struct GetDocumentArgs {
    pub id: DocumentId,
}

/// The full document plus where the listener stopped, so the reader can offer "continue".
#[derive(Debug, Clone, PartialEq)]
pub struct OpenedDocument {
    pub document: Document,
    pub last_sentence_id: Option<SentenceId>,
}

/// Loads a saved document and marks it opened. An outdated one is re-cleaned from its original PDF first.
pub struct GetDocument {
    pipeline: Arc<DocumentPipeline>,
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
    clock: Arc<dyn Clock>,
}

impl GetDocument {
    pub fn new(pipeline: Arc<DocumentPipeline>, reader: Arc<dyn DocumentReader>, writer: Arc<dyn DocumentWriter>, clock: Arc<dyn Clock>) -> Self {
        Self { pipeline, reader, writer, clock }
    }

    fn reclean(&self, stale: &Document, meta: DocumentMeta) -> AppResult<OpenedDocument> {
        let pdf = self.reader.find_original_pdf(&stale.id)?;
        info!(service = Self::NAME, doc_id = %stale.id, found = pdf.is_some(), bytes = pdf.as_ref().map(Vec::len), "original pdf loaded");
        let Some(pdf) = pdf else {
            return Err(AppError::new(ErrorName::StorageFailed).with_detail(format!("original.pdf of {} is missing", stale.id)));
        };

        let (document, meta) = self.pipeline.reclean(self.writer.as_ref(), stale, &pdf, meta)?;
        Ok(OpenedDocument { document, last_sentence_id: meta.last_sentence_id })
    }
}

impl UseCase for GetDocument {
    const NAME: &'static str = "GetDocument";
    type Args = GetDocumentArgs;
    type Output = OpenedDocument;

    fn run(&self, ctx: &RequestContext, args: GetDocumentArgs) -> AppResult<OpenedDocument> {
        let id = args.id;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %id, "getting document");

        let summary = self.reader.find_summary(&id)?;
        info!(service = Self::NAME, doc_id = %id, found = summary.is_some(), "library summary looked up");
        let Some(summary) = summary else {
            return Err(AppError::new(ErrorName::DocumentNotFound));
        };
        let stored = self.reader.find(&id)?;
        info!(service = Self::NAME, doc_id = %id, found = stored.is_some(), cleaner_version = stored.as_ref().map(|document| document.cleaner_version), "stored document loaded");
        let Some(stored) = stored else {
            return Err(AppError::new(ErrorName::DocumentNotFound).with_detail("meta.json exists but document.json is missing"));
        };
        let now = self.clock.now();

        if self.pipeline.is_outdated(&stored) {
            return self.reclean(&stored, DocumentMeta { last_opened_at: now, ..meta_of(&summary) });
        }

        self.writer.mark_opened(&id, now)?;
        info!(service = Self::NAME, doc_id = %id, blocks = stored.blocks.len(), "document marked opened");
        Ok(OpenedDocument { document: stored, last_sentence_id: summary.last_sentence_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::fakes::{Library, PDF_BYTES, at, id_for};

    fn get(library: &Library, id: DocumentId) -> AppResult<OpenedDocument> {
        let service = GetDocument::new(library.pipeline(), library.reader(), library.writer(), library.clock());
        service.run(&RequestContext::new("test"), GetDocumentArgs { id })
    }

    #[test]
    fn returns_the_document_with_its_position_and_marks_it_opened() {
        let library = Library::new(1);
        let id = library.add_book();
        library.store.set_position(&id, 3);
        library.clock.set(at(500));

        let opened = get(&library, id.clone()).unwrap();

        assert_eq!(opened.document, library.store.document(&id).unwrap());
        assert_eq!(opened.last_sentence_id, Some(SentenceId(3)));
        assert_eq!(library.store.summary(&id).unwrap().last_opened_at, at(500));
        assert_eq!(library.cleaner.calls(), 1);
    }

    #[test]
    fn unknown_id_is_document_not_found() {
        let library = Library::new(1);

        let error = get(&library, id_for(9)).unwrap_err();

        assert_eq!(error.primary(), ErrorName::DocumentNotFound);
    }

    #[test]
    fn outdated_document_is_re_cleaned_from_the_saved_pdf() {
        let library = Library::new(1);
        let id = library.add_book();
        library.store.set_position(&id, 4);
        library.cleaner.set_version(2);
        library.clock.set(at(600));

        let opened = get(&library, id.clone()).unwrap();

        assert_eq!(opened.document.cleaner_version, 2);
        assert_eq!(opened.last_sentence_id, Some(SentenceId(4)));
        assert_eq!(library.cleaner.calls(), 2);
        assert_eq!(library.source.calls(), 2);
        let summary = library.store.summary(&id).unwrap();
        assert_eq!((summary.added_at, summary.last_opened_at), (at(100), at(600)));
        assert_eq!(library.store.document(&id).unwrap().cleaner_version, 2);
    }

    #[test]
    fn a_document_saved_before_the_toc_existed_gains_one_when_re_cleaned() {
        let library = Library::new(1);
        let id = library.add_book();
        let saved_without_toc = Document { toc: Vec::new(), ..library.store.document(&id).unwrap() };
        let summary = library.store.summary(&id).unwrap();
        let meta = DocumentMeta { added_at: summary.added_at, last_opened_at: summary.last_opened_at, last_sentence_id: None };
        library.store.save(&saved_without_toc, PDF_BYTES, &meta).unwrap();
        library.cleaner.set_version(2);

        let opened = get(&library, id.clone()).unwrap();

        assert_eq!(opened.document.toc.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>(), ["Intro"]);
        assert_eq!(library.store.document(&id).unwrap().toc, opened.document.toc);
    }
}
