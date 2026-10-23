//! Step T: problems found while reading whole books, one test per fix (pages added to the fixtures in step T).
//! Needs the local book fixtures (see `cleaning_support`); skips without them. Needles are kept to a few words;
//! `cleaning_invented.rs` repeats these cases with invented text.

mod cleaning_support;

use cleaning_support::{block, clean, has_text, is_silent, kinds_on, spoken};
use lectrice_domain::BlockKind;

#[test]
fn inline_code_wrapped_to_the_start_of_a_line_stays_in_its_paragraph() {
    let Some(document) = clean("kubernetes-book") else { return };
    let port = block(&document, "called svc-shield on port");
    assert_eq!(port.kind, BlockKind::Paragraph);
    assert!(spoken(port).ends_with("on port 8080."));
    let api = block(&document, "deploy an Ingress object");
    assert!(spoken(api).ends_with("networking.k8s.io/v1 API."));
    let Some(architecture) = clean("clean-architecture") else { return };
    let scenario = block(&architecture, "The OutputData is");
    assert!(spoken(scenario).ends_with("to the Presenter."));
}

#[test]
fn a_prose_line_mostly_in_inline_code_joins_its_paragraph() {
    let Some(document) = clean("kubernetes-book") else { return };
    let rollout = block(&document, "the following kubectl");
    assert!(spoken(rollout).ends_with("kubectl rollout commands."));
}

#[test]
fn inline_code_inside_a_list_item_is_spoken_with_its_spaces() {
    let Some(document) = clean("kubernetes-book") else { return };
    let item = block(&document, "Client sends traffic to");
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(spoken(item).ends_with(" shield.mcu.com or mcu.com/shield"));
}

#[test]
fn short_inline_code_listings_in_the_inline_style_stay_code() {
    let Some(document) = clean("clean-architecture") else { return };
    assert_eq!(block(&document, "/pickupAddress/%s/").kind, BlockKind::Code);
    let rules = block(&document, "that translation code");
    assert!(spoken(rules).ends_with("the BusinessRules."));
}

#[test]
fn a_sentence_that_starts_with_figure_is_spoken() {
    let Some(kubernetes) = clean("kubernetes-book") else { return };
    let sentence = block(&kubernetes, "Figure 8.2 shows");
    assert_eq!(sentence.kind, BlockKind::Paragraph);
    assert!(!is_silent(sentence));
    assert_eq!(block(&kubernetes, "Figure 8.2 Host-based routing").kind, BlockKind::Caption);
    let Some(distributed) = clean("designing-distributed-systems") else { return };
    assert_eq!(block(&distributed, "Figure 12-4 shows").kind, BlockKind::Paragraph);
}

#[test]
fn a_bold_word_or_figure_reference_wrapped_onto_its_own_line_continues_the_sentence() {
    let Some(kubernetes) = clean("kubernetes-book") else { return };
    assert!(!has_text_kind(&kubernetes, 121, BlockKind::Heading, "svc-shield"));
    let item = block(&kubernetes, "Deploy an app called");
    assert!(spoken(item).ends_with("called svc-shield"));
    let Some(architecture) = clean("clean-architecture") else { return };
    let paragraph = block(&architecture, "curve in Figure 1.5");
    assert!(spoken(paragraph).contains("in Figure 1.2. That"));
}

#[test]
fn a_list_label_on_its_own_line_is_a_separate_item() {
    let Some(document) = clean("linkedin-posts") else { return };
    let items = kinds_on(&document, 10, BlockKind::ListItem);
    assert!(items.contains(&"1. New York Times".to_string()), "{items:?}");
    assert!(block(&document, "The newspaper website").kind == BlockKind::Paragraph);
}

#[test]
fn dash_numbered_lines_are_list_items_and_carry_on_without_a_hanging_indent() {
    let Some(document) = clean("linkedin-posts") else { return };
    let items = kinds_on(&document, 212, BlockKind::ListItem);
    assert!(items.contains(&"1 - Database Indexing".to_string()) && items.contains(&"2 - Caching".to_string()), "{items:?}");
    let modification = block(&document, "Data Modification:");
    assert_eq!(modification.kind, BlockKind::ListItem);
    assert!(spoken(modification).ends_with("on a table."));
}

#[test]
fn double_spaced_lines_that_carry_on_a_sentence_join() {
    let Some(document) = clean("design-patterns") else { return };
    let definition = block(&document, "A class whose primary purpose");
    let text = spoken(definition);
    assert_eq!(text.matches(". ").count(), 2, "three sentences in one block");
    assert!(text.ends_with("cannot be instantiated."));
}

#[test]
fn centered_dedication_lines_join_their_full_first_line() {
    let Some(kubernetes) = clean("kubernetes-book") else { return };
    assert!(spoken(block(&kubernetes, "Education is about")).ends_with("lots of opportunities!"));
    let Some(architecture) = clean("clean-architecture") else { return };
    assert!(spoken(block(&architecture, "This book is dedicated")).ends_with("of my life."));
}

#[test]
fn short_bare_domains_are_spoken_so_the_sentence_stays_whole() {
    let Some(document) = clean("kubernetes-book") else { return };
    let rules = block(&document, "a host-based rule");
    assert!(spoken(rules).ends_with("arriving on shield.mcu.com"));
    assert!(has_text(&document, "Host-based: shield.mcu.com >> svc-shield"));
}

fn has_text_kind(document: &lectrice_domain::CleanedDocument, page: u32, kind: BlockKind, needle: &str) -> bool {
    kinds_on(document, page, kind).iter().any(|text| text.contains(needle))
}
