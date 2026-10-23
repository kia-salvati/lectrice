use std::ops::Range;

use lectrice_domain::cleaning::build_toc;
use lectrice_domain::{Block, BlockId, CleanedBlock, CleanedDocument, Document, DocumentId, Sentence, SentenceId, SentenceSplitter, TextSpan};

const PDF_EXTENSION: &str = ".pdf";

/// What a document is besides its cleaned content.
pub(crate) struct DocumentOrigin<'a> {
    pub id: &'a DocumentId,
    pub source_file_name: &'a str,
    pub cleaner_version: u32,
}

/// Splits every cleaned block into sentences and numbers blocks and sentences from 0 without gaps
/// (a block's sentences first, then its cue).
pub(crate) fn build_document(origin: DocumentOrigin<'_>, cleaned: CleanedDocument, splitter: &dyn SentenceSplitter) -> Document {
    let mut next_sentence_id = 0;
    let blocks = cleaned
        .blocks
        .iter()
        .filter(|block| block.spans.iter().any(|span| !span.text.trim().is_empty()))
        .zip(0..)
        .map(|(block, id)| build_block(id, block, splitter, &mut next_sentence_id))
        .collect::<Vec<_>>();
    let toc = build_toc(&cleaned.outline, &blocks);
    let title = cleaned.title.map(|title| title.trim().to_string()).filter(|title| !title.is_empty());
    Document {
        id: origin.id.clone(),
        title: title.unwrap_or_else(|| title_from_file_name(origin.source_file_name)),
        source_file_name: origin.source_file_name.to_string(),
        page_count: cleaned.page_count,
        cleaner_version: origin.cleaner_version,
        pages: cleaned.pages,
        toc,
        blocks,
    }
}

fn title_from_file_name(file_name: &str) -> String {
    let lowercase = file_name.to_ascii_lowercase();
    let stem = if lowercase.ends_with(PDF_EXTENSION) { &file_name[..file_name.len() - PDF_EXTENSION.len()] } else { file_name };
    stem.trim().to_string()
}

/// A span's place in the block's joined text.
struct PlacedSpan {
    range: Range<usize>,
    page_index: u32,
    spoken: bool,
}

fn place_spans(spans: &[TextSpan]) -> (String, Vec<PlacedSpan>) {
    let mut text = String::new();
    let mut placed = Vec::with_capacity(spans.len());
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        placed.push(PlacedSpan { range: start..text.len(), page_index: span.page_index, spoken: span.spoken });
    }
    (text, placed)
}

fn build_block(id: BlockId, block: &CleanedBlock, splitter: &dyn SentenceSplitter, next_sentence_id: &mut u32) -> Block {
    let (text, spans) = place_spans(&block.spans);
    let sentences = splitter.split(&text).into_iter().filter_map(|range| build_sentence(&text, &spans, range, block.page_index, next_sentence_id)).collect();
    let cue =
        block.kind.cue_text().map(|cue| Sentence { id: take_id(next_sentence_id), text: cue.to_string(), page_index: block.page_index, speech_text: None });
    let spoken = block.kind.is_spoken_by_default();
    Block { id, kind: block.kind, level: block.level, page_index: block.page_index, spoken, text: (!spoken).then_some(text), sentences, cue }
}

fn build_sentence(text: &str, spans: &[PlacedSpan], range: Range<usize>, block_page: u32, next_sentence_id: &mut u32) -> Option<Sentence> {
    let shown = text.get(range.clone())?;
    let page_index = spans.iter().find(|span| span.range.end > range.start).map_or(block_page, |span| span.page_index);
    let speech_text = speech_text(text, spans, &range).filter(|spoken| spoken != shown);
    Some(Sentence { id: take_id(next_sentence_id), text: shown.to_string(), page_index, speech_text })
}

/// The sentence without its unspoken spans, or `None` when every part of it is spoken.
fn speech_text(text: &str, spans: &[PlacedSpan], range: &Range<usize>) -> Option<String> {
    let overlapping = || spans.iter().filter(|span| span.range.start < range.end && span.range.end > range.start);
    if overlapping().all(|span| span.spoken) {
        return None;
    }
    let spoken: String =
        overlapping().filter(|span| span.spoken).filter_map(|span| text.get(span.range.start.max(range.start)..span.range.end.min(range.end))).collect();
    Some(spoken.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn take_id(next_sentence_id: &mut u32) -> SentenceId {
    let id = SentenceId(*next_sentence_id);
    *next_sentence_id += 1;
    id
}

#[cfg(test)]
mod tests {
    use lectrice_domain::{BlockKind, OutlineItem, PageLabel};

    use super::*;
    use crate::documents::fakes::{FakeSplitter, id_for};

    fn span(text: &str, page_index: u32, spoken: bool) -> TextSpan {
        TextSpan { text: text.to_string(), page_index, spoken }
    }

    fn block(kind: BlockKind, spans: Vec<TextSpan>) -> CleanedBlock {
        CleanedBlock { kind, level: None, page_index: spans[0].page_index, spans }
    }

    fn build(cleaned: CleanedDocument) -> Document {
        let id = id_for(1);
        build_document(DocumentOrigin { id: &id, source_file_name: "My Book.PDF", cleaner_version: 3 }, cleaned, &FakeSplitter)
    }

    #[test]
    fn numbers_blocks_and_sentences_in_order_with_cues_last() {
        let cleaned = CleanedDocument {
            title: Some(" Clean Code ".into()),
            page_count: 9,
            pages: vec![PageLabel { index: 1, label: "i".into() }],
            outline: vec![OutlineItem { title: "Intro".into(), level: 1, pdf_page: Some(1) }],
            blocks: vec![
                block(BlockKind::Heading, vec![span("Intro", 1, true)]),
                block(BlockKind::Paragraph, vec![span("   ", 1, true)]),
                block(BlockKind::Paragraph, vec![span("One. Two.", 1, true)]),
                block(BlockKind::Code, vec![span("let x = 1;", 2, true)]),
            ],
        };

        let document = build(cleaned);

        assert_eq!(document.title, "Clean Code");
        assert_eq!(document.cleaner_version, 3);
        assert_eq!(document.page_count, 9);
        assert_eq!(document.blocks.iter().map(|block| block.id).collect::<Vec<_>>(), [0, 1, 2]);
        let paragraph = &document.blocks[1];
        assert_eq!(paragraph.sentences.iter().map(|s| (s.id.0, s.text.as_str())).collect::<Vec<_>>(), [(1, "One."), (2, "Two.")]);
        assert!(paragraph.spoken && paragraph.text.is_none() && paragraph.cue.is_none());
        let code = &document.blocks[2];
        assert!(!code.spoken);
        assert_eq!(code.text.as_deref(), Some("let x = 1;"));
        assert_eq!(code.sentences[0].id, SentenceId(3));
        assert_eq!(code.cue.as_ref().map(|cue| (cue.id, cue.text.as_str(), cue.page_index)), Some((SentenceId(4), "Code example.", 2)));
        assert_eq!(document.toc.iter().map(|entry| (entry.title.as_str(), entry.sentence_id)).collect::<Vec<_>>(), [("Intro", SentenceId(0))]);
    }

    #[test]
    fn title_falls_back_to_the_file_name() {
        let document = build(CleanedDocument { title: Some("  ".into()), ..CleanedDocument::default() });

        assert_eq!(document.title, "My Book");
        assert_eq!(document.source_file_name, "My Book.PDF");
    }

    #[test]
    fn sentences_take_the_page_they_start_on() {
        let cleaned = CleanedDocument {
            blocks: vec![block(BlockKind::Paragraph, vec![span("First. Sec", 4, true), span("ond. Third.", 5, true)])],
            ..Default::default()
        };

        let document = build(cleaned);

        let pages: Vec<(&str, u32)> = document.blocks[0].sentences.iter().map(|s| (s.text.as_str(), s.page_index)).collect();
        assert_eq!(pages, [("First.", 4), ("Second.", 4), ("Third.", 5)]);
        assert_eq!(document.blocks[0].page_index, 4);
    }

    #[test]
    fn unspoken_spans_are_left_out_of_speech_text() {
        let cleaned = CleanedDocument {
            blocks: vec![block(BlockKind::Paragraph, vec![span("See ", 1, true), span("[12]", 1, false), span(" for more. Plain.", 1, true)])],
            ..Default::default()
        };

        let document = build(cleaned);

        let first = &document.blocks[0].sentences[0];
        assert_eq!(first.text, "See [12] for more.");
        assert_eq!(first.speech_text.as_deref(), Some("See for more."));
        assert_eq!(document.blocks[0].sentences[1].speech_text, None);
    }
}
