//! Survey §5 fixtures 22–30: Clean Architecture (calibre e-book: no headers, raised markers, ALL CAPS headings).
//! Needs the local book fixture (see `cleaning_support`); skips without it. Needles are kept to a few words.

mod cleaning_support;

use cleaning_support::{block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

fn book() -> Option<CleanedDocument> {
    clean("clean-architecture")
}

#[test]
fn fixture_22_marker_silent_footnote_shown_only_short_bare_domain_spoken() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, "and put her");
    assert!(spoken(paragraph).contains("put her in front"));
    let footnote = block(&document, "very likely would");
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
    assert!(shown(footnote).ends_with("programmers."));
    // Step T: short bare domains are read out so the sentence stays whole.
    let register = block(&document, "informit.com/register");
    assert!(spoken(register).contains("informit.com/register"));
}

#[test]
fn fixture_23_exponent_without_footnote_is_kept_on_screen_and_print_page_leftover_removed() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, "10²² more"));
    assert!(has_text(&document, "machines."));
    assert!(!has_text(&document, "{xx}"));
}

#[test]
fn fixture_24_real_hyphen_kept_and_part_heading_from_the_outline() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, "billion-dollar"));
    assert_eq!(kinds_on(&document, 25, BlockKind::Heading), ["PART I Introduction"]);
    assert_eq!(block(&document, "PART I Introduction").level, Some(1));
}

#[test]
fn fixture_25_chapter_heading_takes_the_outline_casing() {
    let Some(document) = book() else { return };
    assert_eq!(kinds_on(&document, 27, BlockKind::Heading), ["Chapter 1 What Is Design and Architecture?"]);
}

#[test]
fn fixture_26_small_italic_block_quote_is_spoken() {
    let Some(document) = book() else { return };
    let quote = block(&document, "The goal of software architecture");
    assert_eq!(quote.kind, BlockKind::Paragraph);
    assert!(!is_silent(quote));
    let headings = kinds_on(&document, 28, BlockKind::Heading);
    assert!(headings.contains(&"The Goal?".to_string()) && headings.contains(&"Case Study".to_string()));
}

#[test]
fn fixture_27_code_image_links_dropped_code_listing_and_spoken_body() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, "Click here to view code image"));
    let listing = block(&document, "struct Point;");
    assert_eq!(listing.kind, BlockKind::Code);
    assert!(shown(listing).starts_with("point.h\n"));
    assert_eq!(block(&document, "The users of point.h").kind, BlockKind::Paragraph);
}

#[test]
fn fixture_28_caption_with_its_credit_line_is_shown_only() {
    let Some(document) = book() else { return };
    let caption = block(&document, "Figure 1.1 Growth of");
    assert_eq!(caption.kind, BlockKind::Caption);
    assert!(shown(caption).contains("Reproduced with permission"));
}

#[test]
fn fixture_29_footnote_with_urls_and_marker_after_a_word() {
    let Some(document) = book() else { return };
    assert!(is_silent(block(&document, "For example, Agile")));
    assert!(spoken(block(&document, "many different publications")).contains("publications over"));
}

#[test]
fn fixture_30_index_dropped_image_pages_empty_copyright_silent() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 335).is_empty());
    assert!(blocks_on(&document, 375).is_empty());
    let copyright = blocks_on(&document, 5);
    assert!(!copyright.is_empty() && copyright.iter().all(|block| is_silent(block)));
    assert_eq!(label(&document, 27), None);
}

#[test]
fn numbered_list_in_the_body_is_not_a_footnote() {
    let Some(document) = book() else { return };
    let item = block(&document, "Urgent and important");
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(!is_silent(item));
}
