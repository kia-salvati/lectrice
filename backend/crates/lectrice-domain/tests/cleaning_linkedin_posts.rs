//! Survey §5 fixtures 31–35: LinkedIn Posts (Google Docs: no headers, styled Unicode letters, emoji bullets).
//! Needs the local book fixture (see `cleaning_support`); skips without it. Needles are kept to a few words.

mod cleaning_support;

use cleaning_support::{block, blocks_on, clean, has_text, kinds_on, shown, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

fn book() -> Option<CleanedDocument> {
    clean("linkedin-posts")
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
    assert_eq!(block(&document, "Big Endian vs Little Endian").kind, BlockKind::Heading);
    assert_eq!(block(&document, "Microprocessor architectures").kind, BlockKind::Paragraph);
    assert!(kinds_on(&document, 8, BlockKind::ListItem).contains(&"Little Endian".to_string()));
    assert!(kinds_on(&document, 9, BlockKind::ListItem).contains(&"Big Endian".to_string()));
}

#[test]
fn fixture_33_styled_letters_become_plain_text() {
    let Some(document) = book() else { return };
    assert!(has_text(&document, "When we merge"));
}

#[test]
fn fixture_34_emoji_bullet_stripped_and_variation_selector_removed() {
    let Some(document) = book() else { return };
    let item = block(&document, "What is Sensitive Data?");
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(shown(item).starts_with("What is Sensitive Data?"));
    assert!(!has_text(&document, "\u{FE0F}"));
    assert_eq!(spoken(block(&document, "What CSS does?")), "What CSS does?");
}

#[test]
fn fixture_35_image_only_pages_have_no_blocks() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 1).is_empty());
    assert!(blocks_on(&document, 368).is_empty());
}
