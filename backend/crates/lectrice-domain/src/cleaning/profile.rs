//! Rule 1: the document profile. Every size rule compares to the body size of this document,
//! because PDF font sizes are only meaningful inside one document.

use std::collections::{HashMap, HashSet};

use super::fonts::FontStyle;
use super::lines::{Line, Piece};
use super::page::Page;

/// Sizes and pitches are bucketed to this precision before counting.
const BUCKET: f32 = 10.0;
const SAME_SIZE_TOLERANCE_RATIO: f32 = 0.02;
const MIN_PITCH_RATIO: f32 = 0.9;
const MAX_PITCH_RATIO: f32 = 2.5;
const DEFAULT_PITCH_RATIO: f32 = 1.2;
const DEFAULT_BODY_SIZE: f32 = 10.0;
/// A monospace style is inline code when more than this share of its chars sits on lines that also hold prose.
const MIN_INLINE_CODE_SHARE: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub body_size: f32,
    pub body_font: String,
    pub body_style: FontStyle,
    /// Body baseline-to-baseline distance divided by the body size.
    pub pitch_ratio: f32,
    /// Monospace styles (family, bucketed size) used mostly inside prose lines (K8s `kubectl` at 0.9× body).
    pub inline_code_styles: HashSet<(String, i32)>,
}

impl Profile {
    /// Measured on the content lines of the kept pages (a dense table of contents would skew it).
    pub fn from_pages(pages: &[&Page]) -> Self {
        let lines = || pages.iter().flat_map(|page| page.content_lines());
        let (body_size, body_font) = most_common_size_and_font(lines()).unwrap_or_else(|| (DEFAULT_BODY_SIZE, String::new()));
        let pitch_ratio = most_common_pitch(pages, body_size).map_or(DEFAULT_PITCH_RATIO, |pitch| pitch / body_size);
        let body_style = FontStyle::parse(&body_font);
        let inline_code_styles = inline_code_styles(lines(), &body_style);
        Self { body_style, body_font, body_size, pitch_ratio, inline_code_styles }
    }

    /// Whether the line's own font and size are the document's inline-code style, not a code-block style.
    pub fn is_inline_code_style(&self, line: &Line) -> bool {
        self.inline_code_styles.contains(&(line.style.family.clone(), bucket(line.size)))
    }

    /// The `mono_body` case of the survey: the body itself is set in a monospace font (GoF).
    pub fn monospace_body(&self) -> bool {
        self.body_style.monospace
    }

    pub fn ratio(&self, size: f32) -> f32 {
        size / self.body_size
    }

    /// The baseline distance expected between two lines of this size inside one paragraph.
    pub fn expected_pitch(&self, size: f32) -> f32 {
        size * self.pitch_ratio
    }

    pub fn is_body_size(&self, size: f32) -> bool {
        (size - self.body_size).abs() <= self.body_size * SAME_SIZE_TOLERANCE_RATIO
    }
}

/// A piece of prose: not monospace and not a superscript (also the `.` after inline code).
pub fn is_prose_piece(piece: &Piece) -> bool {
    !piece.superscript && !FontStyle::parse(&piece.font).monospace && !piece.text.trim().is_empty()
}

fn inline_code_styles<'a>(lines: impl Iterator<Item = &'a Line>, body_style: &FontStyle) -> HashSet<(String, i32)> {
    let mut chars: HashMap<(String, i32), (usize, usize)> = HashMap::new();
    for line in lines {
        let mixed = line.pieces.iter().any(is_prose_piece);
        for piece in line.pieces.iter().filter(|piece| !piece.superscript) {
            let style = FontStyle::parse(&piece.font);
            if !style.monospace || style.family == body_style.family {
                continue;
            }
            let count = piece.text.chars().filter(|character| !character.is_whitespace()).count();
            let entry = chars.entry((style.family, bucket(piece.size))).or_default();
            entry.0 += count;
            if mixed {
                entry.1 += count;
            }
        }
    }
    chars.into_iter().filter(|(_, (total, mixed))| *mixed as f32 > *total as f32 * MIN_INLINE_CODE_SHARE).map(|(style, _)| style).collect()
}

fn bucket(value: f32) -> i32 {
    (value * BUCKET).round() as i32
}

fn most_common_size_and_font<'a>(lines: impl Iterator<Item = &'a Line>) -> Option<(f32, String)> {
    let mut weights: HashMap<(i32, &str), usize> = HashMap::new();
    for line in lines {
        *weights.entry((bucket(line.size), line.font.as_str())).or_default() += line.char_count();
    }
    let ((size, font), _) = weights.into_iter().max_by_key(|((size, font), weight)| (*weight, *size, *font))?;
    Some((size as f32 / BUCKET, font.to_string()))
}

fn most_common_pitch(pages: &[&Page], body_size: f32) -> Option<f32> {
    let mut counts: HashMap<i32, usize> = HashMap::new();
    for page in pages {
        let lines: Vec<&Line> = page.content_lines().collect();
        for pair in lines.windows(2) {
            let (upper, lower) = (pair[0], pair[1]);
            let both_body = [upper, lower].iter().all(|line| (line.size - body_size).abs() <= body_size * SAME_SIZE_TOLERANCE_RATIO);
            let pitch = upper.baseline - lower.baseline;
            if both_body && pitch >= body_size * MIN_PITCH_RATIO && pitch <= body_size * MAX_PITCH_RATIO {
                *counts.entry(bucket(pitch)).or_default() += 1;
            }
        }
    }
    counts.into_iter().max_by_key(|(pitch, count)| (*count, *pitch)).map(|(pitch, _)| pitch as f32 / BUCKET)
}
