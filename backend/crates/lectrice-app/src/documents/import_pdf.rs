use std::sync::Arc;

use lectrice_domain::{
    AppError, AppResult, Clock, ContentHasher, DocumentId, DocumentMeta, DocumentReader, DocumentSummary, DocumentWriter, ErrorName, Timestamp,
};
use tracing::info;

use super::pipeline::{DocumentPipeline, meta_of, summarize};
use crate::{RequestContext, UseCase};

const PDF_MAGIC: &[u8] = b"%PDF-";
/// Readers accept the header anywhere in the first KiB (some files start with junk bytes).
const PDF_HEADER_SEARCH_BYTES: usize = 1024;
const FALLBACK_FILE_NAME: &str = "document.pdf";

#[derive(Debug, Clone)]
pub struct ImportPdfArgs {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPdf {
    pub summary: DocumentSummary,
    pub already_in_library: bool,
}

/// Adds a PDF to the library, or reopens it when the same bytes were imported before.
pub struct ImportPdf {
    hasher: Arc<dyn ContentHasher>,
    pipeline: Arc<DocumentPipeline>,
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
    clock: Arc<dyn Clock>,
    max_upload_bytes: usize,
}

impl ImportPdf {
    pub fn new(
        hasher: Arc<dyn ContentHasher>,
        pipeline: Arc<DocumentPipeline>,
        reader: Arc<dyn DocumentReader>,
        writer: Arc<dyn DocumentWriter>,
        clock: Arc<dyn Clock>,
        max_upload_bytes: usize,
    ) -> Self {
        Self { hasher, pipeline, reader, writer, clock, max_upload_bytes }
    }

    fn check_pdf(&self, bytes: &[u8]) -> AppResult<()> {
        if bytes.is_empty() {
            return Err(AppError::new(ErrorName::UploadMissingFile).with_detail("uploaded file is empty"));
        }
        if bytes.len() > self.max_upload_bytes {
            return Err(AppError::new(ErrorName::UploadTooLarge).with_detail(format!("{} bytes, limit {}", bytes.len(), self.max_upload_bytes)));
        }
        let header = &bytes[..bytes.len().min(PDF_HEADER_SEARCH_BYTES)];
        if !header.windows(PDF_MAGIC.len()).any(|window| window == PDF_MAGIC) {
            return Err(AppError::new(ErrorName::PdfInvalid).with_detail("no %PDF- header"));
        }
        Ok(())
    }

    /// The stored document's summary when these bytes are already in the library (re-cleaned first if outdated).
    fn reopen(&self, id: &DocumentId, pdf: &[u8], now: Timestamp) -> AppResult<Option<DocumentSummary>> {
        let summary = self.reader.find_summary(id)?;
        info!(service = Self::NAME, doc_id = %id, found = summary.is_some(), "library summary looked up");
        let Some(summary) = summary else {
            return Ok(None);
        };
        let stored = self.reader.find(id)?;
        info!(service = Self::NAME, doc_id = %id, found = stored.is_some(), cleaner_version = stored.as_ref().map(|document| document.cleaner_version), "stored document loaded");
        let Some(stored) = stored else {
            return Ok(None);
        };

        if self.pipeline.is_outdated(&stored) {
            let meta = DocumentMeta { last_opened_at: now, ..meta_of(&summary) };
            // The upload has the same bytes as the saved original.pdf (same sha256), so it is cleaned directly.
            let (document, meta) = self.pipeline.reclean(self.writer.as_ref(), &stored, pdf, meta)?;
            return Ok(Some(summarize(&document, &meta)));
        }

        self.writer.mark_opened(id, now)?;
        info!(service = Self::NAME, doc_id = %id, "document marked opened");
        Ok(Some(DocumentSummary { last_opened_at: now, ..summary }))
    }
}

impl UseCase for ImportPdf {
    const NAME: &'static str = "ImportPdf";
    type Args = ImportPdfArgs;
    type Output = ImportedPdf;

    fn run(&self, ctx: &RequestContext, args: ImportPdfArgs) -> AppResult<ImportedPdf> {
        let file_name = normalize_file_name(&args.file_name);
        info!(service = Self::NAME, request_id = %ctx.request_id, file_name = %file_name, bytes = args.bytes.len(), "importing pdf");

        self.check_pdf(&args.bytes)?;
        let id = self.hasher.document_id(&args.bytes)?;
        let now = self.clock.now();
        info!(service = Self::NAME, doc_id = %id, "pdf accepted");

        if let Some(summary) = self.reopen(&id, &args.bytes, now)? {
            info!(service = Self::NAME, doc_id = %id, already_in_library = true, "pdf import finished");
            return Ok(ImportedPdf { summary, already_in_library: true });
        }

        let document = self.pipeline.process(&id, &file_name, &args.bytes)?;
        let meta = DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None };
        self.writer.save(&document, &args.bytes, &meta)?;
        info!(service = Self::NAME, doc_id = %id, already_in_library = false, "document saved");
        Ok(ImportedPdf { summary: summarize(&document, &meta), already_in_library: false })
    }
}

/// Keeps only the base name (browsers may send a path) and trims it.
fn normalize_file_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    if base.is_empty() { FALLBACK_FILE_NAME.to_string() } else { base.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::fakes::{FakeHasher, Library, PDF_BYTES, at};

    const MAX_BYTES: usize = 1024;

    fn import(library: &Library, file_name: &str, bytes: &[u8]) -> AppResult<ImportedPdf> {
        let service = ImportPdf::new(Arc::new(FakeHasher), library.pipeline(), library.reader(), library.writer(), library.clock(), MAX_BYTES);
        service.run(&RequestContext::new("test"), ImportPdfArgs { file_name: file_name.to_string(), bytes: bytes.to_vec() })
    }

    #[test]
    fn first_import_cleans_and_saves() {
        let library = Library::new(1);

        let imported = import(&library, "  /home/me/Book.pdf ", PDF_BYTES).unwrap();

        assert!(!imported.already_in_library);
        assert_eq!(imported.summary.source_file_name, "Book.pdf");
        assert_eq!(imported.summary.sentence_count, 5);
        assert_eq!(imported.summary.added_at, at(100));
        assert_eq!(imported.summary.id.as_str().len(), 64);
        assert_eq!(library.store.original_pdf(&imported.summary.id).as_deref(), Some(PDF_BYTES));
        assert_eq!(library.cleaner.calls(), 1);
    }

    #[test]
    fn second_import_is_already_in_library_without_cleaning_again() {
        let library = Library::new(1);
        let first = import(&library, "Book.pdf", PDF_BYTES).unwrap();
        library.clock.set(at(200));

        let second = import(&library, "Book.pdf", PDF_BYTES).unwrap();

        assert!(second.already_in_library);
        assert_eq!(second.summary.id, first.summary.id);
        assert_eq!(second.summary.last_opened_at, at(200));
        assert_eq!(library.store.summary(&first.summary.id).unwrap().last_opened_at, at(200));
        assert_eq!(library.cleaner.calls(), 1);
        assert_eq!(library.source.calls(), 1);
    }

    #[test]
    fn renamed_file_has_the_same_id() {
        let library = Library::new(1);
        let first = import(&library, "Book.pdf", PDF_BYTES).unwrap();

        let renamed = import(&library, "copy of book (2).pdf", PDF_BYTES).unwrap();

        assert!(renamed.already_in_library);
        assert_eq!(renamed.summary.id, first.summary.id);
        assert_eq!(renamed.summary.source_file_name, "Book.pdf");
    }

    #[test]
    fn newer_cleaner_version_re_cleans_and_keeps_bookkeeping() {
        let library = Library::new(1);
        let first = import(&library, "Book.pdf", PDF_BYTES).unwrap();
        library.store.set_position(&first.summary.id, 2);
        library.cleaner.set_version(2);
        library.clock.set(at(300));

        let again = import(&library, "Book.pdf", PDF_BYTES).unwrap();

        assert!(again.already_in_library);
        assert_eq!(library.cleaner.calls(), 2);
        assert_eq!(library.store.document(&first.summary.id).unwrap().cleaner_version, 2);
        assert_eq!((again.summary.added_at, again.summary.last_opened_at), (at(100), at(300)));
        assert_eq!(again.summary.last_sentence_id.map(|id| id.0), Some(2));
    }

    #[test]
    fn not_a_pdf_is_pdf_invalid() {
        let library = Library::new(1);

        let error = import(&library, "notes.txt", b"hello, this is text").unwrap_err();

        assert_eq!(error.primary(), ErrorName::PdfInvalid);
        assert_eq!(library.source.calls(), 0);
    }

    #[test]
    fn header_after_leading_junk_is_accepted() {
        let library = Library::new(1);

        let imported = import(&library, "junk.pdf", b"\x00\x00junk%PDF-1.4 rest").unwrap();

        assert!(!imported.already_in_library);
    }

    #[test]
    fn too_large_and_empty_uploads_are_rejected() {
        let library = Library::new(1);
        let mut large = PDF_BYTES.to_vec();
        large.resize(MAX_BYTES + 1, b' ');

        assert_eq!(import(&library, "big.pdf", &large).unwrap_err().primary(), ErrorName::UploadTooLarge);
        assert_eq!(import(&library, "empty.pdf", b"").unwrap_err().primary(), ErrorName::UploadMissingFile);
    }

    #[test]
    fn file_name_falls_back_when_blank() {
        assert_eq!(normalize_file_name("  "), FALLBACK_FILE_NAME);
        assert_eq!(normalize_file_name("C:\\fakepath\\Book.pdf"), "Book.pdf");
    }
}
