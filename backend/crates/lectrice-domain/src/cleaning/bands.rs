//! Rules 4 and 5: running headers, footers and page numbers, found by position (never by char order).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use super::lines::{Line, LineRole};
use super::page::Page;

/// The top band starts at this share of the page height (measured from the bottom).
const TOP_BAND_START: f32 = 0.88;
/// The bottom band ends at this share of the page height.
const BOTTOM_BAND_END: f32 = 0.10;
/// A band text seen on this share of the pages with text is a running header or footer…
const REPEATED_SHARE: f32 = 0.3;
/// …and on at least this many pages.
const MIN_REPEATED_PAGES: usize = 3;

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").expect("valid regex"));
/// Printed roman page numbers are lowercase; an uppercase `I` is more likely a part heading.
static PAGE_NUMBER_ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,4}|[ivxlc]{1,7})$").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Band {
    Top,
    Bottom,
}

pub fn band_of(line: &Line, page_height: f32) -> Option<Band> {
    if line.baseline >= page_height * TOP_BAND_START {
        Some(Band::Top)
    } else if line.baseline <= page_height * BOTTOM_BAND_END {
        Some(Band::Bottom)
    } else {
        None
    }
}

/// The first and last tokens of a band line, split on spaces and `|` (`12 | Chapter 2`, `… Sidecars | 13`).
pub fn edge_tokens(text: &str) -> Option<(&str, &str)> {
    let mut tokens = text.split(|character: char| character.is_whitespace() || character == '|').filter(|token| !token.is_empty());
    let first = tokens.next()?;
    Some((first, tokens.next_back().unwrap_or(first)))
}

/// Marks running headers, footers and page numbers as `RunningHeader`. `labels[i]` is the printed
/// label expected on `pages[i]`.
pub fn mark_running_headers(pages: &mut [Page], labels: &[Option<String>]) {
    let repeated = repeated_band_texts(pages);
    for (page, label) in pages.iter_mut().zip(labels) {
        let height = page.height;
        for line in page.lines.iter_mut().filter(|line| line.is_content()) {
            let Some(band) = band_of(line, height) else { continue };
            let is_repeated = repeated.contains(&(band, band_key(&line.text)));
            if is_repeated || carries_label(&line.text, label.as_deref()) || PAGE_NUMBER_ONLY.is_match(line.text.trim()) {
                line.role = LineRole::RunningHeader;
            }
        }
    }
}

fn carries_label(text: &str, label: Option<&str>) -> bool {
    let Some(label) = label else { return false };
    edge_tokens(text).is_some_and(|(first, last)| first.eq_ignore_ascii_case(label) || last.eq_ignore_ascii_case(label))
}

/// Digits become `#` so `12 | Chapter 2` and `14 | Chapter 2` compare equal.
fn band_key(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let without_digits = DIGITS.replace_all(&lowered, "#");
    without_digits.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn repeated_band_texts(pages: &[Page]) -> HashSet<(Band, String)> {
    let mut pages_seen: HashMap<(Band, String), usize> = HashMap::new();
    let mut text_pages = 0;
    for page in pages {
        if page.lines.is_empty() {
            continue;
        }
        text_pages += 1;
        let keys: HashSet<(Band, String)> = page.lines.iter().filter_map(|line| band_of(line, page.height).map(|band| (band, band_key(&line.text)))).collect();
        for key in keys {
            *pages_seen.entry(key).or_default() += 1;
        }
    }
    let threshold = MIN_REPEATED_PAGES.max((text_pages as f32 * REPEATED_SHARE).ceil() as usize);
    pages_seen.into_iter().filter(|(_, count)| *count >= threshold).map(|(key, _)| key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_tokens_split_on_bars_and_spaces() {
        assert_eq!(edge_tokens("12 | Chapter 2: The Sidecar Pattern"), Some(("12", "Pattern")));
        assert_eq!(edge_tokens("Dynamic Configuration with Sidecars | 13"), Some(("Dynamic", "13")));
        assert_eq!(edge_tokens("1: Kubernetes primer 4"), Some(("1:", "4")));
        assert_eq!(edge_tokens("vii"), Some(("vii", "vii")));
    }

    #[test]
    fn band_keys_ignore_digits_and_case() {
        assert_eq!(band_key("12 | Chapter 2: The Sidecar Pattern"), band_key("14 | CHAPTER 2: The  Sidecar Pattern"));
        assert_ne!(band_key("1: Kubernetes primer 4"), band_key("4: Working with Pods 52"));
    }

    #[test]
    fn labels_must_sit_at_an_edge() {
        assert!(carries_label("1: Kubernetes primer 4", Some("4")));
        assert!(!carries_label("1https://www.cncf.io", Some("1")));
        assert!(!carries_label("Chapter 4 ends here", Some("4")));
        assert!(!carries_label("Anything 4", None));
    }
}
