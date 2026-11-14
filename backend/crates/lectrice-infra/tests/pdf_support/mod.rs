#![allow(dead_code)]

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use lectrice_infra::pdf::PdfiumTextSource;
use serde::Deserialize;

/// Points at a `test-data` folder other than this checkout's (a second worktree uses the main checkout's).
pub const TEST_DATA_ENV: &str = "LECTRICE_TEST_DATA";
/// The books' file names and the text the tests look for: book data, so it stays in the gitignored folder.
const EXPECTED_FILE: &str = "expected.json";

/// The user's books on the Desktop, read in place. Tests that need them are `#[ignore]`.
pub const BOOK_KEYS: [&str; 5] = ["book-a", "book-b", "book-c", "book-d", "book-e"];

pub struct Book {
    pub key: &'static str,
    pub file_name: &'static str,
    needles: Option<&'static HashMap<String, String>>,
}

#[derive(Deserialize)]
struct Expected {
    files: HashMap<String, String>,
    #[serde(default)]
    needles: HashMap<String, HashMap<String, String>>,
}

/// The local `backend/test-data/books/` folder (or the one under `LECTRICE_TEST_DATA`).
pub fn book_fixture_dir() -> PathBuf {
    std::env::var_os(TEST_DATA_ENV).map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data"), PathBuf::from).join("books")
}

fn expected() -> Option<&'static Expected> {
    static EXPECTED: OnceLock<Option<Expected>> = OnceLock::new();
    EXPECTED
        .get_or_init(|| {
            let path = book_fixture_dir().join(EXPECTED_FILE);
            let text = std::fs::read_to_string(&path).ok()?;
            Some(serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
        })
        .as_ref()
}

/// The book, or `None` (with one skip line) when the local book data is missing.
pub fn book(key: &'static str) -> Option<Book> {
    let Some(expected) = expected() else {
        skip();
        return None;
    };
    let file_name = expected.files.get(key).unwrap_or_else(|| panic!("{EXPECTED_FILE} has no file name for {key}"));
    Some(Book { key, file_name: file_name.as_str(), needles: expected.needles.get(key) })
}

/// Every book, or `None` when the local book data is missing.
pub fn books() -> Option<Vec<Book>> {
    BOOK_KEYS.into_iter().map(book).collect()
}

/// Writes to the real stderr, not through `eprintln!`, which the test harness hides for passing tests.
fn skip() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        let path = book_fixture_dir().join(EXPECTED_FILE);
        let _ = writeln!(std::io::stderr(), "skipped: no local book fixtures ({} is missing; set {TEST_DATA_ENV} to a test-data folder)", path.display());
    }
}

impl Book {
    pub fn path(&self) -> PathBuf {
        std::env::home_dir().expect("home dir").join("Desktop").join(self.file_name)
    }

    pub fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.path()).expect("the book is on the Desktop")
    }

    /// The book text stored under `name` in the local `expected.json`.
    pub fn needle(&self, name: &str) -> &'static str {
        self.needles.and_then(|needles| needles.get(name)).map(String::as_str).unwrap_or_else(|| panic!("{EXPECTED_FILE} has no needle {}.{name}", self.key))
    }
}

/// One shared source: PDFium can be bound only once at a time per process, and tests run in parallel.
pub fn text_source() -> &'static PdfiumTextSource {
    static SOURCE: OnceLock<PdfiumTextSource> = OnceLock::new();
    SOURCE.get_or_init(|| PdfiumTextSource::new(&pdfium_library()).expect("libpdfium loads"))
}

pub fn pdfium_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/lib/libpdfium.so")
}
