//! Survey §5 fixtures 1–8: The Kubernetes Book (XeTeX, embedded labels, running header, mixed hyphens).
//! Needs the local book fixture (see `cleaning_support`); skips without it. Needles are kept to a few words.

mod cleaning_support;

use cleaning_support::{block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

const BOOK: &str = "kubernetes-book";

fn book() -> Option<CleanedDocument> {
    clean(BOOK)
}

#[test]
fn fixture_01_header_dropped_label_kept_hyphen_kept_bullets_stripped() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, "1: Kubernetes primer 4"));
    assert_eq!(label(&document, 12), Some("4"));
    assert!(has_text(&document, "first-choice"));
    let headings = kinds_on(&document, 12, BlockKind::Heading);
    assert!(headings.contains(&"Cloud native".to_string()) && headings.contains(&"Microservices".to_string()));
    let items = kinds_on(&document, 12, BlockKind::ListItem);
    assert_eq!(items[..2], ["Web front-end".to_string(), "Catalog".to_string()]);
}

#[test]
fn fixture_02_preface_heading_and_syllable_break_joined() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, "Preface").kind, BlockKind::Heading);
    assert!(has_text(&document, "Unfortunately, a"));
    assert!(!has_text(&document, "Unfortu-"));
}

#[test]
fn fixture_03_footnote_marker_silent_and_footnote_shown_only() {
    let Some(document) = book() else { return };
    let paragraph = block(&document, "Foundation (CNCF)");
    assert!(spoken(paragraph).contains("Foundation (CNCF)."));
    assert!(shown(paragraph).contains("(CNCF)¹."));
    let footnote = block(&document, "https://www.cncf.io");
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
}

#[test]
fn fixture_04_chapter_heading_without_header_and_marker_removed_from_speech() {
    let Some(document) = book() else { return };
    assert_eq!(block(&document, "Getting Kubernetes").kind, BlockKind::Heading);
    assert!(spoken(block(&document, "as it’s free")).contains("as it’s free"));
    assert!(!spoken(block(&document, "as it’s free")).contains("free3"));
    assert!(is_silent(block(&document, "Docker Desktop is free")));
}

#[test]
fn fixture_05_yaml_is_one_code_block_with_rebuilt_indent() {
    let Some(document) = book() else { return };
    let yaml = block(&document, "kind: Pod");
    assert_eq!(yaml.kind, BlockKind::Code);
    let text = shown(yaml);
    assert!(text.contains("metadata:\n  name: "));
    assert!(text.ends_with("<Snip>"));
    assert_eq!(block(&document, "$ kubectl logs").kind, BlockKind::Code);
    assert_eq!(block(&document, "kubectl exec").kind, BlockKind::Heading);
}

#[test]
fn fixture_06_terminal_output_stays_in_the_code_block_and_pod_name_keeps_its_hyphen() {
    let Some(document) = book() else { return };
    let output = block(&document, "PID");
    assert_eq!(output.kind, BlockKind::Code);
    assert!(shown(output).starts_with("$ kubectl exec hello-pod"));
    assert!(spoken(block(&document, "the hello-pod Pod")).contains("hello-pod Pod"));
    assert!(!has_text(&document, "hellopod"));
}

#[test]
fn fixture_07_tables_are_shown_with_a_cue_and_body_is_spoken() {
    let Some(document) = book() else { return };
    let tables = kinds_on(&document, 228, BlockKind::Table);
    assert_eq!(tables.len(), 2);
    assert!(tables[0].starts_with("apiGroup | resource |"));
    assert!(tables[1].contains("create | POST | 201"));
    assert_eq!(BlockKind::Table.cue_text(), Some("Table."));
    assert_eq!(block(&document, "An empty set of").kind, BlockKind::Paragraph);
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
    let terminology = block(&document, "Admission controller");
    assert_eq!(terminology.kind, BlockKind::Table);
    assert!(shown(terminology).contains("\n | policies."));
}
