//! Survey §5 fixtures 16–21: Design Patterns (GoF): everything is Courier, code is the smaller size.
//! Needs the local book fixture (see `cleaning_support`); skips without it. Needles are kept to a few words.

mod cleaning_support;

use cleaning_support::{block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

fn book() -> Option<CleanedDocument> {
    clean("design-patterns")
}

#[test]
fn fixture_16_running_header_and_bold_page_number_dropped_label_inferred() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, "Design Patterns: Elements of Reusable Object-Oriented Software"));
    assert_eq!(label(&document, 11), Some("11"));
    let headings = kinds_on(&document, 11, BlockKind::Heading);
    assert_eq!(headings, ["1. Introduction"]);
    assert!(blocks_on(&document, 11).iter().all(|block| !shown(block).ends_with(' ')));
}

#[test]
fn fixture_17_citation_is_shown_but_not_spoken() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, "[AIS+77]. Even");
    assert!(spoken(paragraph).contains("twice\". Even"));
    assert!(!spoken(paragraph).contains("[AIS+77]"));
    assert!(kinds_on(&document, 12, BlockKind::Heading).contains(&"What is a Design Pattern?".to_string()));
}

#[test]
fn fixture_18_smaller_courier_is_code_and_joined_words_stay() {
    let Some(document) = book() else { return };
    let code = block(&document, "XWindowImp::DeviceRect");
    assert_eq!(code.kind, BlockKind::Code);
    let body_line = shown(code).lines().find(|line| line.contains("int x = round(")).map(str::to_string).unwrap();
    let indent = body_line.len() - body_line.trim_start().len();
    assert!(indent >= 4, "indent rebuilt from x");
    assert_eq!(block(&document, "stores theWindowImp").kind, BlockKind::Paragraph);
}

#[test]
fn fixture_19_table_caption_is_shown_only() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, "We classify design patterns").kind, BlockKind::Paragraph);
    assert!(!BlockKind::Caption.is_spoken_by_default());
}

#[test]
fn fixture_20_symbol_bullet_item_spans_two_lines() {
    let Some(document) = book() else { return };
    let items = kinds_on(&document, 100, BlockKind::ListItem);
    assert!(items[0].starts_with("a system should"));
    assert!(items[0].ends_with("represented."));
    assert!(!items[0].contains('\n'));
    assert!(kinds_on(&document, 100, BlockKind::Heading).contains(&"Applicability".to_string()));
}

#[test]
fn fixture_21_toc_dropped_bibliography_silent_figure_gap_splits_paragraphs() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 2).is_empty());
    let bibliography = blocks_on(&document, 416);
    assert!(!bibliography.is_empty() && bibliography.iter().all(|block| is_silent(block)));
    let before_figure = block(&document, "and pie chart");
    assert!(!shown(before_figure).contains("Taken at face"));
}

#[test]
fn page_cross_references_are_not_spoken() {
    let Some(document) = book() else { return };
    let heading = block(&document, "State (338)");
    assert_eq!(spoken(heading), "State");
}
