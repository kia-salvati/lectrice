//! Problems found while reading whole books, one test per fix (pages added to the fixtures for them).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`;
//! `cleaning_invented.rs` repeats these cases with invented text.

mod cleaning_support;

use cleaning_support::{block, clean, has_text, is_silent, kinds_on, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

#[test]
fn inline_code_wrapped_to_the_start_of_a_line_stays_in_its_paragraph() {
    let Some(document) = clean("book-e") else { return };
    let port = block(&document, document.needle("t_port"));
    assert_eq!(port.kind, BlockKind::Paragraph);
    assert!(spoken(port).ends_with(document.needle("t_port_end")));
    let api = block(&document, document.needle("t_api"));
    assert!(spoken(api).ends_with(document.needle("t_api_end")));
    let Some(book_a) = clean("book-a") else { return };
    let scenario = block(&book_a, book_a.needle("t_scenario"));
    assert!(spoken(scenario).ends_with(book_a.needle("t_scenario_end")));
}

#[test]
fn a_prose_line_mostly_in_inline_code_joins_its_paragraph() {
    let Some(document) = clean("book-e") else { return };
    let rollout = block(&document, document.needle("t_rollout"));
    assert!(spoken(rollout).ends_with(document.needle("t_rollout_end")));
}

#[test]
fn inline_code_inside_a_list_item_is_spoken_with_its_spaces() {
    let Some(document) = clean("book-e") else { return };
    let item = block(&document, document.needle("t_code_item"));
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(spoken(item).ends_with(document.needle("t_code_item_end")));
}

#[test]
fn short_inline_code_listings_in_the_inline_style_stay_code() {
    let Some(document) = clean("book-a") else { return };
    assert_eq!(block(&document, document.needle("t_inline_listing")).kind, BlockKind::Code);
    let rules = block(&document, document.needle("t_after_listing"));
    assert!(spoken(rules).ends_with(document.needle("t_after_listing_end")));
}

#[test]
fn a_sentence_that_starts_with_figure_is_spoken() {
    let Some(book_e) = clean("book-e") else { return };
    let sentence = block(&book_e, book_e.needle("t_figure_sentence"));
    assert_eq!(sentence.kind, BlockKind::Paragraph);
    assert!(!is_silent(sentence));
    assert_eq!(block(&book_e, book_e.needle("t_figure_caption")).kind, BlockKind::Caption);
    let Some(book_b) = clean("book-b") else { return };
    assert_eq!(block(&book_b, book_b.needle("t_figure_sentence")).kind, BlockKind::Paragraph);
}

#[test]
fn a_bold_word_or_figure_reference_wrapped_onto_its_own_line_continues_the_sentence() {
    let Some(book_e) = clean("book-e") else { return };
    assert!(!has_text_kind(&book_e, 121, BlockKind::Heading, book_e.needle("t_bold_word")));
    let item = block(&book_e, book_e.needle("t_bold_item"));
    assert!(spoken(item).ends_with(book_e.needle("t_bold_item_end")));
    let Some(book_a) = clean("book-a") else { return };
    let paragraph = block(&book_a, book_a.needle("t_figure_reference"));
    assert!(spoken(paragraph).contains(book_a.needle("t_figure_reference_spoken")));
}

#[test]
fn a_list_label_on_its_own_line_is_a_separate_item() {
    let Some(document) = clean("book-d") else { return };
    let items = kinds_on(&document, 10, BlockKind::ListItem);
    assert!(items.iter().any(|item| item == document.needle("t_label_item")), "{items:?}");
    assert!(block(&document, document.needle("t_after_label")).kind == BlockKind::Paragraph);
}

#[test]
fn dash_numbered_lines_are_list_items_and_carry_on_without_a_hanging_indent() {
    let Some(document) = clean("book-d") else { return };
    let items = kinds_on(&document, 212, BlockKind::ListItem);
    assert!(
        items.iter().any(|item| item == document.needle("t_dash_item_1")) && items.iter().any(|item| item == document.needle("t_dash_item_2")),
        "{items:?}"
    );
    let modification = block(&document, document.needle("t_carried_item"));
    assert_eq!(modification.kind, BlockKind::ListItem);
    assert!(spoken(modification).ends_with(document.needle("t_carried_item_end")));
}

#[test]
fn double_spaced_lines_that_carry_on_a_sentence_join() {
    let Some(document) = clean("book-c") else { return };
    let definition = block(&document, document.needle("t_definition"));
    let text = spoken(definition);
    assert_eq!(text.matches(". ").count(), 2, "three sentences in one block");
    assert!(text.ends_with(document.needle("t_definition_end")));
}

#[test]
fn centered_dedication_lines_join_their_full_first_line() {
    let Some(book_e) = clean("book-e") else { return };
    assert!(spoken(block(&book_e, book_e.needle("t_dedication"))).ends_with(book_e.needle("t_dedication_end")));
    let Some(book_a) = clean("book-a") else { return };
    assert!(spoken(block(&book_a, book_a.needle("t_dedication"))).ends_with(book_a.needle("t_dedication_end")));
}

#[test]
fn short_bare_domains_are_spoken_so_the_sentence_stays_whole() {
    let Some(document) = clean("book-e") else { return };
    let rules = block(&document, document.needle("t_host_rule"));
    assert!(spoken(rules).ends_with(document.needle("t_host_rule_end")));
    assert!(has_text(&document, document.needle("t_host_rule_shown")));
}

fn has_text_kind(document: &CleanedDocument, page: u32, kind: BlockKind, needle: &str) -> bool {
    kinds_on(document, page, kind).iter().any(|text| text.contains(needle))
}
