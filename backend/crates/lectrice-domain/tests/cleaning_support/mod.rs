#![allow(dead_code)]

//! Loads the `TextRun` fixtures written by `lectrice-infra/tests/pdf_fixtures.rs` (a few pages of
//! each book, survey §5) and helpers to look at the cleaned blocks.
//!
//! The fixtures hold text from the user's books, so they live in the gitignored `backend/test-data/books/`
//! and are never committed. On a fresh clone they are missing and the book tests skip with a message;
//! `cleaning_invented.rs` covers the same rules with invented text.

use std::io::Write as _;
use std::path::{Path, PathBuf};
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

pub fn book_data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data/books")
}

/// The book's runs, or `None` (with a skip message) when the local fixture is missing.
pub fn content(book: &str) -> Option<PdfContent> {
    let path = book_data_dir().join(format!("{book}.jsonl"));
    if !path.exists() {
        warn_once(&path);
        return None;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Some(parse(&text))
}

/// Writes to the real stderr, not through `eprintln!`, which the test harness hides for passing tests.
fn warn_once(path: &Path) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        let message = format!("book tests skipped: {} is missing (local book data, not in git); rebuild it with `{REGEN_COMMAND}`", path.display());
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

/// The cleaned book, or `None` when its local fixture is missing (the test then returns early).
pub fn clean(book: &str) -> Option<CleanedDocument> {
    content(book).map(|content| clean_content(&content))
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
