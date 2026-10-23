//! Rules 19 and 20: text that stays on screen but is not read aloud: citations (`[12]`, `[AIS+77]`),
//! URLs, long bare domains, e-mail addresses, emoji and page cross-references (`Visitor (page 366)`).

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

/// Punctuation that ends a sentence rather than a URL.
const URL_TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', ')', '’', '”', '"', '\'', '!', '?'];

static CITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|\s)\[(\d+([,–-]\s?\d+)*|[A-Z][A-Za-z+]*\d{2}[a-z]?(,\s*[A-Z][A-Za-z+]*\d{2}[a-z]?)*)\]").expect("valid regex"));
/// Bare domains up to this length (`shop.acme.com`, `example.com/signup`) are read out: a sentence that
/// drops them (`requests for … go to`) is worse to hear than the name. Longer ones are links.
const MAX_SPOKEN_BARE_DOMAIN_CHARS: usize = 30;

static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(https?://|www\.)\S+|@?\b[\w.+-]+@[\w-]+(\.[\w-]+)+").expect("valid regex"));
static BARE_DOMAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[a-z0-9][a-z0-9-]*(\.[a-z0-9-]+)*\.(com|org|net|io|dev|ai|gov|edu|co|me|info|ly)(/\S*)?\b").expect("valid regex"));
static EMOJI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Extended_Pictographic}\s?").expect("valid regex"));
static PAGE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Z][A-Za-z]+( \((page )?\d{2,3}\))").expect("valid regex"));

/// Byte ranges of `text` to leave out of speech, sorted and not overlapping.
pub fn silent_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    ranges.extend(CITATION.find_iter(text).map(|found| found.range()));
    ranges.extend(URL.find_iter(text).map(|found| trim_url(text, found.range())));
    ranges.extend(BARE_DOMAIN.find_iter(text).map(|found| trim_url(text, found.range())).filter(|range| range.len() > MAX_SPOKEN_BARE_DOMAIN_CHARS));
    ranges.extend(EMOJI.find_iter(text).map(|found| found.range()));
    ranges.extend(PAGE_REFERENCE.captures_iter(text).filter_map(|captures| captures.get(1)).map(|found| found.range()));
    merge(ranges)
}

fn trim_url(text: &str, range: Range<usize>) -> Range<usize> {
    let trimmed = text[range.clone()].trim_end_matches(URL_TRAILING_PUNCTUATION);
    range.start..range.start + trimmed.len()
}

pub fn merge(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.retain(|range| !range.is_empty());
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spoken(text: &str) -> String {
        let mut kept = String::new();
        let mut cursor = 0;
        for range in silent_ranges(text) {
            kept.push_str(&text[cursor..range.start]);
            cursor = range.end;
        }
        kept.push_str(&text[cursor..]);
        kept
    }

    #[test]
    fn citations_are_not_spoken() {
        assert_eq!(spoken("as the old note says\" [ABC+77]. Still, we"), "as the old note says\". Still, we");
        assert_eq!(spoken("as shown [12, 14] here"), "as shown here");
        assert_eq!(spoken("see [JCJO92, Boo94]."), "see.");
        assert_eq!(spoken("an array a[0] stays"), "an array a[0] stays");
    }

    #[test]
    fn urls_long_domains_and_emails_are_not_spoken_but_trailing_punctuation_is() {
        assert_eq!(spoken("Visit http://localhost:9000/stats."), "Visit .");
        assert_eq!(spoken("Ping someone@example.com today"), "Ping  today");
        assert_eq!(spoken("Mastodon: @someone@social.example.io"), "Mastodon: ");
        assert_eq!(spoken("see files.example-mirror.com/projects/sample-tool/x.yaml now"), "see  now");
        assert_eq!(spoken("the file shape.h is small"), "the file shape.h is small");
    }

    #[test]
    fn short_bare_domains_are_spoken() {
        assert_eq!(spoken("go to example.com/signup and"), "go to example.com/signup and");
        assert_eq!(spoken("In this example, shop.acme.com is routed"), "In this example, shop.acme.com is routed");
    }

    #[test]
    fn emoji_and_page_references_are_not_spoken() {
        assert_eq!(spoken("🔹 What is Sensitive Data?"), "What is Sensitive Data?");
        assert_eq!(spoken("Visitor (page 366) and Iterator (289) help"), "Visitor and Iterator help");
        assert_eq!(spoken("port (80) stays"), "port (80) stays");
    }
}
