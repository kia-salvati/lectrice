//! The human-readable dump: headings, paragraphs with `[p. 37]` page markers, unspoken text in
//! `⟦…⟧`, and skipped blocks labelled with what playback says instead.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::RangeInclusive;

use lectrice_domain::{BlockKind, CleanedBlock, CleanedDocument, Document};

const CODE_INDENT: &str = "    ";

pub fn render(document: &Document, cleaned: &CleanedDocument, pages: Option<&RangeInclusive<u32>>) -> String {
    let labels: HashMap<u32, &str> = cleaned.pages.iter().map(|page| (page.index, page.label.as_str())).collect();
    let page_name = |index: u32| labels.get(&index).map_or_else(|| format!("pdf {index}"), |label| format!("{label} · pdf {index}"));
    let sentences = document.sentence_count();

    let mut out = String::new();
    let _ = writeln!(out, "# {}\npages: {}, blocks: {}, sentences (cues included): {}", document.title, document.page_count, document.blocks.len(), sentences);
    let mut current_page = None;
    for block in cleaned.blocks.iter().filter(|block| pages.is_none_or(|range| range.contains(&block.page_index))) {
        if current_page != Some(block.page_index) {
            let _ = writeln!(out, "\n[p. {}]", page_name(block.page_index));
            current_page = Some(block.page_index);
        }
        let body = block_body(block, &page_name);
        if let Some(last_page) = block.spans.last().map(|span| span.page_index) {
            current_page = Some(last_page);
        }
        let _ = writeln!(out, "\n{}", labelled(block, &body));
    }
    out
}

fn labelled(block: &CleanedBlock, body: &str) -> String {
    let all_silent = block.spans.iter().all(|span| !span.spoken);
    match block.kind {
        BlockKind::Heading => format!("{} {body}", "#".repeat(usize::from(block.level.unwrap_or(1)) + 1)),
        BlockKind::Paragraph if all_silent => format!("[SHOWN, NOT SPOKEN] {body}"),
        BlockKind::Paragraph => body.to_string(),
        BlockKind::ListItem => format!("• {body}"),
        BlockKind::Note if all_silent => format!("[FOOTNOTE — not spoken] {body}"),
        BlockKind::Note => format!("[NOTE] {body}"),
        BlockKind::Caption => format!("[CAPTION — not spoken] {body}"),
        BlockKind::Code | BlockKind::Table => {
            let name = if block.kind == BlockKind::Code { "CODE" } else { "TABLE" };
            let cue = block.kind.cue_text().unwrap_or_default();
            let indented: Vec<String> = body.lines().map(|line| format!("{CODE_INDENT}{line}")).collect();
            format!("[{name} — spoken: {cue}]\n{}", indented.join("\n"))
        }
    }
}

/// The block's text with page markers where it crosses a page and unspoken parts in `⟦…⟧`
/// (left unmarked when the whole block is unspoken).
fn block_body(block: &CleanedBlock, page_name: &impl Fn(u32) -> String) -> String {
    let all_silent = block.spans.iter().all(|span| !span.spoken);
    let mut body = String::new();
    let mut page = block.page_index;
    for span in &block.spans {
        if span.page_index != page {
            page = span.page_index;
            let _ = write!(body, " [p. {}] ", page_name(page));
        }
        if span.spoken || all_silent {
            body.push_str(&span.text);
        } else {
            let _ = write!(body, "⟦{}⟧", span.text);
        }
    }
    body
}
