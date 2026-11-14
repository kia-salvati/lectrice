#![allow(dead_code)]

//! Loads the `TextRun` fixtures written by `lectrice-infra/tests/pdf_fixtures.rs` (a few pages of
//! each book) and helpers to look at the cleaned blocks.
//!
//! The fixtures and the text the tests look for (`expected.json`, "needles") come from the user's books, so they
//! live in the gitignored `backend/test-data/books/` and are never committed. `LECTRICE_TEST_DATA` can point at
//! another `test-data` folder (a second worktree uses the main checkout's). When the data is missing the book
//! tests print one skip line and return; `cleaning_invented.rs` covers the same rules with invented text.

use std::collections::HashMap;
use std::io::Write as _;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use lectrice_domain::cleaning::clean_content;
use lectrice_domain::{BlockKind, BoundingBox, CleanedBlock, CleanedDocument, OutlineItem, PdfContent, PdfPage, TextRun};
use serde::Deserialize;

#[derive(Deserialize)]
struct Header {
    title: Option<String>,
    pages: Vec<PdfPage>,
    outline: Vec<OutlineItem>,
}

type Row = (u32, f32, f32, f32, f32, f32, f32, bool, String, String);

/// How to rebuild the local book fixtures (needs the books in `~/Desktop`).
pub const REGEN_COMMAND: &str = "LECTRICE_REGEN_FIXTURES=1 cargo test -p lectrice-infra --test pdf_fixtures -- --ignored";
/// Points at a `test-data` folder other than this checkout's.
pub const TEST_DATA_ENV: &str = "LECTRICE_TEST_DATA";
const EXPECTED_FILE: &str = "expected.json";

#[derive(Deserialize)]
struct Expected {
    needles: HashMap<String, HashMap<String, String>>,
}

pub fn book_data_dir() -> PathBuf {
    std::env::var_os(TEST_DATA_ENV).map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data"), PathBuf::from).join("books")
}

/// A cleaned book plus the text its tests look for. Derefs to the document so the helpers take it as is.
pub struct Book {
    document: CleanedDocument,
    key: String,
    needles: &'static HashMap<String, String>,
}

impl Book {
    /// The book text stored under `name` in the local `expected.json`.
    pub fn needle(&self, name: &str) -> &'static str {
        self.needles.get(name).map(String::as_str).unwrap_or_else(|| panic!("{EXPECTED_FILE} has no needle {}.{name}", self.key))
    }
}

impl Deref for Book {
    type Target = CleanedDocument;

    fn deref(&self) -> &CleanedDocument {
        &self.document
    }
}

/// The book's runs, or `None` (with a skip message) when the local fixture is missing.
pub fn content(book: &str) -> Option<PdfContent> {
    let path = book_data_dir().join(format!("{book}.jsonl"));
    if !path.exists() {
        skip(&path);
        return None;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Some(parse(&text))
}

fn expected() -> Option<&'static Expected> {
    static EXPECTED: OnceLock<Option<Expected>> = OnceLock::new();
    EXPECTED
        .get_or_init(|| {
            let path = book_data_dir().join(EXPECTED_FILE);
            let text = std::fs::read_to_string(&path).ok()?;
            Some(serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
        })
        .as_ref()
}

/// Writes to the real stderr, not through `eprintln!`, which the test harness hides for passing tests.
fn skip(path: &Path) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        let message = format!(
            "skipped: no local book fixtures ({} is missing; set {TEST_DATA_ENV} to a test-data folder or rebuild it with `{REGEN_COMMAND}`)",
            path.display()
        );
        let _ = writeln!(std::io::stderr(), "{message}");
    }
}

/// Parses the fixture format: a `{ title, pages, outline }` header line, then one run per line.
pub fn parse(text: &str) -> PdfContent {
    let mut lines = text.lines();
    let header: Header = serde_json::from_str(lines.next().expect("a header line")).expect("a valid header");
    let runs = lines
        .map(|line| {
            let (pdf_page, left, bottom, right, top, baseline, font_size, line_end_hyphen, font_name, text): Row =
                serde_json::from_str(line).expect("a valid run");
            TextRun { pdf_page, text, font_name, font_size, bbox: BoundingBox { left, bottom, right, top }, baseline, line_end_hyphen }
        })
        .collect();
    PdfContent { title: header.title, pages: header.pages, runs, outline: header.outline }
}

/// The cleaned book with its needles, or `None` when the local data is missing (the test then returns early).
pub fn clean(book: &str) -> Option<Book> {
    let content = content(book)?;
    let Some(expected) = expected() else {
        skip(&book_data_dir().join(EXPECTED_FILE));
        return None;
    };
    let needles = expected.needles.get(book).unwrap_or_else(|| panic!("{EXPECTED_FILE} has no needles for {book}"));
    Some(Book { document: clean_content(&content), key: book.to_string(), needles })
}

pub fn shown(block: &CleanedBlock) -> String {
    block.spans.iter().map(|span| span.text.as_str()).collect()
}

/// The spoken text with whitespace collapsed, as the library stores `speechText`.
pub fn spoken(block: &CleanedBlock) -> String {
    let text: String = block.spans.iter().filter(|span| span.spoken).map(|span| span.text.as_str()).collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn is_silent(block: &CleanedBlock) -> bool {
    block.spans.iter().all(|span| !span.spoken)
}

/// The first block whose shown text contains `needle`.
pub fn block<'a>(document: &'a CleanedDocument, needle: &str) -> &'a CleanedBlock {
    document.blocks.iter().find(|block| shown(block).contains(needle)).unwrap_or_else(|| panic!("no block contains {needle:?}"))
}

pub fn has_text(document: &CleanedDocument, needle: &str) -> bool {
    document.blocks.iter().any(|block| shown(block).contains(needle))
}

pub fn blocks_on(document: &CleanedDocument, page: u32) -> Vec<&CleanedBlock> {
    document.blocks.iter().filter(|block| block.page_index == page).collect()
}

pub fn kinds_on(document: &CleanedDocument, page: u32, kind: BlockKind) -> Vec<String> {
    blocks_on(document, page).into_iter().filter(|block| block.kind == kind).map(shown).collect()
}

pub fn label(document: &CleanedDocument, page: u32) -> Option<&str> {
    document.pages.iter().find(|label| label.index == page).map(|label| label.label.as_str())
}
