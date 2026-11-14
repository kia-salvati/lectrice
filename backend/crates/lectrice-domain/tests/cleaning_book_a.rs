//! Fixtures 22–30: book A, a calibre e-book (no headers, raised markers, ALL CAPS headings).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`.

mod cleaning_support;

use cleaning_support::{Book, block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::BlockKind;

fn book() -> Option<Book> {
    clean("book-a")
}

#[test]
fn fixture_22_marker_silent_footnote_shown_only_short_bare_domain_spoken() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, document.needle("marked_paragraph"));
    assert!(spoken(paragraph).contains(document.needle("marked_spoken")));
    let footnote = block(&document, document.needle("footnote"));
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
    assert!(shown(footnote).ends_with(document.needle("footnote_end")));
    // Short bare domains are read out so the sentence stays whole.
    let register = block(&document, document.needle("bare_domain"));
    assert!(spoken(register).contains(document.needle("bare_domain")));
}

#[test]
fn fixture_23_exponent_without_footnote_is_kept_on_screen_and_print_page_leftover_removed() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, document.needle("exponent")));
    assert!(has_text(&document, document.needle("after_exponent")));
    assert!(!has_text(&document, "{xx}"));
}

#[test]
fn fixture_24_real_hyphen_kept_and_part_heading_from_the_outline() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, document.needle("real_hyphen")));
    assert_eq!(kinds_on(&document, 25, BlockKind::Heading), [document.needle("part_heading")]);
    assert_eq!(block(&document, document.needle("part_heading")).level, Some(1));
}

#[test]
fn fixture_25_chapter_heading_takes_the_outline_casing() {
    let Some(document) = book() else { return };
    assert_eq!(kinds_on(&document, 27, BlockKind::Heading), [document.needle("chapter_heading")]);
}

#[test]
fn fixture_26_small_italic_block_quote_is_spoken() {
    let Some(document) = book() else { return };
    let quote = block(&document, document.needle("quote"));
    assert_eq!(quote.kind, BlockKind::Paragraph);
    assert!(!is_silent(quote));
    let headings = kinds_on(&document, 28, BlockKind::Heading);
    assert!(headings.iter().any(|heading| heading == document.needle("heading_1")) && headings.iter().any(|heading| heading == document.needle("heading_2")));
}

#[test]
fn fixture_27_code_image_links_dropped_code_listing_and_spoken_body() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, document.needle("code_image_link")));
    let listing = block(&document, document.needle("listing"));
    assert_eq!(listing.kind, BlockKind::Code);
    assert!(shown(listing).starts_with(document.needle("listing_start")));
    assert_eq!(block(&document, document.needle("after_listing")).kind, BlockKind::Paragraph);
}

#[test]
fn fixture_28_caption_with_its_credit_line_is_shown_only() {
    let Some(document) = book() else { return };
    let caption = block(&document, document.needle("caption"));
    assert_eq!(caption.kind, BlockKind::Caption);
    assert!(shown(caption).contains(document.needle("caption_credit")));
}

#[test]
fn fixture_29_footnote_with_urls_and_marker_after_a_word() {
    let Some(document) = book() else { return };
    assert!(is_silent(block(&document, document.needle("url_footnote"))));
    assert!(spoken(block(&document, document.needle("marked_word"))).contains(document.needle("marked_word_spoken")));
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
    let item = block(&document, document.needle("numbered_item"));
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(!is_silent(item));
}
