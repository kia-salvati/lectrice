//! Rule 9 (view part): code keeps its layout. PDFs drop leading spaces, so the indent and the
//! gaps between words are rebuilt from x positions and the monospace char width.

use super::lines::Line;

const MIN_CHARS_FOR_WIDTH: usize = 3;
/// Fallback when no piece is long enough to measure: a typical monospace advance.
const DEFAULT_CHAR_WIDTH_RATIO: f32 = 0.6;
/// A gap narrower than this share of a char width is no space at all.
const MIN_SPACE_SHARE: f32 = 0.5;
/// Broken PDFs report zero or near-zero font sizes; a smaller width would turn every gap into
/// millions of spaces.
const MIN_CHAR_WIDTH: f32 = 1.0;
/// The most spaces one indent or one gap between words can add.
const MAX_SPACES: usize = 80;

/// Each code line's text with its rebuilt indent, in order.
pub fn code_lines(lines: &[Line]) -> Vec<String> {
    let char_width = char_width(lines);
    lines
        .iter()
        .map(|line| {
            let base = lines.iter().filter(|other| other.page == line.page).map(|other| other.left).fold(f32::INFINITY, f32::min);
            format!("{}{}", " ".repeat(space_count(line.left - base, char_width)), line_text(line, char_width))
        })
        .collect()
}

fn line_text(line: &Line, char_width: f32) -> String {
    let mut text = String::new();
    for (index, piece) in line.pieces.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).map(|previous| &line.pieces[previous]) {
            let gap = piece.left - previous.right;
            let has_space = text.ends_with(' ') || piece.text.starts_with(' ');
            if !has_space && gap > char_width * MIN_SPACE_SHARE {
                text.push_str(&" ".repeat(space_count(gap, char_width).max(1)));
            }
        }
        text.push_str(&piece.text);
    }
    text.trim_end().to_string()
}

/// How many spaces fill `width` points, at most `MAX_SPACES` (a NaN width gives none).
fn space_count(width: f32, char_width: f32) -> usize {
    (width / char_width).round().clamp(0.0, MAX_SPACES as f32) as usize
}

/// The median monospace advance, never below `MIN_CHAR_WIDTH`.
fn char_width(lines: &[Line]) -> f32 {
    let mut widths: Vec<f32> = lines
        .iter()
        .flat_map(|line| line.pieces.iter())
        .filter_map(|piece| {
            let chars = piece.text.chars().count();
            (chars >= MIN_CHARS_FOR_WIDTH).then(|| (piece.right - piece.left) / chars as f32)
        })
        .filter(|width| *width > 0.0)
        .collect();
    let measured = if widths.is_empty() {
        lines.first().map_or(MIN_CHAR_WIDTH, |line| line.size * DEFAULT_CHAR_WIDTH_RATIO)
    } else {
        widths.sort_by(f32::total_cmp);
        widths[widths.len() / 2]
    };
    // `f32::max` returns the other value for NaN, so a NaN size also ends up at the minimum.
    measured.max(MIN_CHAR_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaning::lines::tests::{lines_of, run};

    #[test]
    fn indent_is_rebuilt_from_x() {
        let lines =
            lines_of(&[run("metadata:", 54.0, 500.0, 7.0, "Mono"), run("name: demo", 61.0, 490.0, 7.0, "Mono"), run("spec:", 54.0, 480.0, 7.0, "Mono")]);
        assert_eq!(code_lines(&lines), ["metadata:", "  name: demo", "spec:"]);
    }

    /// Review L1: a zero font size made the width 0, so a gap asked for `usize::MAX` spaces.
    #[test]
    fn zero_and_tiny_font_sizes_add_a_bounded_number_of_spaces() {
        for size in [0.0, 1e-30, f32::MIN_POSITIVE] {
            let lines = lines_of(&[run("a", 54.0, 500.0, size, "Mono"), run("b", 300.0, 500.0, size, "Mono"), run("c", 900.0, 490.0, size, "Mono")]);
            for text in code_lines(&lines) {
                assert!(text.len() <= 2 * MAX_SPACES + 3, "size {size}: {} chars", text.len());
            }
        }
    }

    #[test]
    fn nan_and_infinite_gaps_add_a_bounded_number_of_spaces() {
        assert_eq!(space_count(f32::NAN, 1.0), 0);
        assert_eq!(space_count(f32::INFINITY, 1.0), MAX_SPACES);
    }
}
