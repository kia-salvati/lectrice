//! Survey §5 fixtures 9–15: Designing Distributed Systems (O'Reilly: U+2010 syllable breaks, footers, note boxes).
//! Needs the local book fixture (see `cleaning_support`); skips without it. Needles are kept to a few words.

mod cleaning_support;

use cleaning_support::{block, blocks_on, clean, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::{BlockKind, CleanedDocument};

fn book() -> Option<CleanedDocument> {
    clean("designing-distributed-systems")
}

#[test]
fn fixture_09_even_footer_dropped_unicode_hyphens_joined_heading_and_caption() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, "12 | Chapter 2: The Sidecar Pattern"));
    assert_eq!(label(&document, 26), Some("12"));
    assert!(has_text(&document, "internal network"));
    assert!(has_text(&document, "Many pre-existing"));
    assert!(kinds_on(&document, 26, BlockKind::Heading).contains(&"Dynamic Configuration with Sidecars".to_string()));
    assert_eq!(block(&document, "Figure 2-2.").kind, BlockKind::Caption);
}

#[test]
fn fixture_10_odd_footer_dropped_real_hyphen_kept() {
    let Some(document) = book() else { return };
    assert!(!has_text(&document, "Dynamic Configuration with Sidecars | 13"));
    assert!(has_text(&document, "notification"));
    assert!(!has_text(&document, "notifi‐"));
}

#[test]
fn fixture_11_stacked_chapter_heading_and_lone_page_number() {
    let Some(document) = book() else { return };
    let headings = kinds_on(&document, 15, BlockKind::Heading);
    assert_eq!(headings[0], "Chapter 1. Introduction");
    assert!(blocks_on(&document, 15).iter().all(|block| shown(block).trim() != "1"));
    assert!(has_text(&document, "object-oriented programming"));
}

#[test]
fn fixture_12_note_box_is_a_spoken_note_with_breaks_joined() {
    let Some(document) = book() else { return };
    let note = block(&document, "What is a pattern for");
    assert_eq!(note.kind, BlockKind::Note);
    assert!(!is_silent(note));
    assert!(!shown(note).contains('‐'));
}

#[test]
fn fixture_13_footnote_first_in_char_order_is_moved_out_and_marker_not_spoken() {
    let Some(document) = book() else { return };
    let footnote = block(&document, "Kubernetes is an open source");
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
    let paragraph = block(&document, "a pod in Kubernetes,");
    assert!(spoken(paragraph).contains("Kubernetes, but"));
    assert!(shown(paragraph).contains("Kubernetes,¹ but"));
}

#[test]
fn fixture_14_two_code_blocks_and_an_unspoken_url() {
    let Some(document) = book() else { return };
    let docker = block(&document, "$ docker run --pid=");
    assert_eq!(docker.kind, BlockKind::Code);
    assert_eq!(shown(docker).lines().count(), 4);
    let script = block(&document, "#!/bin/bash");
    assert_eq!(script.kind, BlockKind::Code);
    assert!(shown(script).ends_with("done"));
    let with_url = block(&document, "http://localhost:8080/");
    assert!(!spoken(with_url).contains("localhost:8080"));
}

#[test]
fn fixture_15_copyright_silent_toc_and_index_dropped() {
    let Some(document) = book() else { return };
    assert!(blocks_on(&document, 4).iter().all(|block| is_silent(block)));
    assert!(blocks_on(&document, 5).is_empty());
    assert!(blocks_on(&document, 159).is_empty());
}
