mod pdf_support;

use lectrice_domain::{ErrorName, PdfTextSource};
use pdf_support::{book, text_source};

#[test]
fn bytes_that_are_not_a_pdf_are_pdf_invalid() {
    let error = text_source().read_runs(b"hello, not a pdf").unwrap_err();
    assert_eq!(error.primary(), ErrorName::PdfInvalid);
}

/// The line-end hyphen arrives as a flagged run ending in `-`, and the font size is the effective one.
#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn book_e_page_9_has_a_flagged_line_end_hyphen_and_scaled_sizes() {
    let Some(book) = book("book-e") else { return };
    let content = text_source().read_runs(&book.bytes()).unwrap();
    assert_eq!(content.pages[8].label.as_deref(), Some("1"));
    let page_runs: Vec<_> = content.runs.iter().filter(|run| run.pdf_page == 9).collect();
    let broken = page_runs.iter().position(|run| run.line_end_hyphen && run.text.ends_with(book.needle("page_9_broken"))).expect("the broken word is flagged");
    assert!(page_runs[broken + 1].text.starts_with(book.needle("page_9_next")));
    assert!((page_runs[broken].font_size - 9.96).abs() < 0.01);
}

#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn book_a_body_size_is_the_scaled_size_not_the_nominal_one() {
    let Some(book) = book("book-a") else { return };
    let content = text_source().read_runs(&book.bytes()).unwrap();
    let hyphenated = content.runs.iter().find(|run| run.pdf_page == 25 && run.text.contains(book.needle("page_25_hyphen"))).expect("the hyphen on p25");
    assert!(hyphenated.line_end_hyphen);
    assert!((hyphenated.font_size - 15.0).abs() < 0.01, "nominal 20, effective 15");
    assert!(content.outline.len() > 200);
    assert!(content.pages.iter().all(|page| page.label.is_none()));
}
