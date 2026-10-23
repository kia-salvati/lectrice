//! Rule 16 and the block side of rules 8b, 9, 11–15: lines become blocks. Paragraphs join across
//! lines and pages; footnotes are placed after the block that was open when their page ended.

use super::block_text::BlockText;
use super::code::code_lines;
use super::footnotes::Footnote;
use super::headings::{RawHeading, name_headings};
use super::hyphenation::{LineJoin, Vocabulary};
use super::line_kinds::{Column, LineKind, ends_sentence, list_glyph_length, starts_lowercase};
use super::lines::Line;
use super::profile::Profile;
use super::speech_filters::silent_ranges;
use super::tables::{row_text, table_columns};
use crate::cleaned::{CleanedBlock, TextSpan};
use crate::document::BlockKind;
use crate::pdf::OutlineItem;

/// Lines of one paragraph are at most this many expected pitches apart.
pub const MAX_PARAGRAPH_PITCHES: f32 = 1.35;
const MAX_CAPTION_PITCHES: f32 = 2.0;
/// A full line that stops mid-sentence still joins a lowercase next line this many pitches below (GoF's
/// double-spaced glossary).
const MAX_CONTINUATION_PITCHES: f32 = 2.0;
/// A line is centered when its left and right margins differ by less than this share of its size.
const CENTERED_TOLERANCE_RATIO: f32 = 2.0;
/// Stacked heading lines (`CHAPTER 1` over `Introduction`) are at most this many sizes apart.
const MAX_HEADING_STACK_GAP_RATIO: f32 = 2.5;
const MAX_HEADING_STACK_LINES: usize = 4;
/// A line indented more than this share of its size starts a new paragraph.
const INDENT_RATIO: f32 = 0.8;
/// List continuation lines are indented at least this share of the size past the bullet.
const LIST_HANG_RATIO: f32 = 0.3;
/// A line ending a sentence this far (in sizes) before the column's right edge ends its paragraph.
const SHORT_LINE_RATIO: f32 = 2.5;
const SAME_SIZE_TOLERANCE_RATIO: f32 = 0.05;
const SUPERSCRIPT_DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
const CODE_LINE_SEPARATOR: &str = "\n";
const PROSE_LINE_SEPARATOR: &str = " ";

/// A content line with its kind and its offset from the page's text column.
#[derive(Debug, Clone)]
pub struct PlacedLine {
    pub line: Line,
    pub kind: LineKind,
    pub indent: f32,
    /// Distance from the line's right end to the column's right edge.
    pub right_gap: f32,
}

impl PlacedLine {
    pub fn new(line: Line, kind: LineKind, column: Column) -> Self {
        Self { indent: line.left - column.left, right_gap: column.right - line.right, line, kind }
    }

    /// Centered lines (dedications) change their indent on every line without starting a new paragraph.
    fn is_centered(&self) -> bool {
        self.indent > self.line.size && (self.indent - self.right_gap).abs() <= self.line.size * CENTERED_TOLERANCE_RATIO
    }
}

/// One page as the assembler sees it: its content lines in reading order and its footnotes.
#[derive(Debug, Clone)]
pub struct PageContent {
    pub silent: bool,
    pub lines: Vec<PlacedLine>,
    pub footnotes: Vec<Footnote>,
}

struct OpenBlock {
    kind: LineKind,
    silent: bool,
    lines: Vec<PlacedLine>,
}

pub struct Assembler<'a> {
    profile: &'a Profile,
    vocabulary: &'a Vocabulary,
    outline: &'a [OutlineItem],
    blocks: Vec<CleanedBlock>,
    /// Heading blocks (by index in `blocks`) waiting for their outline title and level.
    headings: Vec<(usize, RawHeading)>,
    open: Option<OpenBlock>,
    pending_footnotes: Vec<Footnote>,
}

impl<'a> Assembler<'a> {
    pub fn new(profile: &'a Profile, vocabulary: &'a Vocabulary, outline: &'a [OutlineItem]) -> Self {
        Self { profile, vocabulary, outline, blocks: Vec::new(), headings: Vec::new(), open: None, pending_footnotes: Vec::new() }
    }

    pub fn assemble(mut self, pages: Vec<PageContent>) -> Vec<CleanedBlock> {
        for page in pages {
            for placed in page.lines {
                self.add(placed, page.silent);
            }
            self.pending_footnotes.extend(page.footnotes);
            if self.open.is_none() {
                self.flush_footnotes();
            }
        }
        self.close();
        self.name_headings();
        self.blocks
    }

    fn name_headings(&mut self) {
        let raw: Vec<RawHeading> = self.headings.iter().map(|(_, heading)| heading.clone()).collect();
        for ((index, heading), (title, level)) in self.headings.iter().zip(name_headings(self.outline, &raw)) {
            let block = &mut self.blocks[*index];
            let silent = block.spans.iter().all(|span| !span.spoken);
            let mut text = BlockText::default();
            text.push(&title, heading.page);
            block.spans = finish_spans(text, silent);
            block.level = Some(level);
        }
    }

    fn add(&mut self, placed: PlacedLine, silent: bool) {
        let continues = self.open.as_ref().is_some_and(|open| open.silent == silent && self.continues(open, &placed));
        if continues {
            if let Some(open) = self.open.as_mut() {
                open.lines.push(placed);
            }
            return;
        }
        self.close();
        self.open = Some(OpenBlock { kind: placed.kind, silent, lines: vec![placed] });
    }

    fn continues(&self, open: &OpenBlock, next: &PlacedLine) -> bool {
        let previous = open.lines.last().expect("an open block has a line");
        let first = &open.lines[0];
        match (open.kind, next.kind) {
            (LineKind::Code, LineKind::Code) | (LineKind::TableRow, LineKind::TableRow) => next.line.page <= previous.line.page + 1,
            (LineKind::Heading, LineKind::Heading) => open.lines.len() < MAX_HEADING_STACK_LINES && self.stacks_under(previous, next),
            (LineKind::Caption, LineKind::Body | LineKind::NoteBox) => self.continues_caption(previous, next),
            (LineKind::ListItem, LineKind::Body) => self.continues_list_item(open, previous, next),
            (LineKind::Note, LineKind::Body) => self.continues_prose(previous, next) && (next.indent - first.indent).abs() <= next.line.size * INDENT_RATIO,
            (LineKind::NoteBox, LineKind::NoteBox) | (LineKind::Body, LineKind::Body) => self.continues_prose(previous, next),
            _ => false,
        }
    }

    fn stacks_under(&self, upper: &PlacedLine, lower: &PlacedLine) -> bool {
        let gap = upper.line.baseline - lower.line.baseline;
        upper.line.page == lower.line.page && gap > 0.0 && gap <= upper.line.size.max(lower.line.size) * MAX_HEADING_STACK_GAP_RATIO
    }

    /// Caption lines (a credit line under CA's `Figure 1.1 …`) may sit a bit further apart than prose lines.
    fn continues_caption(&self, previous: &PlacedLine, next: &PlacedLine) -> bool {
        let (upper, lower) = (&previous.line, &next.line);
        let styled_apart = lower.font != self.profile.body_font || !self.profile.is_body_size(lower.size);
        let same_size = (upper.size - lower.size).abs() <= self.profile.body_size * SAME_SIZE_TOLERANCE_RATIO;
        let pitch = upper.baseline - lower.baseline;
        let close_enough = pitch > 0.0 && pitch <= self.profile.expected_pitch(upper.size.max(lower.size)) * MAX_CAPTION_PITCHES;
        styled_apart && same_size && upper.page == lower.page && close_enough
    }

    fn continues_list_item(&self, open: &OpenBlock, previous: &PlacedLine, next: &PlacedLine) -> bool {
        let first = &open.lines[0];
        // Items without a hanging indent (`1 - Step One: … it can be a read,` over `write, …`)
        // go on when the next line carries on the sentence.
        let carries_on =
            starts_lowercase(&next.line.text) && !ends_sentence(&previous.line.text) && next.indent >= first.indent - next.line.size * LIST_HANG_RATIO;
        let hangs = next.indent >= first.indent + next.line.size * LIST_HANG_RATIO || carries_on;
        let bold_label_ends = open.lines.len() == 1 && first.line.all_bold && !next.line.all_bold;
        // A first line with room left for the next line's first word was broken on purpose: it is a label
        // (`1. New York Times` over `The newspaper website …`), not a wrapped item.
        let short_label_ends = open.lines.len() == 1 && first.right_gap > first_word_width(&next.line) + next.line.size;
        hangs && !bold_label_ends && !short_label_ends && self.continues_prose_ignoring_indent(previous, next)
    }

    fn continues_prose(&self, previous: &PlacedLine, next: &PlacedLine) -> bool {
        let both_centered = previous.is_centered() && next.is_centered();
        // A centered paragraph's first line can fill the column (dedications): a centered line that carries on
        // its sentence is not an indent.
        let full_previous = previous.right_gap <= previous.line.size * SHORT_LINE_RATIO;
        let centered_continuation = next.is_centered() && full_previous && !ends_sentence(&previous.line.text);
        let indented = next.indent > previous.indent + next.line.size * INDENT_RATIO && !both_centered && !centered_continuation;
        let new_page_indented = next.line.page != previous.line.page && next.indent > next.line.size * INDENT_RATIO;
        !indented && !new_page_indented && self.continues_prose_ignoring_indent(previous, next)
    }

    fn continues_prose_ignoring_indent(&self, previous: &PlacedLine, next: &PlacedLine) -> bool {
        let (upper, lower) = (&previous.line, &next.line);
        if (upper.size - lower.size).abs() > self.profile.body_size * SAME_SIZE_TOLERANCE_RATIO {
            return false;
        }
        let full_line = previous.right_gap <= upper.size * SHORT_LINE_RATIO;
        if lower.page == upper.page + 1 {
            return starts_lowercase(&lower.text) || (full_line && !ends_sentence(&upper.text));
        }
        if lower.page != upper.page {
            return false;
        }
        let pitch = upper.baseline - lower.baseline;
        let expected_pitch = self.profile.expected_pitch(upper.size.max(lower.size));
        let close_enough = pitch > 0.0 && pitch <= expected_pitch * MAX_PARAGRAPH_PITCHES;
        let mid_sentence = full_line && !ends_sentence(&upper.text) && starts_lowercase(&lower.text);
        let continues_sentence = mid_sentence && pitch > 0.0 && pitch <= expected_pitch * MAX_CONTINUATION_PITCHES;
        (close_enough && !(ends_sentence(&upper.text) && !full_line)) || continues_sentence
    }

    fn close(&mut self) {
        if let Some(open) = self.open.take() {
            let (block, heading) = self.build_block(open);
            if block.spans.iter().any(|span| !span.text.trim().is_empty()) {
                if let Some(heading) = heading {
                    self.headings.push((self.blocks.len(), heading));
                }
                self.blocks.push(block);
            }
        }
        self.flush_footnotes();
    }

    fn flush_footnotes(&mut self) {
        for footnote in std::mem::take(&mut self.pending_footnotes) {
            let mut text = BlockText::default();
            self.join_prose(&mut text, footnote.lines.iter(), false);
            self.blocks.push(CleanedBlock { kind: BlockKind::Note, level: None, page_index: footnote.page, spans: text.into_spans(true) });
        }
    }

    /// The block, plus the printed heading when it is one (titles and levels are set once all headings are known).
    fn build_block(&self, open: OpenBlock) -> (CleanedBlock, Option<RawHeading>) {
        let page_index = open.lines[0].line.page;
        let lines: Vec<Line> = open.lines.into_iter().map(|placed| placed.line).collect();
        let (kind, spans) = match open.kind {
            LineKind::Code => (BlockKind::Code, layout_spans(&lines, code_lines(&lines))),
            LineKind::TableRow => (BlockKind::Table, layout_spans(&lines, table_rows(&lines))),
            LineKind::Heading => (BlockKind::Heading, self.prose_spans(&lines, false, open.silent)),
            LineKind::Caption => (BlockKind::Caption, self.prose_spans(&lines, false, open.silent)),
            LineKind::ListItem => (BlockKind::ListItem, self.prose_spans(&lines, true, open.silent)),
            LineKind::Note | LineKind::NoteBox => (BlockKind::Note, self.prose_spans(&lines, false, open.silent)),
            LineKind::Body => (BlockKind::Paragraph, self.prose_spans(&lines, false, open.silent)),
        };
        let heading = (kind == BlockKind::Heading).then(|| RawHeading {
            text: spans.iter().map(|span| span.text.as_str()).collect::<String>().trim().to_string(),
            page: page_index,
            size: lines.iter().map(|line| line.size).fold(0.0, f32::max),
        });
        (CleanedBlock { kind, level: None, page_index, spans }, heading)
    }

    fn prose_spans(&self, lines: &[Line], strip_list_glyph: bool, silent: bool) -> Vec<TextSpan> {
        let mut text = BlockText::default();
        self.join_prose(&mut text, lines.iter(), strip_list_glyph);
        finish_spans(text, silent)
    }

    fn join_prose<'l>(&self, text: &mut BlockText, lines: impl Iterator<Item = &'l Line>, strip_list_glyph: bool) {
        let mut previous: Option<&Line> = None;
        for line in lines {
            let start = match previous {
                None if strip_list_glyph => list_glyph_length(&line.text),
                _ => 0,
            };
            if let Some(upper) = previous {
                match self.vocabulary.join(&upper.text, upper.line_end_hyphen, &line.text) {
                    LineJoin::Space if !text.ends_with_space() => text.push(PROSE_LINE_SEPARATOR, line.page),
                    LineJoin::RemoveHyphen => text.pop_char(),
                    LineJoin::Space | LineJoin::KeepHyphen => {}
                }
            }
            self.push_line(text, line, start);
            previous = Some(line);
        }
    }

    /// Pushes the line's text from byte `start`; digit superscripts become superscript chars and are
    /// not spoken (footnote markers, and the rare exponent, which a voice can't read from `10²²` anyway).
    fn push_line(&self, text: &mut BlockText, line: &Line, start: usize) {
        let mut cursor = start;
        for range in line.superscripts.iter().filter(|range| range.start >= start) {
            text.push(&line.text[cursor..range.start], line.page);
            let marker = &line.text[range.clone()];
            if marker.chars().all(|character| character.is_ascii_digit()) {
                text.push_silent(&to_superscript(marker), line.page);
            } else {
                text.push(marker, line.page);
            }
            cursor = range.end;
        }
        text.push(&line.text[cursor..], line.page);
    }
}

fn finish_spans(mut text: BlockText, silent: bool) -> Vec<TextSpan> {
    let ranges = silent_ranges(text.text());
    text.silence(ranges);
    text.into_spans(silent)
}

fn layout_spans(lines: &[Line], texts: Vec<String>) -> Vec<TextSpan> {
    let mut text = BlockText::default();
    for (index, (line, line_text)) in lines.iter().zip(texts).enumerate() {
        if index > 0 {
            text.push(CODE_LINE_SEPARATOR, line.page);
        }
        text.push(&line_text, line.page);
    }
    text.into_spans(false)
}

fn table_rows(lines: &[Line]) -> Vec<String> {
    let columns = table_columns(lines);
    lines.iter().map(|line| row_text(line, &columns)).collect()
}

/// The width of the line's first word, from the average char width of its first piece.
fn first_word_width(line: &Line) -> f32 {
    let Some(piece) = line.pieces.first() else { return 0.0 };
    let chars = piece.text.chars().count().max(1) as f32;
    let word_chars = piece.text.split_whitespace().next().map_or(0, |word| word.chars().count()) as f32;
    (piece.right - piece.left) / chars * word_chars
}

fn to_superscript(digits: &str) -> String {
    digits.chars().filter_map(|digit| digit.to_digit(10)).map(|digit| SUPERSCRIPT_DIGITS[digit as usize]).collect()
}
