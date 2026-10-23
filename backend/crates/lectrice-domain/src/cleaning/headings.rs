//! Rule 11: heading titles and levels. A heading that matches an outline item near its page takes
//! the outline's casing and level (CA prints `WHAT IS DESIGN AND ARCHITECTURE?`). Other headings get
//! the level that outline-matched headings of the same size have, or one below the nearest larger
//! matched size, or else their size rank.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::pdf::OutlineItem;

/// Outline items may point one page before or after the printed heading.
const PAGE_SLACK: u32 = 1;
const MAX_LEVEL: u8 = 4;
const SIZE_BUCKET: f32 = 2.0;

/// Words that number a heading rather than name it: `Chapter 1`, `PART I`, `3:`.
static NUMBERING_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(chapter|part|appendix|\d+|[a-z]|x{0,3}(ix|iv|v?i{1,3}|v))$").expect("valid regex"));

/// A heading as printed: its joined lines, the page it starts on and its largest font size.
#[derive(Debug, Clone, PartialEq)]
pub struct RawHeading {
    pub text: String,
    pub page: u32,
    pub size: f32,
}

/// The shown title and level of each heading, in the same order.
pub fn name_headings(outline: &[OutlineItem], headings: &[RawHeading]) -> Vec<(String, u8)> {
    let outline: Vec<(&OutlineItem, String)> = outline.iter().map(|item| (item, core_words(&item.title))).filter(|(_, core)| !core.is_empty()).collect();
    let matches: Vec<Option<&OutlineItem>> = headings.iter().map(|heading| outline_match(&outline, heading)).collect();
    let matched_levels = matched_level_by_size(headings, &matches);
    let mut size_ranks: Vec<i32> = headings.iter().map(|heading| size_bucket(heading.size)).collect();
    size_ranks.sort_unstable_by(|left, right| right.cmp(left));
    size_ranks.dedup();

    headings
        .iter()
        .zip(matches)
        .map(|(heading, matched)| match matched {
            Some(item) => (item.title.clone(), item.level.min(MAX_LEVEL)),
            None => (heading.text.clone(), unmatched_level(size_bucket(heading.size), &matched_levels, &size_ranks)),
        })
        .collect()
}

fn outline_match<'a>(outline: &[(&'a OutlineItem, String)], heading: &RawHeading) -> Option<&'a OutlineItem> {
    let core = core_words(&heading.text);
    if core.is_empty() {
        return None;
    }
    let near = |item: &OutlineItem| item.pdf_page.is_none_or(|item_page| item_page.abs_diff(heading.page) <= PAGE_SLACK);
    outline.iter().find(|(item, item_core)| *item_core == core && near(item)).map(|(item, _)| *item)
}

/// The most common outline level of the matched headings, per size bucket.
fn matched_level_by_size(headings: &[RawHeading], matches: &[Option<&OutlineItem>]) -> HashMap<i32, u8> {
    let mut counts: HashMap<(i32, u8), usize> = HashMap::new();
    for (heading, matched) in headings.iter().zip(matches) {
        if let Some(item) = matched {
            *counts.entry((size_bucket(heading.size), item.level.min(MAX_LEVEL))).or_default() += 1;
        }
    }
    let mut best: HashMap<i32, (u8, usize)> = HashMap::new();
    for ((bucket, level), count) in counts {
        let entry = best.entry(bucket).or_insert((level, count));
        if count > entry.1 || (count == entry.1 && level < entry.0) {
            *entry = (level, count);
        }
    }
    best.into_iter().map(|(bucket, (level, _))| (bucket, level)).collect()
}

fn unmatched_level(bucket: i32, matched_levels: &HashMap<i32, u8>, size_ranks: &[i32]) -> u8 {
    if let Some(level) = matched_levels.get(&bucket) {
        return *level;
    }
    let below_larger = matched_levels.iter().filter(|(matched_bucket, _)| **matched_bucket > bucket).map(|(_, level)| level + 1).max();
    let by_rank = || (size_ranks.iter().position(|rank| *rank <= bucket).unwrap_or(size_ranks.len()) + 1).min(usize::from(MAX_LEVEL)) as u8;
    below_larger.unwrap_or_else(by_rank).min(MAX_LEVEL)
}

fn size_bucket(size: f32) -> i32 {
    (size * SIZE_BUCKET).round() as i32
}

/// Lowercase words without punctuation and without the leading numbering words.
fn core_words(text: &str) -> String {
    let lowered = text.to_lowercase();
    let words: Vec<&str> = lowered.split(|character: char| !character.is_alphanumeric()).filter(|word| !word.is_empty()).collect();
    let first_name_word = words.iter().position(|word| !NUMBERING_WORD.is_match(word)).unwrap_or(words.len());
    words[first_name_word..].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, level: u8, page: u32) -> OutlineItem {
        OutlineItem { title: title.into(), level, pdf_page: Some(page) }
    }

    fn heading(text: &str, page: u32, size: f32) -> RawHeading {
        RawHeading { text: text.into(), page, size }
    }

    #[test]
    fn outline_casing_and_level_win_when_the_words_match_nearby() {
        let outline = [item("PART I Introduction", 1, 25), item("Chapter 1 What Is Design and Architecture?", 2, 27), item("The Goal?", 3, 28)];
        let headings = [heading("I INTRODUCTION", 25, 30.0), heading("1 WHAT IS DESIGN AND ARCHITECTURE?", 27, 30.0), heading("THE GOAL?", 28, 24.75)];
        let names = name_headings(&outline, &headings);
        assert_eq!(names[0], ("PART I Introduction".into(), 1));
        assert_eq!(names[1], ("Chapter 1 What Is Design and Architecture?".into(), 2));
        assert_eq!(names[2], ("The Goal?".into(), 3));
    }

    #[test]
    fn unmatched_headings_follow_matched_sizes() {
        let outline = [item("1: Kubernetes primer", 1, 11), item("Important Kubernetes background", 2, 11)];
        let headings = [
            heading("1: Kubernetes primer", 11, 20.7),
            heading("Important Kubernetes background", 11, 14.3),
            heading("Orchestration", 11, 12.0),
            heading("Pods", 40, 14.3),
        ];
        let levels: Vec<u8> = name_headings(&outline, &headings).into_iter().map(|(_, level)| level).collect();
        assert_eq!(levels, [1, 2, 3, 2]);
    }

    #[test]
    fn without_an_outline_levels_follow_the_size_rank() {
        let names = name_headings(&[], &[heading("Big", 1, 20.0), heading("Small", 1, 12.0)]);
        assert_eq!(names, [("Big".into(), 1), ("Small".into(), 2)]);
    }

    #[test]
    fn far_away_outline_items_do_not_match() {
        let names = name_headings(&[item("Conclusion", 3, 34)], &[heading("CONCLUSION", 90, 12.0)]);
        assert_eq!(names[0].0, "CONCLUSION");
    }
}
