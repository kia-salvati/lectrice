//! Rule 3: build lines from runs. Runs on one baseline join, and so do raised smaller runs
//! (superscripts), even when PDFium put a line break between them.

use std::ops::Range;

use super::fonts::FontStyle;
use super::junk::strip_inline_junk;
use super::normalize::normalize_text;
use crate::pdf::TextRun;

/// Runs whose baselines differ by less than this share of the line's size are on the same line.
const SAME_BASELINE_RATIO: f32 = 0.3;
/// A superscript is at most this share of the line's size…
const SUPERSCRIPT_MAX_SIZE_RATIO: f32 = 0.85;
/// …and its baseline is raised by at least this share (and at most `SUPERSCRIPT_MAX_RAISE_RATIO`).
const SUPERSCRIPT_MIN_RAISE_RATIO: f32 = 0.2;
const SUPERSCRIPT_MAX_RAISE_RATIO: f32 = 0.8;
/// How far a run may start left of the previous run's right edge and still continue the line.
const BACKTRACK_RATIO: f32 = 0.5;
/// A gap wider than this share of the size gets a space when no space char is there.
const WORD_GAP_RATIO: f32 = 0.1;
/// A gap wider than this share of the size separates table cells (GoF's justified lines have word gaps above 1.2).
const CELL_GAP_RATIO: f32 = 2.0;
/// GoF marks sub-items with a lone `o` instead of a bullet glyph.
const LETTER_BULLET: &str = "o";

/// What a later rule decided about a line. Only `Content` lines become blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRole {
    Content,
    RunningHeader,
    Footnote,
    Junk,
}

/// One run inside a line, after Unicode normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub text: String,
    pub left: f32,
    pub right: f32,
    pub size: f32,
    pub baseline: f32,
    pub font: String,
    pub superscript: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub page: u32,
    pub pieces: Vec<Piece>,
    /// The shown text; superscripts are included as they are and listed in `superscripts`.
    pub text: String,
    pub superscripts: Vec<Range<usize>>,
    /// Left edges of the cells (pieces separated by a wide gap); one entry for a normal line.
    pub cell_lefts: Vec<f32>,
    pub left: f32,
    pub right: f32,
    pub baseline: f32,
    pub size: f32,
    pub font: String,
    pub style: FontStyle,
    pub all_bold: bool,
    pub line_end_hyphen: bool,
    pub role: LineRole,
}

impl Line {
    pub fn char_count(&self) -> usize {
        self.text.chars().filter(|character| !character.is_whitespace()).count()
    }

    pub fn is_content(&self) -> bool {
        self.role == LineRole::Content
    }

    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    pub fn has_cells(&self) -> bool {
        self.cell_lefts.len() > 1
    }
}

/// Builds the lines of one page from its runs, in PDFium order.
pub fn build_lines(page: u32, runs: &[&TextRun]) -> Vec<Line> {
    let mut groups: Vec<Vec<&TextRun>> = Vec::new();
    for run in runs {
        match groups.last_mut() {
            Some(group) if continues_line(group, run) => group.push(run),
            _ => groups.push(vec![run]),
        }
    }
    let groups = merge_split_lines(groups);
    groups.into_iter().filter_map(|group| make_line(page, &group)).collect()
}

fn anchor<'a>(group: &[&'a TextRun]) -> &'a TextRun {
    group.iter().copied().max_by(|left, right| left.font_size.total_cmp(&right.font_size)).expect("a line group is never empty")
}

fn continues_line(group: &[&TextRun], run: &TextRun) -> bool {
    let anchor = anchor(group);
    let previous = group.last().expect("a line group is never empty");
    let raise = run.baseline - anchor.baseline;
    let same_baseline = raise.abs() <= anchor.font_size * SAME_BASELINE_RATIO;
    let raised_smaller = is_raised_smaller(run.font_size, raise, anchor.font_size);
    let anchor_was_raised = is_raised_smaller(anchor.font_size, -raise, run.font_size);
    let moves_right = run.bbox.left >= previous.bbox.right - run.font_size.max(previous.font_size) * BACKTRACK_RATIO;
    // A drop letter or overprinted run may start left of the previous right edge but still extends the line.
    let extends_line = same_baseline && run.bbox.right > previous.bbox.right;
    let moves_right = moves_right || extends_line;
    (same_baseline || raised_smaller || anchor_was_raised) && moves_right
}

fn is_raised_smaller(size: f32, raise: f32, line_size: f32) -> bool {
    size <= line_size * SUPERSCRIPT_MAX_SIZE_RATIO && raise >= line_size * SUPERSCRIPT_MIN_RAISE_RATIO && raise <= line_size * SUPERSCRIPT_MAX_RAISE_RATIO
}

/// PDFium sometimes splits one visual line into consecutive groups; join them back.
fn merge_split_lines(groups: Vec<Vec<&TextRun>>) -> Vec<Vec<&TextRun>> {
    let mut merged: Vec<Vec<&TextRun>> = Vec::new();
    for group in groups {
        match merged.last_mut() {
            Some(previous) if continues_line(previous, group[0]) => previous.extend(group),
            _ => merged.push(group),
        }
    }
    merged
}

fn make_line(page: u32, group: &[&TextRun]) -> Option<Line> {
    let anchor = anchor(group);
    let mut pieces: Vec<Piece> = group
        .iter()
        .map(|run| Piece {
            text: strip_inline_junk(&normalize_text(&run.text)).into_owned(),
            left: run.bbox.left,
            right: run.bbox.right,
            size: run.font_size,
            baseline: run.baseline,
            font: run.font_name.clone(),
            superscript: is_raised_smaller(run.font_size, run.baseline - anchor.baseline, anchor.font_size),
        })
        .collect();
    pieces.retain(|piece| !piece.text.trim().is_empty());
    if pieces.is_empty() {
        return None;
    }
    let (size, font) = dominant_size_and_font(&pieces);
    let JoinedPieces { text, superscripts, cell_lefts } = join_pieces(&pieces);
    let style = FontStyle::parse(&font);
    let all_bold = pieces.iter().filter(|piece| !piece.superscript).all(|piece| FontStyle::parse(&piece.font).bold);
    let baseline = pieces.iter().find(|piece| !piece.superscript).map_or(anchor.baseline, |piece| piece.baseline);
    Some(Line {
        page,
        left: pieces.iter().map(|piece| piece.left).fold(f32::INFINITY, f32::min),
        right: pieces.iter().map(|piece| piece.right).fold(f32::NEG_INFINITY, f32::max),
        baseline,
        size,
        font,
        style,
        all_bold,
        line_end_hyphen: group.last().is_some_and(|run| run.line_end_hyphen),
        text,
        superscripts,
        cell_lefts,
        pieces,
        role: LineRole::Content,
    })
}

/// The size and font carrying the most chars (superscripts don't count).
fn dominant_size_and_font(pieces: &[Piece]) -> (f32, String) {
    let mut weights: Vec<(f32, &str, usize)> = Vec::new();
    for piece in pieces.iter().filter(|piece| !piece.superscript) {
        let chars = piece.text.chars().count();
        match weights.iter_mut().find(|(size, font, _)| (*size - piece.size).abs() < f32::EPSILON && *font == piece.font) {
            Some(entry) => entry.2 += chars,
            None => weights.push((piece.size, &piece.font, chars)),
        }
    }
    weights.iter().max_by_key(|(_, _, chars)| *chars).map_or_else(|| (pieces[0].size, pieces[0].font.clone()), |(size, font, _)| (*size, (*font).to_string()))
}

struct JoinedPieces {
    text: String,
    superscripts: Vec<Range<usize>>,
    cell_lefts: Vec<f32>,
}

fn join_pieces(pieces: &[Piece]) -> JoinedPieces {
    let mut text = String::new();
    let mut superscripts = Vec::new();
    let mut cell_lefts = vec![pieces[0].left];
    for (index, piece) in pieces.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).map(|previous| &pieces[previous]) {
            let gap = piece.left - previous.right;
            let size = piece.size.max(previous.size);
            let needs_space = !piece.superscript && gap > size * WORD_GAP_RATIO && !text.ends_with(' ') && !piece.text.starts_with(' ');
            if needs_space {
                text.push(' ');
            }
            // A gap after a bullet or a list number is a hanging indent, not a cell break.
            let after_words = text.chars().any(char::is_alphabetic) && text.trim() != LETTER_BULLET;
            if gap > size * CELL_GAP_RATIO && after_words && !piece.superscript && !previous.superscript {
                cell_lefts.push(piece.left);
            }
        }
        let start = text.len();
        text.push_str(if piece.superscript { piece.text.trim() } else { &piece.text });
        if piece.superscript {
            superscripts.push(start..text.len());
        }
    }
    let trimmed_end = text.trim_end().len();
    text.truncate(trimmed_end);
    JoinedPieces { text, superscripts, cell_lefts }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::pdf::BoundingBox;

    pub fn run(text: &str, left: f32, baseline: f32, size: f32, font: &str) -> TextRun {
        let width = text.chars().count() as f32 * size * 0.5;
        TextRun {
            pdf_page: 1,
            text: text.into(),
            font_name: font.into(),
            font_size: size,
            bbox: BoundingBox { left, bottom: baseline - size * 0.25, right: left + width, top: baseline + size * 0.8 },
            baseline,
            line_end_hyphen: false,
        }
    }

    /// A profile whose body is 10 pt `Body` at a 12 pt pitch, like the lines `run` makes.
    pub fn profile_of_body() -> crate::cleaning::profile::Profile {
        use crate::cleaning::page::{Page, PageRole};
        let lines = lines_of(&[run("body text", 54.0, 700.0, 10.0, "Body"), run("body text", 54.0, 688.0, 10.0, "Body")]);
        let page = Page { index: 1, width: 612.0, height: 792.0, embedded_label: None, lines, role: PageRole::Body };
        crate::cleaning::profile::Profile::from_pages(&[&page])
    }

    pub fn line_with_text(text: &str) -> Line {
        lines_of(&[run(text, 54.0, 500.0, 10.0, "Body")]).remove(0)
    }

    pub fn lines_of(runs: &[TextRun]) -> Vec<Line> {
        let references: Vec<&TextRun> = runs.iter().collect();
        build_lines(1, &references)
    }

    #[test]
    fn a_raised_smaller_run_joins_the_line_as_a_superscript() {
        let lines =
            lines_of(&[run("Foundation (CNCF)", 54.0, 273.3, 10.0, "Body"), run("1", 139.0, 276.9, 6.97, "Body"), run(".", 143.0, 273.3, 10.0, "Body")]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Foundation (CNCF)1.");
        assert_eq!(&lines[0].text[lines[0].superscripts[0].clone()], "1");
    }

    #[test]
    fn a_run_further_down_or_back_to_the_left_starts_a_new_line() {
        let lines =
            lines_of(&[run("first line", 54.0, 500.0, 10.0, "Body"), run("second", 54.0, 488.0, 10.0, "Body"), run("same", 200.0, 488.0, 10.0, "Body")]);
        let texts: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["first line", "second same"]);
    }

    #[test]
    fn wide_gaps_make_cells_and_words_get_spaces() {
        let lines = lines_of(&[run("create", 99.0, 420.0, 9.0, "Body"), run("POST", 200.0, 420.0, 9.0, "Body"), run("201", 225.0, 420.0, 9.0, "Body")]);
        assert_eq!(lines[0].text, "create POST 201");
        assert_eq!(lines[0].cell_lefts, [99.0, 200.0]);
    }

    #[test]
    fn dominant_font_and_bold_flags_come_from_the_chars() {
        let lines = lines_of(&[
            run("Use the ", 54.0, 400.0, 10.0, "Crimson-Roman"),
            run("kubectl", 90.0, 400.0, 10.0, "SourceCodePro-Bold"),
            run(" command now", 125.0, 400.0, 10.0, "Crimson-Roman"),
        ]);
        assert_eq!(lines[0].font, "Crimson-Roman");
        assert!(!lines[0].all_bold);
    }
}
