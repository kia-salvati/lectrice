//! What an export reads, in playback order, with chapter marks from the table of contents (or the headings), cut
//! into parts of about `part_minutes` of listening each. Pure, no I/O.

use std::ops::Range;

use lectrice_domain::{AppError, AppResult, Block, BlockKind, Document, ErrorName, ExportRange, Sentence, SentenceId};

use crate::speech::{CHARS_PER_SECOND, SECONDS_PER_MINUTE, playable_in_order};

/// TOC entries and headings up to this level become chapter marks (1 = parts/chapters, 2 = chapters/sections).
const CHAPTER_MAX_LEVEL: u8 = 2;
/// Longer chapter titles are cut (players show one line).
const MAX_CHAPTER_TITLE_CHARS: usize = 120;
const TITLE_SEPARATOR: &str = " ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanOptions {
    /// Read code, tables and captions instead of their cues.
    pub read_skipped: bool,
    /// Keeps only the first sentences (a short export for testing).
    pub max_sentences: Option<u32>,
    pub part_minutes: u32,
    /// Only these sentences (a partial export).
    pub range: Option<ExportRange>,
}

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
    /// Never empty when there are steps. The same document and options always give the same parts, so a resumed
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

/// The playback order (with `read_skipped` and cues exactly as the player reads them) inside the range, with a chapter
/// mark before the first sentence of each chapter. The first sentence also gets the mark of the chapter it is in, so
/// a range that starts mid-chapter opens with that chapter's title. When several chapters start at the same sentence
/// (a part and its first chapter), the deepest one is kept. A new part starts once the current one holds
/// `part_minutes` of estimated 1x audio; 0 puts every sentence in its own part.
pub fn export_plan(document: &Document, options: PlanOptions) -> ExportPlan<'_> {
    let limit = options.max_sentences.map_or(usize::MAX, |limit| limit as usize);
    let points = chapter_points(document);
    let mut next_point = 0;
    let mut plan = ExportPlan { steps: Vec::new(), sentences: 0, spoken_chars: 0, parts: Vec::new() };
    let in_range = |sentence: &&Sentence| options.range.is_none_or(|range| range.contains(sentence.id));
    let sentences = playable_in_order(document, options.read_skipped).into_iter().map(|(_, sentence)| sentence).filter(in_range).take(limit);
    for sentence in sentences {
        let mut title = None;
        while let Some((_, point_title)) = points.get(next_point).filter(|(start, _)| *start <= sentence.id) {
            title = Some(point_title.clone());
            next_point += 1;
        }
        if let Some(title) = title {
            plan.steps.push(ExportStep::Chapter(title));
        }
        plan.sentences += 1;
        plan.spoken_chars += spoken_chars(sentence);
        plan.steps.push(ExportStep::Sentence(sentence));
    }
    plan.parts = split_into_parts(&plan.steps, part_chars(options.part_minutes));
    plan
}

/// A range must run forwards and stay inside the document.
pub fn check_range(document: &Document, range: ExportRange) -> AppResult<()> {
    if range.from_sentence > range.to_sentence || range.to_sentence.0 >= document.sentence_count() {
        let detail = format!("range {}..={} is not inside 0..{}", range.from_sentence, range.to_sentence, document.sentence_count());
        return Err(AppError::new(ErrorName::RequestInvalid).with_detail(detail));
    }
    Ok(())
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

/// Where chapters start, in reading order: the TOC entries of level ≤ 2 (so the marks match the chapters list), or the
/// headings of level ≤ 2 when the book has no TOC.
fn chapter_points(document: &Document) -> Vec<(SentenceId, String)> {
    let mut points: Vec<(SentenceId, String)> = if document.toc.is_empty() {
        document.blocks.iter().filter_map(heading_point).collect()
    } else {
        document.toc.iter().filter(|entry| entry.level <= CHAPTER_MAX_LEVEL).map(|entry| (entry.sentence_id, entry.title.clone())).collect()
    };
    for (_, title) in &mut points {
        *title = title.trim().chars().take(MAX_CHAPTER_TITLE_CHARS).collect::<String>().trim_end().to_string();
    }
    points.retain(|(_, title)| !title.is_empty());
    points.sort_by_key(|(start, _)| *start);
    points
}

fn heading_point(block: &Block) -> Option<(SentenceId, String)> {
    let is_chapter = block.kind == BlockKind::Heading && block.level.is_none_or(|level| level <= CHAPTER_MAX_LEVEL);
    let first = block.sentences.first().filter(|_| is_chapter)?;
    let title = block.sentences.iter().map(|sentence| sentence.text.trim()).collect::<Vec<_>>().join(TITLE_SEPARATOR);
    Some((first.id, title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lectrice_domain::{DocumentId, TocEntry};

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

    fn toc_entry(id: u32, title: &str, level: u8, sentence_id: u32) -> TocEntry {
        TocEntry { id, title: title.into(), level, sentence_id: SentenceId(sentence_id), page_index: 1 }
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

    fn options(read_skipped: bool, max_sentences: Option<u32>, part_minutes: u32) -> PlanOptions {
        PlanOptions { read_skipped, max_sentences, part_minutes, range: None }
    }

    fn whole_book() -> PlanOptions {
        options(false, None, WHOLE_BOOK_PART_MINUTES)
    }

    fn range(from: u32, to: u32) -> ExportRange {
        ExportRange { from_sentence: SentenceId(from), to_sentence: SentenceId(to) }
    }

    #[test]
    fn follows_playback_order_with_cues_and_marks_level_one_and_two_headings() {
        let document = book();
        let plan = export_plan(&document, whole_book());

        assert_eq!(describe(&plan), ["# Chapter 1. Start", "0", "1", "2", "3", "4", "6", "# Chapter 2", "7", "8"]);
        assert_eq!(plan.sentences, 8);
        assert_eq!(plan.spoken_chars, ["Chapter 1.", "Start", "One.", "Two.", "Detail", "Code example.", "Chapter 2", "Three."].concat().len() as u64);
        assert_eq!(plan.parts, [PartPlan { steps: 0..10, sentences: 8, spoken_chars: plan.spoken_chars }], "a short book is one part");
    }

    /// C23: with a TOC, its level 1–2 entries are the marks (titles cut at 120 chars); level 3 and headings are not.
    #[test]
    fn the_toc_gives_the_chapter_marks() {
        let mut document = book();
        let long = "L".repeat(130);
        document.toc = vec![toc_entry(0, "Part One", 1, 0), toc_entry(1, " First ", 2, 0), toc_entry(2, "Deep", 3, 4), toc_entry(3, &long, 2, 6)];

        let plan = export_plan(&document, whole_book());

        let expected_long = format!("# {}", "L".repeat(MAX_CHAPTER_TITLE_CHARS));
        assert_eq!(describe(&plan), ["# First", "0", "1", "2", "3", "4", &expected_long, "6", "7", "8"]);
    }

    #[test]
    fn read_skipped_reads_the_code_instead_of_its_cue() {
        let document = book();
        let plan = export_plan(&document, options(true, None, WHOLE_BOOK_PART_MINUTES));

        assert_eq!(describe(&plan)[5..7], ["4".to_string(), "5".to_string()]);
    }

    #[test]
    fn max_sentences_keeps_only_the_start() {
        let document = book();
        let plan = export_plan(&document, options(false, Some(3), WHOLE_BOOK_PART_MINUTES));

        assert_eq!(describe(&plan), ["# Chapter 1. Start", "0", "1", "2"]);
        assert_eq!(plan.sentences, 3);
    }

    /// C30: only the range is planned, and a range that starts mid-chapter opens with that chapter's mark.
    #[test]
    fn a_range_plans_only_its_sentences_and_opens_with_the_current_chapter() {
        let document = book();

        let mid_chapter = export_plan(&document, PlanOptions { range: Some(range(2, 7)), ..whole_book() });
        let at_chapter = export_plan(&document, PlanOptions { range: Some(range(7, 8)), ..whole_book() });

        assert_eq!(describe(&mid_chapter), ["# Chapter 1. Start", "2", "3", "4", "6", "# Chapter 2", "7"]);
        assert_eq!(mid_chapter.sentences, 5);
        assert_eq!(describe(&at_chapter), ["# Chapter 2", "7", "8"]);
    }

    #[test]
    fn a_range_must_run_forwards_inside_the_document() {
        let document = book();

        assert!(check_range(&document, range(0, 8)).is_ok());
        for bad in [range(5, 4), range(3, 9)] {
            assert_eq!(check_range(&document, bad).unwrap_err().primary(), ErrorName::RequestInvalid);
        }
    }

    #[test]
    fn parts_hold_about_part_minutes_and_a_chapter_mark_opens_the_next_part() {
        let minute = "x".repeat((SECONDS_PER_MINUTE * CHARS_PER_SECOND) as usize);
        let paragraph = |id: u32| block(BlockKind::Paragraph, None, vec![sentence(id, &minute)], None);
        let mut document = book();
        document.blocks = vec![paragraph(0), paragraph(1), paragraph(2), block(BlockKind::Heading, Some(1), vec![sentence(3, "Two")], None), paragraph(4)];

        let plan = export_plan(&document, options(false, None, 3));

        let ranges: Vec<Range<usize>> = plan.parts.iter().map(|part| part.steps.clone()).collect();
        assert_eq!(describe(&plan), ["0", "1", "2", "# Two", "3", "4"]);
        assert_eq!(ranges, [0..3, 3..6]);
        assert_eq!(plan.parts.iter().map(|part| part.sentences).sum::<u32>(), plan.sentences);
    }

    #[test]
    fn zero_part_minutes_gives_every_sentence_its_own_part() {
        let document = book();
        let plan = export_plan(&document, options(false, None, 0));

        let first_steps: Vec<String> = plan.parts.iter().map(|part| describe(&plan)[part.steps.start].clone()).collect();
        assert_eq!(first_steps, ["# Chapter 1. Start", "1", "2", "3", "4", "6", "# Chapter 2", "8"]);
    }
}
