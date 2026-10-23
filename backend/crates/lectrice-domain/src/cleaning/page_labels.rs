//! Rule 2: printed page labels. Embedded labels win; without them the label is inferred from page
//! numbers printed in the header/footer bands (GoF prints `11` on PDF page 11).

use std::collections::{HashMap, HashSet};

use super::bands::{band_of, edge_tokens};
use super::page::Page;
use crate::document::PageLabel;

/// The inferred offset (PDF index − printed number) must be seen on this share of the pages with text…
const MIN_OFFSET_SHARE: f32 = 0.2;
/// …and on at least this many pages.
const MIN_OFFSET_PAGES: usize = 3;

/// The label expected on each page, in page order.
pub fn resolve_labels(pages: &[Page]) -> Vec<Option<String>> {
    if pages.iter().any(|page| page.embedded_label.is_some()) {
        return pages.iter().map(|page| page.embedded_label.clone()).collect();
    }
    let Some(offset) = printed_number_offset(pages) else { return vec![None; pages.len()] };
    pages.iter().map(|page| i64::from(page.index).checked_sub(offset).filter(|number| *number >= 1).map(|number| number.to_string())).collect()
}

pub fn page_labels(pages: &[Page], labels: &[Option<String>]) -> Vec<PageLabel> {
    pages.iter().zip(labels).filter_map(|(page, label)| label.as_ref().map(|label| PageLabel { index: page.index, label: label.clone() })).collect()
}

fn printed_number_offset(pages: &[Page]) -> Option<i64> {
    let mut pages_per_offset: HashMap<i64, usize> = HashMap::new();
    let mut text_pages = 0;
    for page in pages.iter().filter(|page| !page.lines.is_empty()) {
        text_pages += 1;
        let offsets: HashSet<i64> = page
            .lines
            .iter()
            .filter(|line| band_of(line, page.height).is_some())
            .filter_map(|line| edge_tokens(&line.text))
            .flat_map(|(first, last)| [first, last])
            .filter_map(|token| token.parse::<u32>().ok())
            .map(|number| i64::from(page.index) - i64::from(number))
            .collect();
        for offset in offsets {
            *pages_per_offset.entry(offset).or_default() += 1;
        }
    }
    let threshold = MIN_OFFSET_PAGES.max((text_pages as f32 * MIN_OFFSET_SHARE).ceil() as usize);
    pages_per_offset.into_iter().filter(|(_, count)| *count >= threshold).max_by_key(|(offset, count)| (*count, -offset.abs())).map(|(offset, _)| offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaning::lines::tests::{lines_of, run};
    use crate::cleaning::page::PageRole;

    fn page(index: u32, footer: &str) -> Page {
        let lines = lines_of(&[run("Body text", 111.0, 500.0, 8.4, "Courier"), run(footer, 480.0, 63.0, 13.2, "Courier,Bold")]);
        Page { index, width: 612.0, height: 792.0, embedded_label: None, lines, role: PageRole::Body }
    }

    #[test]
    fn printed_footer_numbers_give_the_labels_when_none_are_embedded() {
        let pages = [page(11, "11"), page(12, "12"), page(13, "13"), page(14, "Chapter end")];
        assert_eq!(resolve_labels(&pages), [Some("11".into()), Some("12".into()), Some("13".into()), Some("14".into())]);
    }

    #[test]
    fn embedded_labels_win_and_scattered_numbers_give_none() {
        let mut embedded = page(9, "1");
        embedded.embedded_label = Some("1".into());
        assert_eq!(resolve_labels(&[embedded]), [Some("1".into())]);
        assert_eq!(resolve_labels(&[page(1, "7"), page(2, "40")]), [None, None]);
    }
}
