//! Fixtures 1–8: book E (XeTeX, embedded labels, running header, mixed hyphens).
//! Needs the local book data (see `cleaning_support`); skips without it. The text looked for is in `expected.json`.

mod cleaning_support;

use cleaning_support::{Book, block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::BlockKind;

const BOOK: &str = "book-e";

fn book() -> Option<Book> {
    clean(BOOK)
}

#[test]
fn fixture_01_header_dropped_label_kept_hyphen_kept_bullets_stripped() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, document.needle("running_header")));
    assert_eq!(label(&document, 12), Some("4"));
    assert!(has_text(&document, document.needle("real_hyphen")));
    let headings = kinds_on(&document, 12, BlockKind::Heading);
    assert!(headings.iter().any(|heading| heading == document.needle("heading_1")) && headings.iter().any(|heading| heading == document.needle("heading_2")));
    let items = kinds_on(&document, 12, BlockKind::ListItem);
    assert_eq!(items[..2], [document.needle("item_1"), document.needle("item_2")]);
}

#[test]
fn fixture_02_preface_heading_and_syllable_break_joined() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, document.needle("preface")).kind, BlockKind::Heading);
    assert!(has_text(&document, document.needle("joined_break")));
    assert!(!has_text(&document, document.needle("broken_word")));
}

#[test]
fn fixture_03_footnote_marker_silent_and_footnote_shown_only() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, document.needle("marked_paragraph"));
    assert!(spoken(paragraph).contains(document.needle("marked_spoken")));
    assert!(shown(paragraph).contains(document.needle("marked_shown")));
    let footnote = block(&document, document.needle("footnote"));
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
}

#[test]
fn fixture_04_chapter_heading_without_header_and_marker_removed_from_speech() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, document.needle("chapter_heading")).kind, BlockKind::Heading);
    let marked = block(&document, document.needle("marker_sentence"));
    assert!(spoken(marked).contains(document.needle("marker_sentence")));
    assert!(!spoken(marked).contains(document.needle("marker_glued")));
    assert!(is_silent(block(&document, document.needle("silent_footnote"))));
}

#[test]
fn fixture_05_yaml_is_one_code_block_with_rebuilt_indent() {
    let Some(document) = book() else { return };
    let yaml = block(&document, document.needle("yaml"));
    assert_eq!(yaml.kind, BlockKind::Code);
    let text = shown(yaml);
    assert!(text.contains(document.needle("yaml_indent")));
    assert!(text.ends_with(document.needle("yaml_end")));
    assert_eq!(block(&document, document.needle("command")).kind, BlockKind::Code);
    assert_eq!(block(&document, document.needle("command_heading")).kind, BlockKind::Heading);
}

#[test]
fn fixture_06_terminal_output_stays_in_the_code_block_and_pod_name_keeps_its_hyphen() {
    let Some(document) = book() else { return };
    let output = block(&document, document.needle("output"));
    assert_eq!(output.kind, BlockKind::Code);
    assert!(shown(output).starts_with(document.needle("output_start")));
    assert!(spoken(block(&document, document.needle("pod_sentence"))).contains(document.needle("pod_spoken")));
    assert!(!has_text(&document, document.needle("pod_joined")));
}

#[test]
fn fixture_07_tables_are_shown_with_a_cue_and_body_is_spoken() {
    let Some(document) = book() else { return };
    let tables = kinds_on(&document, 228, BlockKind::Table);
    assert_eq!(tables.len(), 2);
    assert!(tables[0].starts_with(document.needle("table_1_start")));
    assert!(tables[1].contains(document.needle("table_2_row")));
    assert_eq!(BlockKind::Table.cue_text(), Some("Table."));
    assert_eq!(block(&document, document.needle("after_table")).kind, BlockKind::Paragraph);
}

#[test]
fn fixture_08_toc_and_empty_index_dropped_cover_without_blocks() {
    let Some(document) = book() else { return };
    for page in [1, 5, 6, 7, 8, 308] {
        assert!(blocks_on(&document, page).is_empty(), "page {page} should have no blocks");
    }
}

#[test]
fn terminology_appendix_is_a_table_with_wrapped_cells() {
    let Some(document) = book() else { return };
    let terminology = block(&document, document.needle("terminology"));
    assert_eq!(terminology.kind, BlockKind::Table);
    assert!(shown(terminology).contains(document.needle("terminology_wrap")));
}
