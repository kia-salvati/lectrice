//! Fixtures 16–21: book C, an all-Courier book (code is the smaller size).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`.

mod cleaning_support;

use cleaning_support::{Book, block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::BlockKind;

fn book() -> Option<Book> {
    clean("book-c")
}

#[test]
fn fixture_16_running_header_and_bold_page_number_dropped_label_inferred() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, document.needle("running_header")));
    assert_eq!(label(&document, 11), Some("11"));
    let headings = kinds_on(&document, 11, BlockKind::Heading);
    assert_eq!(headings, [document.needle("chapter_heading")]);
    assert!(blocks_on(&document, 11).iter().all(|block| !shown(block).ends_with(' ')));
}

#[test]
fn fixture_17_citation_is_shown_but_not_spoken() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, document.needle("cited_paragraph"));
    assert!(spoken(paragraph).contains(document.needle("cited_spoken")));
    assert!(!spoken(paragraph).contains(document.needle("citation")));
    assert!(kinds_on(&document, 12, BlockKind::Heading).iter().any(|heading| heading == document.needle("section_heading")));
}

#[test]
fn fixture_18_smaller_courier_is_code_and_joined_words_stay() {
    let Some(document) = book() else { return };
    let code = block(&document, document.needle("code"));
    assert_eq!(code.kind, BlockKind::Code);
    let body_line = shown(code).lines().find(|line| line.contains(document.needle("code_body_line"))).map(str::to_string).unwrap();
    let indent = body_line.len() - body_line.trim_start().len();
    assert!(indent >= 4, "indent rebuilt from x");
    assert_eq!(block(&document, document.needle("joined_words")).kind, BlockKind::Paragraph);
}

#[test]
fn fixture_19_table_caption_is_shown_only() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, document.needle("before_table")).kind, BlockKind::Paragraph);
    assert!(!BlockKind::Caption.is_spoken_by_default());
}

#[test]
fn fixture_20_symbol_bullet_item_spans_two_lines() {
    let Some(document) = book() else { return };
    let items = kinds_on(&document, 100, BlockKind::ListItem);
    assert!(items[0].starts_with(document.needle("item_start")));
    assert!(items[0].ends_with(document.needle("item_end")));
    assert!(!items[0].contains('\n'));
    assert!(kinds_on(&document, 100, BlockKind::Heading).iter().any(|heading| heading == document.needle("item_heading")));
}

#[test]
fn fixture_21_toc_dropped_bibliography_silent_figure_gap_splits_paragraphs() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 2).is_empty());
    let bibliography = blocks_on(&document, 416);
    assert!(!bibliography.is_empty() && bibliography.iter().all(|block| is_silent(block)));
    let before_figure = block(&document, document.needle("before_figure"));
    assert!(!shown(before_figure).contains(document.needle("after_figure")));
}

#[test]
fn page_cross_references_are_not_spoken() {
    let Some(document) = book() else { return };
    let heading = block(&document, document.needle("cross_reference"));
    assert_eq!(spoken(heading), document.needle("cross_reference_spoken"));
}
