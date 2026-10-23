//! Builds the cleaner's `TextRun` fixtures (survey §5) in `backend/test-data/books/` from the books.
//! That folder is gitignored: the fixtures hold book text and must never be committed.
//! By default the ignored test only checks that the local fixtures still match the books (read-only).
//! To rewrite them (after changing `FIXTURE_PAGES` or the PDF reader), run explicitly:
//! `LECTRICE_REGEN_FIXTURES=1 cargo test -p lectrice-infra --test pdf_fixtures -- --ignored`.
//! Format: line 1 is `{ title, pages, outline }`, then one run per line:
//! `[page, left, bottom, right, top, baseline, size, lineEndHyphen, font, text]`.

mod pdf_support;

use std::fmt::Write as _;

use lectrice_domain::{PdfContent, PdfTextSource};
use pdf_support::{book, book_fixture_dir, text_source};
use serde_json::json;

const FIXTURE_PAGES: [(&str, &[u32]); 5] = [
    // p282 spells "unfortunately" unbroken, which the book vocabulary needs to join p9's `Unfortu-nately`.
    // Step T added K8s 4, 99, 121–124; DDS 153; GoF 397; CA 6, 32, 94, 169, 200; LinkedIn 212, 222.
    ("kubernetes-book", &[1, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 36, 60, 61, 68, 99, 121, 122, 123, 124, 228, 282, 299, 308]),
    ("designing-distributed-systems", &[4, 5, 15, 18, 23, 25, 26, 27, 29, 153, 159]),
    ("design-patterns", &[2, 11, 12, 13, 15, 21, 69, 100, 397, 416]),
    ("clean-architecture", &[5, 6, 20, 21, 25, 27, 28, 29, 32, 39, 53, 54, 74, 94, 169, 200, 335, 375]),
    ("linkedin-posts", &[1, 3, 4, 5, 6, 7, 8, 9, 10, 56, 92, 107, 212, 222, 368]),
];

/// Set to `1` to rewrite the local fixtures instead of comparing against them.
const REGEN_ENV: &str = "LECTRICE_REGEN_FIXTURES";

#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn cleaner_fixtures_match_the_books() {
    let regenerate = std::env::var(REGEN_ENV).is_ok_and(|value| value == "1");
    let source = text_source();
    let mut stale = Vec::new();
    for (key, pages) in FIXTURE_PAGES {
        let content = source.read_runs(&book(key).bytes()).unwrap();
        let path = book_fixture_dir().join(format!("{key}.jsonl"));
        let expected = fixture_text(&content, pages);
        if regenerate {
            std::fs::create_dir_all(book_fixture_dir()).unwrap();
            std::fs::write(&path, expected).unwrap();
            println!("{} written", path.display());
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(expected.as_str()) {
            stale.push(path.display().to_string());
        }
    }
    assert!(stale.is_empty(), "fixtures differ from the books: {stale:?}; rerun with {REGEN_ENV}=1 to rewrite them");
}

fn fixture_text(content: &PdfContent, pages: &[u32]) -> String {
    let kept_pages: Vec<_> = content.pages.iter().filter(|page| pages.contains(&page.index)).collect();
    let header = json!({ "title": content.title, "pages": kept_pages, "outline": content.outline });
    let mut text = format!("{header}\n");
    for run in content.runs.iter().filter(|run| pages.contains(&run.pdf_page)) {
        let bbox = run.bbox;
        let row = json!([
            run.pdf_page,
            round(bbox.left),
            round(bbox.bottom),
            round(bbox.right),
            round(bbox.top),
            round(run.baseline),
            round(run.font_size),
            run.line_end_hyphen,
            run.font_name,
            run.text
        ]);
        writeln!(text, "{row}").unwrap();
    }
    text
}

fn round(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}
