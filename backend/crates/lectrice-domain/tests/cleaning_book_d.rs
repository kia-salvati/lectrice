//! Fixtures 31–35: book D, a Google Docs export (no headers, styled Unicode letters, emoji bullets).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`.

mod cleaning_support;

use cleaning_support::{Book, block, blocks_on, clean, has_text, kinds_on, shown, spoken};
use lectrice_domain::BlockKind;

fn book() -> Option<Book> {
    clean("book-d")
}

#[test]
fn fixture_31_toc_pages_dropped_including_the_leaked_entry() {
    let Some(document) = book() else { return };
    for page in 3..=7 {
        assert!(blocks_on(&document, page).is_empty(), "page {page} should have no blocks");
    }
}

#[test]
fn fixture_32_post_heading_paragraph_after_the_image_and_bold_bullet() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, document.needle("post_heading")).kind, BlockKind::Heading);
    assert_eq!(block(&document, document.needle("after_image")).kind, BlockKind::Paragraph);
    assert!(kinds_on(&document, 8, BlockKind::ListItem).iter().any(|item| item == document.needle("bullet_1")));
    assert!(kinds_on(&document, 9, BlockKind::ListItem).iter().any(|item| item == document.needle("bullet_2")));
}

#[test]
fn fixture_33_styled_letters_become_plain_text() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, document.needle("styled_letters")));
}

#[test]
fn fixture_34_emoji_bullet_stripped_and_variation_selector_removed() {
    let Some(document) = book() else { return };
    let item = block(&document, document.needle("emoji_item"));
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(shown(item).starts_with(document.needle("emoji_item")));
    assert!(!has_text(&document, "\u{FE0F}"));
    assert_eq!(spoken(block(&document, document.needle("question_item"))), document.needle("question_item"));
}

#[test]
fn fixture_35_image_only_pages_have_no_blocks() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 1).is_empty());
    assert!(blocks_on(&document, 368).is_empty());
}
