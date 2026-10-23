use lectrice_domain::{BoundingBox, TextRun};

/// PDFium reports a line-end hyphen as this control char (and removes the line break after it).
const PDFIUM_HYPHEN_MARKER: u32 = 0x02;
const SIZE_TOLERANCE: f32 = 0.05;
/// Baselines closer than this share a run.
const BASELINE_TOLERANCE: f32 = 0.5;
/// A gap wider than this share of the font size starts a new run, so the cleaner sees word gaps
/// that have no space char and the gaps between table cells.
const RUN_GAP_RATIO: f32 = 0.25;
/// After a space a wider gap is normal, but one wider than this share of the font size separates
/// table cells or columns (PDFium puts a single space there).
const CELL_GAP_RATIO: f32 = 1.2;
/// A char may start slightly left of the previous char's right edge (kerning).
const OVERLAP_TOLERANCE_RATIO: f32 = 0.3;

const HIGH_SURROGATES: std::ops::RangeInclusive<u32> = 0xD800..=0xDBFF;
const LOW_SURROGATES: std::ops::RangeInclusive<u32> = 0xDC00..=0xDFFF;
const SUPPLEMENTARY_BASE: u32 = 0x10000;
const SURROGATE_BITS: u32 = 10;

/// What the run builder needs to know about one PDFium char.
#[derive(Debug, Clone, PartialEq)]
pub struct CharInfo {
    pub unicode: u32,
    pub generated: bool,
    pub hyphen: bool,
    pub font_name: String,
    pub font_size: f32,
    pub bounds: BoundingBox,
    pub baseline: f32,
}

/// Groups the chars of one page into [`TextRun`]s: same font, size and baseline, no visible gap.
pub struct RunBuilder {
    pdf_page: u32,
    runs: Vec<TextRun>,
    current: Option<TextRun>,
    /// PDFium reports a char outside the BMP (LinkedIn's `𝐦`, emoji) as two UTF-16 surrogate chars.
    high_surrogate: Option<u32>,
}

impl RunBuilder {
    pub fn new(pdf_page: u32) -> Self {
        Self { pdf_page, runs: Vec::new(), current: None, high_surrogate: None }
    }

    pub fn push(&mut self, mut char_info: CharInfo) {
        let unicode = char_info.unicode;
        if HIGH_SURROGATES.contains(&unicode) {
            self.high_surrogate = Some(unicode);
            return;
        }
        if let Some(high) = self.high_surrogate.take()
            && LOW_SURROGATES.contains(&unicode)
        {
            char_info.unicode = SUPPLEMENTARY_BASE + ((high - HIGH_SURROGATES.start()) << SURROGATE_BITS) + (unicode - LOW_SURROGATES.start());
        }
        let Some(character) = char::from_u32(char_info.unicode) else { return };
        if character == '\r' || character == '\n' {
            self.close();
            return;
        }
        if character.is_whitespace() {
            self.push_space();
            return;
        }
        if char_info.unicode == PDFIUM_HYPHEN_MARKER || char_info.hyphen {
            self.push_line_end_hyphen(&char_info);
            return;
        }
        if character.is_control() || char_info.generated {
            return;
        }
        self.push_visible(character, char_info);
    }

    pub fn finish(mut self) -> Vec<TextRun> {
        self.close();
        self.runs
    }

    fn push_space(&mut self) {
        if let Some(run) = self.current.as_mut()
            && !run.text.ends_with(' ')
        {
            run.text.push(' ');
        }
    }

    fn push_line_end_hyphen(&mut self, char_info: &CharInfo) {
        if self.current.is_none() {
            self.start('-', char_info.clone());
        } else if let Some(run) = self.current.as_mut() {
            run.text.push('-');
            run.bbox.right = run.bbox.right.max(char_info.bounds.right);
        }
        if let Some(run) = self.current.as_mut() {
            run.line_end_hyphen = true;
        }
        self.close();
    }

    fn push_visible(&mut self, character: char, char_info: CharInfo) {
        let continues = self.current.as_ref().is_some_and(|run| continues_run(run, &char_info));
        if !continues {
            self.close();
            self.start(character, char_info);
            return;
        }
        if let Some(run) = self.current.as_mut() {
            run.text.push(character);
            run.bbox.right = run.bbox.right.max(char_info.bounds.right);
            run.bbox.bottom = run.bbox.bottom.min(char_info.bounds.bottom);
            run.bbox.top = run.bbox.top.max(char_info.bounds.top);
        }
    }

    fn start(&mut self, character: char, char_info: CharInfo) {
        self.current = Some(TextRun {
            pdf_page: self.pdf_page,
            text: character.to_string(),
            font_name: char_info.font_name,
            font_size: char_info.font_size,
            bbox: char_info.bounds,
            baseline: char_info.baseline,
            line_end_hyphen: false,
        });
    }

    fn close(&mut self) {
        if let Some(mut run) = self.current.take() {
            let trimmed_length = run.text.trim_end().len();
            run.text.truncate(trimmed_length);
            if !run.text.is_empty() {
                self.runs.push(run);
            }
        }
    }
}

fn continues_run(run: &TextRun, char_info: &CharInfo) -> bool {
    let same_style = run.font_name == char_info.font_name && (run.font_size - char_info.font_size).abs() < SIZE_TOLERANCE;
    let same_baseline = (run.baseline - char_info.baseline).abs() < BASELINE_TOLERANCE;
    let gap = char_info.bounds.left - run.bbox.right;
    let gap_ratio = if run.text.ends_with(' ') { CELL_GAP_RATIO } else { RUN_GAP_RATIO };
    let gap_allowed = run.font_size * gap_ratio;
    let adjacent = gap >= -run.font_size * OVERLAP_TOLERANCE_RATIO && gap <= gap_allowed;
    same_style && same_baseline && adjacent
}

/// Subset fonts are named like `DZPUDC+CrimsonText-Roman`; the prefix carries no meaning.
pub fn strip_subset_prefix(font_name: &str) -> &str {
    match font_name.split_once('+') {
        Some((prefix, rest)) if prefix.len() == 6 && prefix.bytes().all(|byte| byte.is_ascii_uppercase()) => rest,
        _ => font_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_at(character: char, left: f32, baseline: f32) -> CharInfo {
        CharInfo {
            unicode: character as u32,
            generated: false,
            hyphen: false,
            font_name: "Body".into(),
            font_size: 10.0,
            bounds: BoundingBox { left, bottom: baseline - 2.0, right: left + 5.0, top: baseline + 8.0 },
            baseline,
        }
    }

    fn texts(runs: &[TextRun]) -> Vec<&str> {
        runs.iter().map(|run| run.text.as_str()).collect()
    }

    #[test]
    fn adjacent_chars_form_one_run_with_spaces_kept_inside() {
        let mut builder = RunBuilder::new(1);
        builder.push(char_at('a', 0.0, 100.0));
        builder.push(char_at(' ', 5.0, 100.0));
        builder.push(char_at('b', 12.0, 100.0));
        builder.push(char_at(' ', 17.0, 100.0));
        assert_eq!(texts(&builder.finish()), ["a b"]);
    }

    #[test]
    fn a_cell_wide_gap_after_a_space_starts_a_new_run() {
        let mut builder = RunBuilder::new(1);
        builder.push(char_at('a', 0.0, 100.0));
        builder.push(char_at(' ', 5.0, 100.0));
        builder.push(char_at('b', 40.0, 100.0));
        assert_eq!(texts(&builder.finish()), ["a", "b"]);
    }

    #[test]
    fn hyphen_marker_becomes_ascii_hyphen_and_flags_the_run() {
        let mut builder = RunBuilder::new(9);
        builder.push(char_at('u', 0.0, 100.0));
        builder.push(CharInfo { unicode: PDFIUM_HYPHEN_MARKER, hyphen: true, ..char_at('-', 5.0, 100.0) });
        builder.push(char_at('n', 0.0, 88.0));
        let runs = builder.finish();
        assert_eq!(texts(&runs), ["u-", "n"]);
        assert!(runs[0].line_end_hyphen);
        assert!(!runs[1].line_end_hyphen);
    }

    #[test]
    fn a_new_baseline_a_wide_gap_or_a_line_break_starts_a_new_run() {
        let mut builder = RunBuilder::new(1);
        builder.push(char_at('a', 0.0, 100.0));
        builder.push(char_at('b', 40.0, 100.0));
        builder.push(char_at('c', 45.0, 103.0));
        builder.push(CharInfo { generated: true, ..char_at('\n', 50.0, 103.0) });
        builder.push(char_at('d', 50.0, 103.0));
        assert_eq!(texts(&builder.finish()), ["a", "b", "c", "d"]);
    }

    #[test]
    fn surrogate_pairs_become_one_char() {
        let mut builder = RunBuilder::new(56);
        builder.push(CharInfo { unicode: 0xD835, ..char_at('x', 0.0, 100.0) });
        builder.push(CharInfo { unicode: 0xDC26, ..char_at('x', 0.0, 100.0) });
        assert_eq!(texts(&builder.finish()), ["𝐦"]);
    }

    #[test]
    fn subset_prefix_is_removed_only_when_it_looks_like_one() {
        assert_eq!(strip_subset_prefix("DZPUDC+CrimsonText-Roman"), "CrimsonText-Roman");
        assert_eq!(strip_subset_prefix("CourierNew,Bold"), "CourierNew,Bold");
        assert_eq!(strip_subset_prefix("A+B"), "A+B");
    }
}
