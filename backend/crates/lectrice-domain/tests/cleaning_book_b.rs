//! Fixtures 9–15: book B (U+2010 syllable breaks, footers, note boxes).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`.

mod cleaning_support;

use cleaning_support::{Book, block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::BlockKind;

const SYLLABLE_BREAK: char = '\u{2010}';

fn book() -> Option<Book> {
    clean("book-b")
}

#[test]
fn fixture_09_even_footer_dropped_unicode_hyphens_joined_heading_and_caption() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, document.needle("even_footer")));
    assert_eq!(label(&document, 26), Some("12"));
    assert!(has_text(&document, document.needle("joined_break")));
    assert!(has_text(&document, document.needle("real_hyphen")));
    assert!(kinds_on(&document, 26, BlockKind::Heading).iter().any(|heading| heading == document.needle("heading")));
    assert_eq!(block(&document, document.needle("caption")).kind, BlockKind::Caption);
}

#[test]
fn fixture_10_odd_footer_dropped_real_hyphen_kept() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, document.needle("odd_footer")));
    assert!(has_text(&document, document.needle("joined_word")));
    assert!(!has_text(&document, document.needle("broken_word")));
}

#[test]
fn fixture_11_stacked_chapter_heading_and_lone_page_number() {
    let Some(document) = book() else { return };
    let headings = kinds_on(&document, 15, BlockKind::Heading);
    assert_eq!(headings[0], document.needle("chapter_heading"));
    assert!(blocks_on(&document, 15).iter().all(|block| shown(block).trim() != "1"));
    assert!(has_text(&document, document.needle("chapter_text")));
}

#[test]
fn fixture_12_note_box_is_a_spoken_note_with_breaks_joined() {
    let Some(document) = book() else { return };
    let note = block(&document, document.needle("note"));
    assert_eq!(note.kind, BlockKind::Note);
    assert!(!is_silent(note));
    assert!(!shown(note).contains(SYLLABLE_BREAK));
}

#[test]
fn fixture_13_footnote_first_in_char_order_is_moved_out_and_marker_not_spoken() {
    let Some(document) = book() else { return };
    let footnote = block(&document, document.needle("footnote"));
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
    let paragraph = block(&document, document.needle("marked_paragraph"));
    assert!(spoken(paragraph).contains(document.needle("marked_spoken")));
    assert!(shown(paragraph).contains(document.needle("marked_shown")));
}

#[test]
fn fixture_14_two_code_blocks_and_an_unspoken_url() {
    let Some(document) = book() else { return };
    let docker = block(&document, document.needle("command"));
    assert_eq!(docker.kind, BlockKind::Code);
    assert_eq!(shown(docker).lines().count(), 4);
    let script = block(&document, "#!/bin/bash");
    assert_eq!(script.kind, BlockKind::Code);
    assert!(shown(script).ends_with("done"));
    let with_url = block(&document, document.needle("url"));
    assert!(!spoken(with_url).contains(document.needle("url_host")));
}

#[test]
fn fixture_15_copyright_silent_toc_and_index_dropped() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 4).iter().all(|block| is_silent(block)));
    assert!(blocks_on(&document, 5).is_empty());
    assert!(blocks_on(&document, 159).is_empty());
}
