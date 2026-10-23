//! Rules 9, 11, 12, 14 and 15 at line level: what kind of block a line starts or belongs to.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::assemble::MAX_PARAGRAPH_PITCHES;
use super::lines::Line;
use super::profile::{Profile, is_prose_piece};

/// Code is set in a monospace font at most this share of the body size (inline code is larger).
const MAX_CODE_SIZE_RATIO: f32 = 0.92;
/// Headings are at least this share of the body size…
const MIN_HEADING_SIZE_RATIO: f32 = 1.15;
/// …or bold lines about the body size (GoF's `Intent`).
const MIN_BOLD_HEADING_SIZE_RATIO: f32 = 0.95;
const MAX_HEADING_WORDS: usize = 15;
const MAX_BOLD_HEADING_WORDS: usize = 10;
/// Note boxes (DDS) are indented at least this far from the page's text margin, with text a bit smaller than the body.
const MIN_NOTE_BOX_INDENT: f32 = 40.0;
const MIN_NOTE_BOX_SIZE_RATIO: f32 = 0.85;
const MAX_NOTE_BOX_SIZE_RATIO: f32 = 0.97;
/// A caption in the body style is a short line that leaves at least this share of the column empty.
const MAX_SHORT_CAPTION_WORDS: usize = 12;
const MIN_SHORT_CAPTION_SLACK: f32 = 0.15;

static LIST_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([•·●▪◦■□➤✔✓►▶◆❖🔹🔸🔷🔶➢➔→]\s*|[-–o]\s+|\d{1,2}[.)]\s+|\d{1,2}\s[-–]\s)").expect("valid regex"));
static NOTE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(Note|Tip|Warning|Important|Caution)[:.]\s").expect("valid regex"));
static CAPTION_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(Figure|Fig\.|Table|Listing|Example|Exhibit)\s+\d+([.\-–]\d+)*(?<sep>[.:])?(\s|$)").expect("valid regex"));
static NUMBERED_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,2}([.)]\s+|\s[-–]\s)").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Code,
    TableRow,
    Heading,
    Caption,
    ListItem,
    Note,
    NoteBox,
    Body,
}

/// Where a page's text column starts and ends (the most common left and the widest right of body lines).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    pub left: f32,
    pub right: f32,
}

impl Column {
    pub fn of<'a>(lines: impl Iterator<Item = &'a Line>, profile: &Profile) -> Self {
        let body: Vec<&Line> = lines.filter(|line| profile.is_body_size(line.size)).collect();
        let mut lefts: HashMap<i32, usize> = HashMap::new();
        for line in &body {
            *lefts.entry(line.left.round() as i32).or_default() += 1;
        }
        let left = lefts.into_iter().max_by_key(|(left, count)| (*count, -left)).map_or(0.0, |(left, _)| left as f32);
        let right = body.iter().map(|line| line.right).fold(left, f32::max);
        Self { left, right }
    }
}

/// A code line is set in a code font throughout: a line that also holds prose (a wrapped `8080.` or
/// `the Sender interface to the Viewer.`) stays prose.
pub fn is_code_line(line: &Line, profile: &Profile) -> bool {
    let monospace_code_font = line.style.monospace && (line.style.family != profile.body_style.family || profile.monospace_body());
    let code_size = profile.ratio(line.size) <= MAX_CODE_SIZE_RATIO;
    monospace_code_font && code_size && !line.pieces.iter().any(is_prose_piece)
}

/// A lone code line in the document's inline-code style is inline code wrapped onto its own line
/// (`BusinessRules.`); two or more such lines in a row are a short listing (CA sets some in that style).
pub fn demote_lone_inline_code(lines: &[&Line], kinds: &mut [LineKind], profile: &Profile) {
    let inline_code = |index: usize, kinds: &[LineKind]| kinds[index] == LineKind::Code && profile.is_inline_code_style(lines[index]);
    for index in 0..lines.len() {
        let same_style = |other: usize| (lines[other].size - lines[index].size).abs() < f32::EPSILON && lines[other].style.family == lines[index].style.family;
        let listing_neighbour = [index.checked_sub(1), Some(index + 1).filter(|next| *next < lines.len())]
            .into_iter()
            .flatten()
            .any(|other| inline_code(other, kinds) && same_style(other));
        if inline_code(index, kinds) && !listing_neighbour {
            kinds[index] = LineKind::Body;
        }
    }
}

/// The kind of one line by itself (table rows need their neighbours, see `tables`).
pub fn classify_line(line: &Line, profile: &Profile, column: Column) -> LineKind {
    let text = line.text.trim();
    if is_code_line(line, profile) {
        return LineKind::Code;
    }
    if is_caption_start(line, profile, column) {
        return LineKind::Caption;
    }
    if LIST_START.is_match(text) && !(NUMBERED_ITEM.is_match(text) && profile.ratio(line.size) >= MIN_HEADING_SIZE_RATIO) {
        return LineKind::ListItem;
    }
    if is_heading(line, profile) {
        return LineKind::Heading;
    }
    if NOTE_START.is_match(text) {
        return LineKind::Note;
    }
    let ratio = profile.ratio(line.size);
    if line.left >= column.left + MIN_NOTE_BOX_INDENT && (MIN_NOTE_BOX_SIZE_RATIO..MAX_NOTE_BOX_SIZE_RATIO).contains(&ratio) {
        return LineKind::NoteBox;
    }
    LineKind::Body
}

/// A line that goes on with the sentence of the line above it, at the paragraph's pitch, is part of that
/// paragraph even when it looks like a heading (a bold `svc-shield` wrapped onto its own line) or a caption
/// (a line that starts with `Figure 1.2. That …`). Headings set larger than the body are left alone.
pub fn demote_continuations(lines: &[&Line], kinds: &mut [LineKind], profile: &Profile) {
    for index in 1..lines.len() {
        let (upper, lower) = (lines[index - 1], lines[index]);
        let lookalike = match kinds[index] {
            LineKind::Heading => profile.ratio(lower.size) < MIN_HEADING_SIZE_RATIO,
            LineKind::Caption => true,
            _ => false,
        };
        let after_prose = matches!(kinds[index - 1], LineKind::Body | LineKind::ListItem | LineKind::Note | LineKind::NoteBox);
        let pitch = upper.baseline - lower.baseline;
        let at_paragraph_pitch = pitch > 0.0 && pitch <= profile.expected_pitch(upper.size.max(lower.size)) * MAX_PARAGRAPH_PITCHES;
        if lookalike && after_prose && at_paragraph_pitch && profile.is_body_size(upper.size) && !ends_sentence(&upper.text) {
            kinds[index] = LineKind::Body;
        }
    }
}

pub fn ends_sentence(text: &str) -> bool {
    let trimmed = text.trim_end().trim_end_matches(['”', '"', '’', ')', ']']);
    trimmed.ends_with(['.', '?', '!', ':'])
}

pub fn starts_lowercase(text: &str) -> bool {
    text.trim_start().chars().next().is_some_and(char::is_lowercase)
}

/// `Figure 2-1. …` in any style, or a short line in the body style (GoF sets captions in its body font).
/// A sentence about the figure (`Figure 8.2 shows the …`) goes on in lowercase and is not a caption.
fn is_caption_start(line: &Line, profile: &Profile, column: Column) -> bool {
    let text = line.text.trim();
    let Some(captures) = CAPTION_START.captures(text) else { return false };
    if text[captures.get(0).map_or(0, |found| found.end())..].chars().next().is_some_and(char::is_lowercase) {
        return false;
    }
    let styled_apart = line.font != profile.body_font || !profile.is_body_size(line.size);
    let short_line = line.word_count() <= MAX_SHORT_CAPTION_WORDS && column.right - line.right > (column.right - column.left) * MIN_SHORT_CAPTION_SLACK;
    styled_apart || captures.name("sep").is_some() || short_line
}

fn is_heading(line: &Line, profile: &Profile) -> bool {
    let text = line.text.trim();
    let has_letter_or_number = text.chars().any(char::is_alphanumeric);
    let ends_like_prose = text.ends_with([',', ';']);
    if !has_letter_or_number || ends_like_prose || line.has_cells() {
        return false;
    }
    let ratio = profile.ratio(line.size);
    if ratio >= MIN_HEADING_SIZE_RATIO {
        return line.word_count() <= MAX_HEADING_WORDS;
    }
    let bold_heading = line.all_bold && !profile.body_style.bold && ratio >= MIN_BOLD_HEADING_SIZE_RATIO;
    bold_heading && line.word_count() <= MAX_BOLD_HEADING_WORDS && !text.ends_with(['.', ':'])
}

/// The list glyph or number prefix to drop from the shown text (numbers stay: `1. ` is kept).
pub fn list_glyph_length(text: &str) -> usize {
    match LIST_START.find(text) {
        Some(found) if !NUMBERED_ITEM.is_match(text) => found.end(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_shapes() {
        assert!(CAPTION_START.is_match("Figure 2-1. A sample helper layout"));
        assert!(CAPTION_START.is_match("Figure 1.2"));
        assert!(CAPTION_START.is_match("Table 1.1: Sample value space"));
        assert!(!CAPTION_START.is_match("Figures show"));
        assert!(CAPTION_START.captures("Table 1.1: Design").unwrap().name("sep").is_some());
        assert!(CAPTION_START.captures("Figure 2 shows the flow").unwrap().name("sep").is_none());
    }

    #[test]
    fn a_sentence_about_a_figure_is_not_a_caption() {
        let profile = super::super::lines::tests::profile_of_body();
        let column = Column { left: 54.0, right: 450.0 };
        let caption = super::super::lines::tests::line_with_text("Figure 8.2 Path-based routing");
        let sentence = super::super::lines::tests::line_with_text("Figure 8.2 shows the whole setup.");
        assert!(is_caption_start(&caption, &profile, column));
        assert!(!is_caption_start(&sentence, &profile, column));
    }

    #[test]
    fn list_glyphs_are_stripped_but_numbers_stay() {
        assert_eq!(&"• Web front-end"[list_glyph_length("• Web front-end")..], "Web front-end");
        assert_eq!(&"🔹 What is Sensitive Data?"[list_glyph_length("🔹 What is Sensitive Data?")..], "What is Sensitive Data?");
        assert_eq!(list_glyph_length("1. Urgent and important"), 0);
        assert_eq!(list_glyph_length("2 - Caching"), 0);
        assert!(LIST_START.is_match("2 - Caching"));
        assert!(NOTE_START.is_match("Note: You can"));
    }
}
