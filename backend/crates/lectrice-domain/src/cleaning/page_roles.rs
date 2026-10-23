//! Rules 6 and 7: whole-page classifiers. The outline is tried first, then the shape of the lines.

use std::sync::LazyLock;

use regex::Regex;

use super::page::{Page, PageRole};
use crate::pdf::OutlineItem;

/// Fewer real chars than this and the page counts as image-only.
const MIN_CHARS_FOR_TEXT_PAGE: usize = 5;
/// The table of contents is looked for in the first pages, the index in the last ones, copyright at either end.
const FRONT_SHARE: f32 = 0.15;
const BACK_SHARE: f32 = 0.35;
const COPYRIGHT_FRONT_SHARE: f32 = 0.1;
const COPYRIGHT_BACK_SHARE: f32 = 0.03;
const MIN_FRONT_PAGES: usize = 10;
const MIN_LISTING_LINES: usize = 5;
/// Share of a page's lines that must look like TOC or index entries.
const LISTING_SHARE: f32 = 0.4;
const BIBLIOGRAPHY_SHARE: f32 = 0.3;
const MIN_BIBLIOGRAPHY_ENTRIES: usize = 3;

static TOC_ENTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"((\.\s*){2,}|\s)(\d{1,4}|[ivxlc]{1,7})$").expect("valid regex"));
static INDEX_ENTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(,\s*\d+(\s*[–-]\s*\d+)?$)|(\bsee also\b)").expect("valid regex"));
static BIBLIOGRAPHY_ENTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[[A-Za-z+]+\d*[a-z]?\]").expect("valid regex"));
static COPYRIGHT_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bISBN(-1[03])?:?\s*\d[\d‐-]{9,}|all rights reserved|copyright\s*©|©\s*\d{4}").expect("valid regex"));

pub fn classify_pages(pages: &mut [Page], outline: &[OutlineItem]) {
    let page_count = pages.len();
    for page in pages.iter_mut() {
        if page.content_char_count() < MIN_CHARS_FOR_TEXT_PAGE {
            page.role = PageRole::Empty;
        }
    }
    for (role, first, last) in outline_sections(outline, pages.last().map_or(0, |page| page.index)) {
        for page in pages.iter_mut().filter(|page| page.role == PageRole::Body && (first..=last).contains(&page.index)) {
            page.role = role;
        }
    }
    let mut previous_was_index = false;
    for (position, page) in pages.iter_mut().enumerate() {
        if page.role == PageRole::Body {
            page.role = role_from_lines(page, position, page_count, previous_was_index);
        }
        previous_was_index = page.role == PageRole::Index;
    }
}

fn role_for_title(title: &str) -> Option<PageRole> {
    let normalized: String = title.trim_start_matches(|character: char| !character.is_alphabetic()).to_lowercase();
    match normalized.trim() {
        "contents" | "table of contents" | "toc" => Some(PageRole::TableOfContents),
        "index" => Some(PageRole::Index),
        "bibliography" | "references" | "works cited" => Some(PageRole::Bibliography),
        "copyright" => Some(PageRole::Copyright),
        _ => None,
    }
}

/// `(role, first page, last page)` for outline items whose title names a special section. A section
/// ends where the next item of the same or a higher level starts.
fn outline_sections(outline: &[OutlineItem], last_page: u32) -> Vec<(PageRole, u32, u32)> {
    let mut sections = Vec::new();
    for (position, item) in outline.iter().enumerate() {
        let (Some(role), Some(first)) = (role_for_title(&item.title), item.pdf_page) else { continue };
        let next_start = outline[position + 1..].iter().filter(|next| next.level <= item.level).filter_map(|next| next.pdf_page).find(|page| *page > first);
        sections.push((role, first, next_start.map_or(last_page, |page| page - 1)));
    }
    sections
}

fn role_from_lines(page: &Page, position: usize, page_count: usize, previous_was_index: bool) -> PageRole {
    let texts: Vec<&str> = page.content_lines().map(|line| line.text.trim()).collect();
    let share = |regex: &Regex| texts.iter().filter(|text| regex.is_match(text)).count() as f32 / texts.len().max(1) as f32;
    let in_front = |share_of_pages: f32| position < MIN_FRONT_PAGES.max((page_count as f32 * share_of_pages) as usize);
    let in_back = |share_of_pages: f32| position as f32 >= page_count as f32 * (1.0 - share_of_pages);
    let enough_lines = texts.len() >= MIN_LISTING_LINES;

    if enough_lines && in_front(FRONT_SHARE) && share(&TOC_ENTRY) >= LISTING_SHARE {
        return PageRole::TableOfContents;
    }
    let starts_index = texts.first().is_some_and(|first| first.eq_ignore_ascii_case("index"));
    if enough_lines && in_back(BACK_SHARE) && share(&INDEX_ENTRY) >= LISTING_SHARE && (starts_index || previous_was_index) {
        return PageRole::Index;
    }
    let bibliography_entries = texts.iter().filter(|text| BIBLIOGRAPHY_ENTRY.is_match(text)).count();
    if bibliography_entries >= MIN_BIBLIOGRAPHY_ENTRIES && share(&BIBLIOGRAPHY_ENTRY) >= BIBLIOGRAPHY_SHARE {
        return PageRole::Bibliography;
    }
    let at_an_end = in_front(COPYRIGHT_FRONT_SHARE) || in_back(COPYRIGHT_BACK_SHARE);
    if at_an_end && texts.iter().any(|text| COPYRIGHT_MARK.is_match(text)) {
        return PageRole::Copyright;
    }
    PageRole::Body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, level: u8, page: u32) -> OutlineItem {
        OutlineItem { title: title.into(), level, pdf_page: Some(page) }
    }

    #[test]
    fn outline_sections_end_at_the_next_item_of_the_same_level() {
        let outline = [item("Table of Contents", 1, 5), item("0: Preface", 1, 9), item("Editions", 2, 9), item("Index", 1, 308)];
        assert_eq!(outline_sections(&outline, 308), [(PageRole::TableOfContents, 5, 8), (PageRole::Index, 308, 308)]);
    }

    #[test]
    fn entry_shapes() {
        assert!(TOC_ENTRY.is_match("Preface to CD .... 5"));
        assert!(TOC_ENTRY.is_match("Big Endian vs Little Endian 7"));
        assert!(INDEX_ENTRY.is_match("BOSS, 351–352"));
        assert!(INDEX_ENTRY.is_match("adapter patterns, 31-39"));
        assert!(BIBLIOGRAPHY_ENTRY.is_match("[AG90] D. Alexander"));
        assert!(BIBLIOGRAPHY_ENTRY.is_match("[RBP+91] James"));
        assert!(COPYRIGHT_MARK.is_match("ISBN: 978-1-234-56789-7"));
        assert!(COPYRIGHT_MARK.is_match("ISBN-13: 978-0-12-345678-9"));
        assert!(!COPYRIGHT_MARK.is_match("Type the ISBN (9781234567897) in the form and press Send."));
        assert!(!COPYRIGHT_MARK.is_match("the copyright holder of the code"));
    }
}
