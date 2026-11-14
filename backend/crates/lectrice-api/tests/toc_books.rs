//! The table of contents of each of the user's five books, end to end (PDFium → cleaner → sentences → toc):
//! `cargo test --release -p lectrice-api --test toc_books -- --ignored --nocapture` prints the first entries.
//! Reads the PDFs from `~/Desktop` in place and skips a book that isn't there; their file names and expected titles come from
//! the gitignored `backend/test-data/books/expected.json` (or `LECTRICE_TEST_DATA`).

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lectrice_app::documents::DocumentPipeline;
use lectrice_domain::cleaning::{RuleBasedCleaner, build_toc};
use lectrice_domain::{ContentHasher, Document, TocEntry};
use lectrice_infra::{PdfiumTextSource, SentencexSplitter, Sha256ContentHasher};
use serde::Deserialize;

const PRINTED_ENTRIES: usize = 30;
const FALLBACK_PRINTED_ENTRIES: usize = 12;
/// Points at a `test-data` folder other than this checkout's (a second worktree uses the main checkout's).
const TEST_DATA_ENV: &str = "LECTRICE_TEST_DATA";
/// The books' file names and expected chapter titles: book data, so it stays in the gitignored folder.
const EXPECTED_FILE: &str = "books/expected.json";

#[derive(Deserialize)]
struct ExpectedData {
    files: HashMap<String, String>,
    toc: HashMap<String, Expected>,
}

#[derive(Deserialize)]
struct Expected {
    entries: usize,
    first: Vec<(String, u8)>,
    /// An entry further in, with its level.
    also: (String, u8),
}

#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn every_book_has_a_table_of_contents_that_starts_at_its_first_chapter() {
    let Some(expected) = expected_data() else { return };
    let mut keys: Vec<&String> = expected.toc.keys().collect();
    keys.sort();
    let books: Vec<(&str, &Expected, Vec<u8>)> = keys
        .into_iter()
        .filter_map(|key| {
            let file_name = expected.files.get(key).unwrap_or_else(|| panic!("{EXPECTED_FILE} has no file name for {key}"));
            let bytes = std::fs::read(book_path(file_name)).ok();
            if bytes.is_none() {
                println!("skipped, not on this machine: {key}");
            }
            bytes.map(|bytes| (file_name.as_str(), &expected.toc[key], bytes))
        })
        .collect();
    if books.is_empty() {
        return;
    }
    // PDFium is loaded only when there is a book to read: a fresh clone has neither.
    let pipeline = pipeline();
    for (file_name, book, bytes) in &books {
        let document = &document(&pipeline, file_name, bytes);
        print_toc(document);
        let first: Vec<(String, u8)> = document.toc.iter().take(book.first.len()).map(|entry| (entry.title.clone(), entry.level)).collect();
        assert_eq!(first, book.first, "{file_name}");
        assert_eq!(document.toc.len(), book.entries, "{file_name}");
        assert!(document.toc.iter().any(|entry| (entry.title.as_str(), entry.level) == (book.also.0.as_str(), book.also.1)), "{file_name}: {:?}", book.also);
        assert_well_formed(document);

        let from_headings = Document { toc: build_toc(&[], &document.blocks), ..document.clone() };
        println!("\n{} without its outline: {} entries from headings", document.title, from_headings.toc.len());
        from_headings.toc.iter().take(FALLBACK_PRINTED_ENTRIES).for_each(|entry| println!("{}", line(document, entry)));
        assert!(from_headings.toc.len() > book.entries / 2, "{file_name}");
        assert_well_formed(&from_headings);
    }
}

fn assert_well_formed(document: &Document) {
    assert_eq!(document.toc.iter().map(|entry| entry.level).min(), Some(1));
    assert!(document.toc.iter().all(|entry| (1..=3).contains(&entry.level) && !entry.title.is_empty()));
    assert!(document.toc.windows(2).all(|pair| pair[0].sentence_id < pair[1].sentence_id && pair[1].level <= pair[0].level + 1));
    let sentence_count = document.sentence_count();
    assert!(document.toc.iter().all(|entry| entry.sentence_id.0 < sentence_count));
}

fn print_toc(document: &Document) {
    println!("\n{} ({} entries)", document.title, document.toc.len());
    for entry in document.toc.iter().take(PRINTED_ENTRIES) {
        println!("{}", line(document, entry));
    }
}

fn line(document: &Document, entry: &TocEntry) -> String {
    let label =
        document.pages.iter().find(|page| page.index == entry.page_index).map_or_else(|| format!("pdf {}", entry.page_index), |page| page.label.clone());
    format!("{}{} · p. {label} · s{}", "  ".repeat(usize::from(entry.level - 1)), entry.title, entry.sentence_id)
}

fn pipeline() -> DocumentPipeline {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/lib/libpdfium.so");
    let source = PdfiumTextSource::new(&library).expect("libpdfium loads");
    DocumentPipeline::new(Arc::new(source), Arc::new(RuleBasedCleaner), Arc::new(SentencexSplitter))
}

/// The expected tables of contents, or `None` (with a skip line) when the local book data is missing.
fn expected_data() -> Option<ExpectedData> {
    let path = std::env::var_os(TEST_DATA_ENV).map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data"), PathBuf::from).join(EXPECTED_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        // The real stderr: the harness hides `println!` for passing tests.
        let _ = writeln!(std::io::stderr(), "skipped: no local book fixtures ({} is missing; set {TEST_DATA_ENV} to a test-data folder)", path.display());
        return None;
    };
    Some(serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
}

/// The books are the user's own and never committed.
fn book_path(file_name: &str) -> PathBuf {
    std::env::home_dir().expect("home dir").join("Desktop").join(file_name)
}

fn document(pipeline: &DocumentPipeline, file_name: &str, bytes: &[u8]) -> Document {
    let id = Sha256ContentHasher.document_id(bytes).expect("hashable");
    pipeline.process(&id, file_name, bytes).expect("the book cleans")
}
