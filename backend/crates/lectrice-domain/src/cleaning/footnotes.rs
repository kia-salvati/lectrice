//! Rule 8a: footnote bodies. Small, non-code lines at the bottom of a page, below every body line,
//! the first of which starts with a number (`1. And she…`, `1 Kubernetes…`, `1https://…`).

use std::sync::LazyLock;

use regex::Regex;

use super::lines::{Line, LineRole};
use super::page::Page;
use super::profile::Profile;

/// Footnotes are at most this share of the body size (CA 0.85, K8s 0.80, DDS 0.76)…
const MAX_FOOTNOTE_SIZE_RATIO: f32 = 0.88;
/// …and not tiny decorations.
const MIN_FOOTNOTE_SIZE_RATIO: f32 = 0.5;

static FOOTNOTE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,3})(\.\s+|\s+)?[\p{L}“\x22(]").expect("valid regex"));

#[derive(Debug, Clone, PartialEq)]
pub struct Footnote {
    pub page: u32,
    pub number: u32,
    pub lines: Vec<Line>,
}

/// Marks the footnote lines of `page` as `Footnote` and returns them, top to bottom.
pub fn take_footnotes(page: &mut Page, profile: &Profile) -> Vec<Footnote> {
    let mut bottom_up: Vec<usize> = (0..page.lines.len()).filter(|&index| page.lines[index].is_content()).collect();
    bottom_up.sort_by(|&left, &right| page.lines[left].baseline.total_cmp(&page.lines[right].baseline));

    let zone_length = bottom_up.iter().take_while(|&&index| is_footnote_sized(&page.lines[index], profile)).count();
    if zone_length == bottom_up.len() {
        return Vec::new();
    }
    let zone = &bottom_up[..zone_length];
    let Some(top_start) = zone.iter().rposition(|&index| start_number(&page.lines[index]).is_some()) else { return Vec::new() };

    let mut footnotes: Vec<Footnote> = Vec::new();
    for &index in zone[..=top_start].iter().rev() {
        let line = &mut page.lines[index];
        line.role = LineRole::Footnote;
        match start_number(line) {
            Some(number) => footnotes.push(Footnote { page: page.index, number, lines: vec![line.clone()] }),
            None => footnotes.last_mut().expect("the top line starts a footnote").lines.push(line.clone()),
        }
    }
    footnotes
}

fn is_footnote_sized(line: &Line, profile: &Profile) -> bool {
    let ratio = profile.ratio(line.size);
    (MIN_FOOTNOTE_SIZE_RATIO..=MAX_FOOTNOTE_SIZE_RATIO).contains(&ratio) && !line.style.monospace
}

fn start_number(line: &Line) -> Option<u32> {
    FOOTNOTE_START.captures(&line.text).and_then(|captures| captures[1].parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaning::lines::tests::{lines_of, run};
    use crate::cleaning::page::PageRole;

    fn page(lines: Vec<Line>) -> Page {
        Page { index: 21, width: 612.0, height: 792.0, embedded_label: None, lines, role: PageRole::Body }
    }

    fn profile() -> Profile {
        let body = lines_of(&[run("body", 50.0, 700.0, 15.0, "Times"), run("body", 50.0, 683.0, 15.0, "Times")]);
        Profile::from_pages(&[&page(body)])
    }

    #[test]
    fn small_numbered_lines_below_the_body_are_footnotes_even_when_they_come_first() {
        let mut page = page(lines_of(&[
            run("1. Maps of this kind are older than most people", 51.0, 85.0, 12.75, "Times"),
            run("think.", 51.0, 70.0, 12.75, "Times"),
            run("Suppose a walker holds a paper map", 50.0, 700.0, 15.0, "Times"),
        ]));
        let footnotes = take_footnotes(&mut page, &profile());
        assert_eq!(footnotes.len(), 1);
        assert_eq!(footnotes[0].number, 1);
        assert_eq!(footnotes[0].lines.len(), 2);
        assert_eq!(page.lines.iter().filter(|line| line.role == LineRole::Footnote).count(), 2);
    }

    #[test]
    fn small_lines_without_a_number_or_body_sized_numbered_lists_are_not_footnotes() {
        let mut caption_only = page(lines_of(&[run("Body text", 50.0, 700.0, 15.0, "Times"), run("Figure 1.2", 50.0, 90.0, 12.0, "Times")]));
        assert!(take_footnotes(&mut caption_only, &profile()).is_empty());
        let mut list = page(lines_of(&[run("Body text", 50.0, 700.0, 15.0, "Times"), run("1. Urgent and important", 50.0, 90.0, 15.0, "Times")]));
        assert!(take_footnotes(&mut list, &profile()).is_empty());
    }
}
