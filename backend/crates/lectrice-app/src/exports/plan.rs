//! What a whole-book export reads, in playback order, with chapter marks from the headings, cut into parts of
//! about `part_minutes` of listening each. Pure, no I/O.

use std::ops::Range;

use lectrice_domain::{BlockKind, Document, Sentence};

use crate::speech::{CHARS_PER_SECOND, SECONDS_PER_MINUTE, playable_in_order};

/// Headings up to this level become chapter marks (1 = parts/chapters, 2 = chapters/sections, depending on the book).
const CHAPTER_MAX_LEVEL: u8 = 2;
const TITLE_SEPARATOR: &str = " ";

#[derive(Debug, Clone, PartialEq)]
pub enum ExportStep<'a> {
    Chapter(String),
    Sentence(&'a Sentence),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan<'a> {
    pub steps: Vec<ExportStep<'a>>,
    pub sentences: u32,
    /// Characters to speak; progress and the time-left estimate are measured in them.
    pub spoken_chars: u64,
    /// Never empty when there are steps. The same document and settings always give the same parts, so a resumed
    /// export continues where the finished parts end.
    pub parts: Vec<PartPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartPlan {
    /// Indexes into `ExportPlan::steps`.
    pub steps: Range<usize>,
    pub sentences: u32,
    pub spoken_chars: u64,
}

/// The playback order (with `read_skipped` and cues exactly as the player reads them), with a chapter mark before each
/// heading of level ≤ 2. `max_sentences` keeps only the first sentences (a short export for testing). A new part starts
/// once the current one holds `part_minutes` of estimated 1x audio; 0 puts every sentence in its own part.
pub fn export_plan(document: &Document, read_skipped: bool, max_sentences: Option<u32>, part_minutes: u32) -> ExportPlan<'_> {
    let limit = max_sentences.map_or(usize::MAX, |limit| limit as usize);
    let mut plan = ExportPlan { steps: Vec::new(), sentences: 0, spoken_chars: 0, parts: Vec::new() };
    let mut previous_block = None;
    for (block_index, sentence) in playable_in_order(document, read_skipped).into_iter().take(limit) {
        if previous_block != Some(block_index) {
            previous_block = Some(block_index);
            if let Some(title) = chapter_title(document, block_index) {
                plan.steps.push(ExportStep::Chapter(title));
            }
        }
        plan.sentences += 1;
        plan.spoken_chars += spoken_chars(sentence);
        plan.steps.push(ExportStep::Sentence(sentence));
    }
    plan.parts = split_into_parts(&plan.steps, part_chars(part_minutes));
    plan
}

pub fn spoken_chars(sentence: &Sentence) -> u64 {
    sentence.spoken_text().trim().chars().count() as u64
}

fn part_chars(part_minutes: u32) -> u64 {
    (f64::from(part_minutes) * SECONDS_PER_MINUTE * CHARS_PER_SECOND) as u64
}

/// A part is closed before the step that follows its last full minute, so a chapter mark at a boundary opens the next part.
fn split_into_parts(steps: &[ExportStep<'_>], part_chars: u64) -> Vec<PartPlan> {
    let mut parts = Vec::new();
    let mut current = PartPlan { steps: 0..0, sentences: 0, spoken_chars: 0 };
    for (index, step) in steps.iter().enumerate() {
        if current.sentences > 0 && current.spoken_chars >= part_chars {
            let next = PartPlan { steps: index..index, sentences: 0, spoken_chars: 0 };
            parts.push(std::mem::replace(&mut current, next));
        }
        current.steps.end = index + 1;
        if let ExportStep::Sentence(sentence) = step {
            current.sentences += 1;
            current.spoken_chars += spoken_chars(sentence);
        }
    }
    if !current.steps.is_empty() {
        parts.push(current);
    }
    parts
}

fn chapter_title(document: &Document, block_index: usize) -> Option<String> {
    let block = &document.blocks[block_index];
    let is_chapter = block.kind == BlockKind::Heading && block.level.is_none_or(|level| level <= CHAPTER_MAX_LEVEL);
    if !is_chapter {
        return None;
    }
    let title = block.sentences.iter().map(|sentence| sentence.text.trim()).collect::<Vec<_>>().join(TITLE_SEPARATOR);
    (!title.is_empty()).then_some(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lectrice_domain::{Block, DocumentId, SentenceId};

    fn sentence(id: u32, text: &str) -> Sentence {
        Sentence { id: SentenceId(id), text: text.into(), page_index: 1, speech_text: None }
    }

    fn block(kind: BlockKind, level: Option<u8>, sentences: Vec<Sentence>, cue: Option<Sentence>) -> Block {
        Block { id: 0, kind, level, page_index: 1, spoken: kind.is_spoken_by_default(), text: None, sentences, cue }
    }

    /// Chapter 1 (level 1), a paragraph, a section (level 3, no mark), code with its cue, Chapter 2 (level 2), a paragraph.
    fn book() -> Document {
        Document {
            id: DocumentId::parse(&format!("{:064x}", 1)).unwrap(),
            title: "Book".into(),
            source_file_name: "book.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: Vec::new(),
            toc: Vec::new(),
            blocks: vec![
                block(BlockKind::Heading, Some(1), vec![sentence(0, "Chapter 1."), sentence(1, "Start")], None),
                block(BlockKind::Paragraph, None, vec![sentence(2, "One."), sentence(3, "Two.")], None),
                block(BlockKind::Heading, Some(3), vec![sentence(4, "Detail")], None),
                block(BlockKind::Code, None, vec![sentence(5, "let x = 1;")], Some(sentence(6, "Code example."))),
                block(BlockKind::Heading, Some(2), vec![sentence(7, "Chapter 2")], None),
                block(BlockKind::Paragraph, None, vec![sentence(8, "Three.")], None),
            ],
        }
    }

    fn describe(plan: &ExportPlan<'_>) -> Vec<String> {
        plan.steps
            .iter()
            .map(|step| match step {
                ExportStep::Chapter(title) => format!("# {title}"),
                ExportStep::Sentence(sentence) => sentence.id.0.to_string(),
            })
            .collect()
    }

    const WHOLE_BOOK_PART_MINUTES: u32 = 10;

    #[test]
    fn follows_playback_order_with_cues_and_marks_level_one_and_two_headings() {
        let document = book();
        let plan = export_plan(&document, false, None, WHOLE_BOOK_PART_MINUTES);

        assert_eq!(describe(&plan), ["# Chapter 1. Start", "0", "1", "2", "3", "4", "6", "# Chapter 2", "7", "8"]);
        assert_eq!(plan.sentences, 8);
        assert_eq!(plan.spoken_chars, ["Chapter 1.", "Start", "One.", "Two.", "Detail", "Code example.", "Chapter 2", "Three."].concat().len() as u64);
        assert_eq!(plan.parts, [PartPlan { steps: 0..10, sentences: 8, spoken_chars: plan.spoken_chars }], "a short book is one part");
    }

    #[test]
    fn read_skipped_reads_the_code_instead_of_its_cue() {
        let document = book();
        let plan = export_plan(&document, true, None, WHOLE_BOOK_PART_MINUTES);

        assert_eq!(describe(&plan)[5..7], ["4".to_string(), "5".to_string()]);
    }

    #[test]
    fn max_sentences_keeps_only_the_start() {
        let document = book();
        let plan = export_plan(&document, false, Some(3), WHOLE_BOOK_PART_MINUTES);

        assert_eq!(describe(&plan), ["# Chapter 1. Start", "0", "1", "2"]);
        assert_eq!(plan.sentences, 3);
    }

    #[test]
    fn parts_hold_about_part_minutes_and_a_chapter_mark_opens_the_next_part() {
        let minute = "x".repeat((SECONDS_PER_MINUTE * CHARS_PER_SECOND) as usize);
        let paragraph = |id: u32| block(BlockKind::Paragraph, None, vec![sentence(id, &minute)], None);
        let mut document = book();
        document.blocks = vec![paragraph(0), paragraph(1), paragraph(2), block(BlockKind::Heading, Some(1), vec![sentence(3, "Two")], None), paragraph(4)];

        let plan = export_plan(&document, false, None, 3);

        let ranges: Vec<Range<usize>> = plan.parts.iter().map(|part| part.steps.clone()).collect();
        assert_eq!(describe(&plan), ["0", "1", "2", "# Two", "3", "4"]);
        assert_eq!(ranges, [0..3, 3..6]);
        assert_eq!(plan.parts.iter().map(|part| part.sentences).sum::<u32>(), plan.sentences);
    }

    #[test]
    fn zero_part_minutes_gives_every_sentence_its_own_part() {
        let document = book();
        let plan = export_plan(&document, false, None, 0);

        let first_steps: Vec<String> = plan.parts.iter().map(|part| describe(&plan)[part.steps.start].clone()).collect();
        assert_eq!(first_steps, ["# Chapter 1. Start", "1", "2", "3", "4", "6", "# Chapter 2", "8"]);
    }
}
