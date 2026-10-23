//! Cleans each of the user's five books end to end and prints the counts for review:
//! `cargo test --release -p lectrice-infra --test pdf_books -- --ignored --nocapture`.

mod pdf_support;

use std::time::Instant;

use lectrice_domain::cleaning::RuleBasedCleaner;
use lectrice_domain::{BlockKind, CleanedDocument, DocumentCleaner, PdfTextSource, SentenceSplitter};
use lectrice_infra::SentencexSplitter;
use pdf_support::{BOOKS, text_source};

#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn every_book_cleans_without_error() {
    let source = text_source();
    println!(
        "{:<30} {:>5} {:>6} {:>9} {:>5} {:>7} {:>5} {:>9} {:>7} {:>8}",
        "book", "pages", "blocks", "sentences", "code", "caption", "table", "footnotes", "read_ms", "clean_ms"
    );
    for book in &BOOKS {
        let bytes = book.bytes();
        let started = Instant::now();
        let content = source.read_runs(&bytes).unwrap();
        let read_ms = started.elapsed().as_millis();
        let started = Instant::now();
        let cleaned = RuleBasedCleaner.clean(content).unwrap();
        let clean_ms = started.elapsed().as_millis();

        let count = |kind: BlockKind| cleaned.blocks.iter().filter(|block| block.kind == kind).count();
        let footnotes = cleaned.blocks.iter().filter(|block| block.kind == BlockKind::Note && block.spans.iter().all(|span| !span.spoken)).count();
        println!(
            "{:<30} {:>5} {:>6} {:>9} {:>5} {:>7} {:>5} {:>9} {:>7} {:>8}",
            book.key,
            cleaned.page_count,
            cleaned.blocks.len(),
            sentence_count(&cleaned),
            count(BlockKind::Code),
            count(BlockKind::Caption),
            count(BlockKind::Table),
            footnotes,
            read_ms,
            clean_ms
        );
        assert!(cleaned.blocks.len() > 100, "{} has too few blocks", book.key);
    }
}

/// Sentences as the library would number them: each block's sentences plus one cue for code and tables.
fn sentence_count(cleaned: &CleanedDocument) -> usize {
    cleaned
        .blocks
        .iter()
        .map(|block| {
            let text: String = block.spans.iter().map(|span| span.text.as_str()).collect();
            SentencexSplitter.split(&text).len() + usize::from(block.kind.cue_text().is_some())
        })
        .sum()
}
