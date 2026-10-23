//! The table of contents (`Document::toc`). The PDF outline is used when at least half of its entries are
//! found as heading blocks (same words, nearby page); otherwise the heading blocks themselves are. Front-matter
//! entries, entries whose page has no text and extra entries that share a page with the one before (LinkedIn's
//! outline holds post sentences) are left out. Levels become depths: an entry is one deeper than the nearest
//! entry before it with a smaller level, so siblings stay siblings and no step is skipped; deeper than
//! `MAX_TOC_LEVEL` is left out.

use std::sync::LazyLock;

use regex::Regex;

use super::headings::{PAGE_SLACK, core_words};
use crate::document::{Block, BlockKind, Sentence, TocEntry};
use crate::ids::SentenceId;
use crate::pdf::OutlineItem;

const MAX_TOC_LEVEL: u8 = 3;
/// Below this share of entries found as headings, the outline doesn't describe the text well enough.
const MIN_MATCHED_SHARE: f32 = 0.5;
const MAX_TITLE_CHARS: usize = 120;
const ELLIPSIS: char = '…';
/// LinkedIn's outline joins a post's title and its first sentence with a zero-width space.
const TITLE_END: char = '\u{200B}';
const INVISIBLE: [char; 2] = ['\u{FEFF}', '\u{FE0F}'];

static FRONT_MATTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(front )?cover$|^(half )?title( page)?$|^copyright( page)?$|^(table of )?contents$|^tweet this book!?$").expect("valid regex")
});

/// A heading block as the table of contents sees it.
struct Heading<'a> {
    title: String,
    core: String,
    level: u8,
    spoken: bool,
    first: &'a Sentence,
}

/// A table-of-contents line before it is numbered and its level normalized.
struct Anchor {
    title: String,
    level: u8,
    sentence_id: SentenceId,
    page_index: u32,
}

impl Anchor {
    fn at(title: String, level: u8, sentence: &Sentence) -> Self {
        Self { title, level, sentence_id: sentence.id, page_index: sentence.page_index }
    }
}

pub fn build_toc(outline: &[OutlineItem], blocks: &[Block]) -> Vec<TocEntry> {
    let headings = headings(blocks);
    let sentences: Vec<&Sentence> = blocks.iter().flat_map(|block| &block.sentences).collect();
    let anchors = from_outline(outline, &headings, &sentences).unwrap_or_else(|| from_headings(&headings));
    normalize_levels(anchors)
        .into_iter()
        .zip(0..)
        .map(|(anchor, id)| TocEntry { id, title: anchor.title, level: anchor.level, sentence_id: anchor.sentence_id, page_index: anchor.page_index })
        .collect()
}

fn headings(blocks: &[Block]) -> Vec<Heading<'_>> {
    blocks
        .iter()
        .filter(|block| block.kind == BlockKind::Heading)
        .filter_map(|block| {
            let first = block.sentences.first()?;
            let text: Vec<&str> = block.sentences.iter().map(|sentence| sentence.text.as_str()).collect();
            let title = clean_title(&text.join(" "));
            let spoken = block.sentences.iter().any(|sentence| !sentence.spoken_text().trim().is_empty());
            Some(Heading { core: core_words(&title), title, level: block.level.unwrap_or(1), spoken, first })
        })
        .collect()
}

/// `None` when the PDF has no usable outline or it doesn't match the headings well.
fn from_outline(outline: &[OutlineItem], headings: &[Heading], sentences: &[&Sentence]) -> Option<Vec<Anchor>> {
    let mut anchors: Vec<Anchor> = Vec::new();
    let mut next_heading = 0;
    let mut matched = 0_usize;
    for item in outline {
        let title = clean_title(&item.title);
        if is_junk_title(&title) {
            continue;
        }
        let previous = anchors.last();
        let found = find_heading(&headings[next_heading..], &title, item.pdf_page).map(|offset| next_heading + offset);
        let anchor = match found {
            Some(index) => {
                next_heading = index + 1;
                let heading = &headings[index];
                // GoF splits a long title over two outline entries; the printed heading has all of it.
                let title = if starts_with_words(&heading.core, &core_words(&title)) { heading.title.clone() } else { title };
                Anchor::at(title, item.level, heading.first)
            }
            None => match page_start(sentences, item.pdf_page, previous) {
                Some(sentence) => Anchor::at(title, item.level, sentence),
                None => continue,
            },
        };
        if previous.is_some_and(|previous| anchor.sentence_id <= previous.sentence_id) {
            continue;
        }
        matched += usize::from(found.is_some());
        anchors.push(anchor);
    }
    let trusted = !anchors.is_empty() && matched as f32 >= MIN_MATCHED_SHARE * anchors.len() as f32;
    trusted.then_some(anchors)
}

fn from_headings(headings: &[Heading]) -> Vec<Anchor> {
    headings
        .iter()
        .filter(|heading| heading.spoken && !is_junk_title(&heading.title))
        .map(|heading| Anchor::at(heading.title.clone(), heading.level, heading.first))
        .collect()
}

/// The first heading near `page` whose words equal the title's, or start it, or are started by it
/// (`Editions` for `Editions Paperbacks, hardbacks…`).
fn find_heading(headings: &[Heading], title: &str, page: Option<u32>) -> Option<usize> {
    let core = core_words(title);
    if core.is_empty() {
        return None;
    }
    let near = |heading: &Heading| page.is_none_or(|page| page.abs_diff(heading.first.page_index) <= PAGE_SLACK);
    headings.iter().position(|heading| near(heading) && same_words(&core, &heading.core))
}

fn same_words(left: &str, right: &str) -> bool {
    left == right || starts_with_words(left, right) || starts_with_words(right, left)
}

fn starts_with_words(long: &str, short: &str) -> bool {
    !short.is_empty() && long.strip_prefix(short).is_some_and(|rest| rest.starts_with(' '))
}

/// The first sentence on `page` after `previous`, for an outline entry with no heading of its own. `None` when
/// the page has no text (cover, dropped TOC page) or the entry before already points at this page.
/// `sentences` are in id order, so the search starts right after `previous` instead of at the first sentence.
fn page_start<'a>(sentences: &[&'a Sentence], page: Option<u32>, previous: Option<&Anchor>) -> Option<&'a Sentence> {
    let page = page?;
    if previous.is_some_and(|previous| previous.page_index == page) {
        return None;
    }
    let start = previous.map_or(0, |previous| sentences.partition_point(|sentence| sentence.id <= previous.sentence_id));
    let first = sentences[start..].iter().find(|sentence| sentence.page_index >= page)?;
    (first.page_index == page).then_some(*first)
}

/// Each entry's level becomes its depth under the open entries with a smaller level before it.
fn normalize_levels(anchors: Vec<Anchor>) -> Vec<Anchor> {
    let mut open_levels: Vec<u8> = Vec::new();
    anchors
        .into_iter()
        .filter_map(|mut anchor| {
            while open_levels.last().is_some_and(|open| *open >= anchor.level) {
                open_levels.pop();
            }
            open_levels.push(anchor.level);
            anchor.level = u8::try_from(open_levels.len()).unwrap_or(u8::MAX);
            (anchor.level <= MAX_TOC_LEVEL).then_some(anchor)
        })
        .collect()
}

fn is_junk_title(title: &str) -> bool {
    !title.chars().any(char::is_alphanumeric) || FRONT_MATTER.is_match(title)
}

/// The part before a zero-width space, with whitespace collapsed and long titles cut at a word.
fn clean_title(raw: &str) -> String {
    let first_part = raw.split(TITLE_END).map(str::trim).find(|part| !part.is_empty()).unwrap_or_default();
    let visible: String = first_part.chars().filter(|character| !INVISIBLE.contains(character)).collect();
    let title = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title;
    }
    let cut: String = title.chars().take(MAX_TITLE_CHARS).collect();
    let at_word = cut.rfind(' ').map_or(cut.as_str(), |space| &cut[..space]);
    format!("{}{ELLIPSIS}", at_word.trim_end_matches(|character: char| !character.is_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, level: u8, page: u32) -> OutlineItem {
        OutlineItem { title: title.into(), level, pdf_page: Some(page) }
    }

    /// Blocks numbered from sentence 0: `(kind, level, text, page)`, one sentence each.
    fn blocks(specs: &[(BlockKind, Option<u8>, &str, u32)]) -> Vec<Block> {
        specs
            .iter()
            .zip(0..)
            .map(|((kind, level, text, page), id)| Block {
                id,
                kind: *kind,
                level: *level,
                page_index: *page,
                spoken: true,
                text: None,
                sentences: vec![Sentence { id: SentenceId(id), text: (*text).into(), page_index: *page, speech_text: None }],
                cue: None,
            })
            .collect()
    }

    fn heading(level: u8, text: &str, page: u32) -> (BlockKind, Option<u8>, &str, u32) {
        (BlockKind::Heading, Some(level), text, page)
    }

    fn paragraph(text: &str, page: u32) -> (BlockKind, Option<u8>, &str, u32) {
        (BlockKind::Paragraph, None, text, page)
    }

    fn lines(toc: &[TocEntry]) -> Vec<(String, u8, u32, u32)> {
        toc.iter().map(|entry| (entry.title.clone(), entry.level, entry.sentence_id.0, entry.page_index)).collect()
    }

    fn book() -> Vec<Block> {
        blocks(&[
            paragraph("Some cover text.", 1),
            heading(1, "1: Kubernetes primer", 11),
            paragraph("Kubernetes is an orchestrator.", 11),
            heading(2, "Important Kubernetes background", 12),
            paragraph("Text.", 12),
            heading(3, "Orchestration", 13),
            paragraph("More text.", 14),
            heading(1, "2: Principles of operation", 18),
        ])
    }

    #[test]
    fn outline_entries_point_at_their_heading_sentences() {
        let outline = [item("1: Kubernetes primer", 1, 11), item("Important Kubernetes background", 2, 12), item("2: Principles of operation", 1, 18)];

        let toc = build_toc(&outline, &book());

        assert_eq!(
            lines(&toc),
            [("1: Kubernetes primer".into(), 1, 1, 11), ("Important Kubernetes background".into(), 2, 3, 12), ("2: Principles of operation".into(), 1, 7, 18)]
        );
        assert_eq!(toc.iter().map(|entry| entry.id).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn an_outline_entry_without_its_heading_points_at_its_page() {
        let outline = [item("1: Kubernetes primer", 1, 11), item("Deep dive", 2, 14), item("2: Principles of operation", 1, 18)];

        let toc = build_toc(&outline, &book());

        assert_eq!(lines(&toc)[1], ("Deep dive".into(), 2, 6, 14));
    }

    #[test]
    fn a_shorter_or_longer_heading_still_matches() {
        let outline = [item("Important Kubernetes background and history", 1, 12), item("Orchestration", 1, 13)];

        assert_eq!(lines(&build_toc(&outline, &book()))[0].2, 3);
    }

    #[test]
    fn an_outline_title_split_in_two_takes_the_whole_printed_heading() {
        let outline = [item("1: Kubernetes primer", 1, 11), item("Important Kubernetes", 2, 12), item("background", 2, 12)];

        let titles: Vec<String> = build_toc(&outline, &book()).into_iter().map(|entry| entry.title).collect();

        assert_eq!(titles, ["1: Kubernetes primer", "Important Kubernetes background"]);
    }

    #[test]
    fn front_matter_empty_and_textless_entries_are_skipped() {
        let outline = [
            item("Title Page", 1, 1),
            item("Table of Contents", 1, 5),
            item("", 1, 9),
            item("Tweet This Book!", 1, 2),
            item("1: Kubernetes primer", 1, 11),
            item("2: Principles of operation", 1, 18),
        ];

        let titles: Vec<String> = build_toc(&outline, &book()).into_iter().map(|entry| entry.title).collect();

        assert_eq!(titles, ["1: Kubernetes primer", "2: Principles of operation"]);
    }

    #[test]
    fn extra_entries_sharing_the_page_of_the_entry_before_are_skipped() {
        let outline = [
            item("1: Kubernetes primer", 1, 11),
            item("Cloud security is the top priority for any business.", 1, 11),
            item("2: Principles of operation", 1, 18),
        ];

        assert_eq!(build_toc(&outline, &book()).len(), 2);
    }

    #[test]
    fn a_title_stops_at_a_zero_width_space_and_long_titles_are_cut() {
        assert_eq!(clean_title("How does gRPC work?\u{200B}RPC (Remote Procedure Call) is called"), "How does gRPC work?");
        assert_eq!(clean_title("\u{200B}  Cloud   Security\u{FE0F} "), "Cloud Security");
        let long = clean_title(&"word ".repeat(40));
        assert!(long.ends_with("word…") && long.chars().count() <= MAX_TITLE_CHARS + 1);
    }

    #[test]
    fn levels_start_at_one_never_skip_a_step_and_stop_at_three() {
        let outline = [
            item("", 1, 1),
            item("1: Kubernetes primer", 2, 11),
            item("Important Kubernetes background", 4, 12),
            item("Orchestration", 5, 13),
            item("More", 6, 14),
            item("2: Principles of operation", 2, 18),
        ];

        let levels: Vec<(String, u8)> = build_toc(&outline, &book()).into_iter().map(|entry| (entry.title, entry.level)).collect();

        assert_eq!(
            levels,
            [
                ("1: Kubernetes primer".into(), 1),
                ("Important Kubernetes background".into(), 2),
                ("Orchestration".into(), 3),
                ("2: Principles of operation".into(), 1)
            ]
        );
    }

    #[test]
    fn entries_of_the_same_level_stay_siblings_when_the_outline_starts_deeper() {
        let outline = [item("Important Kubernetes background", 2, 12), item("Orchestration", 2, 13), item("2: Principles of operation", 1, 18)];

        let levels: Vec<(String, u8)> = build_toc(&outline, &book()).into_iter().map(|entry| (entry.title, entry.level)).collect();

        assert_eq!(levels, [("Important Kubernetes background".into(), 1), ("Orchestration".into(), 1), ("2: Principles of operation".into(), 1)]);
    }

    #[test]
    fn a_shallower_entry_after_a_deep_one_closes_only_the_deeper_levels() {
        let outline = [
            item("1: Kubernetes primer", 1, 11),
            item("Important Kubernetes background", 3, 12),
            item("Orchestration", 2, 13),
            item("2: Principles of operation", 1, 18),
        ];

        let levels: Vec<u8> = build_toc(&outline, &book()).into_iter().map(|entry| entry.level).collect();

        assert_eq!(levels, [1, 2, 2, 1]);
    }

    #[test]
    fn repeated_titles_match_the_heading_on_their_own_page() {
        let blocks = blocks(&[heading(1, "One", 1), heading(2, "Summary", 2), paragraph("Text.", 3), heading(1, "Two", 4), heading(2, "Summary", 5)]);
        let outline = [item("One", 1, 1), item("Summary", 2, 2), item("Two", 1, 4), item("Summary", 2, 5)];

        let toc = build_toc(&outline, &blocks);

        assert_eq!(toc.iter().map(|entry| entry.sentence_id.0).collect::<Vec<_>>(), [0, 1, 3, 4]);
    }

    #[test]
    fn entries_pointing_past_the_last_page_are_skipped() {
        let outline = [item("1: Kubernetes primer", 1, 11), item("2: Principles of operation", 1, 18), item("Afterword", 1, 99)];

        assert_eq!(build_toc(&outline, &book()).len(), 2);
    }

    #[test]
    fn unmatched_entries_of_a_long_book_are_found_quickly() {
        const PAGES: u32 = 3_000;
        const SENTENCES_PER_PAGE: u32 = 30;
        // Scanning from the first sentence for every entry took 2.3 s here (debug build); from the entry before, 0.1 s.
        const BUDGET: std::time::Duration = std::time::Duration::from_secs(1);
        let names: Vec<String> = (0..PAGES).map(|page| format!("Section {page}")).collect();
        let mut specs = Vec::new();
        for page in 0..PAGES {
            specs.push(heading(1, &names[page as usize], page));
            specs.extend((1..SENTENCES_PER_PAGE).map(|_| paragraph("Text.", page)));
        }
        // Every other entry has no heading of its own, so it is placed by its page.
        let outline: Vec<OutlineItem> = (0..PAGES).map(|page| item(if page % 2 == 0 { &names[page as usize] } else { "Notes" }, 1, page)).collect();
        let started = std::time::Instant::now();

        let toc = build_toc(&outline, &blocks(&specs));

        assert_eq!(toc.len(), PAGES as usize);
        assert!(started.elapsed() < BUDGET, "{:?}", started.elapsed());
    }

    #[test]
    fn without_an_outline_the_spoken_headings_are_used() {
        let mut blocks = book();
        blocks.push(Block {
            id: 8,
            kind: BlockKind::Heading,
            level: Some(1),
            page_index: 20,
            spoken: true,
            text: None,
            sentences: vec![Sentence { id: SentenceId(8), text: "[GoF95]".into(), page_index: 20, speech_text: Some(String::new()) }],
            cue: None,
        });

        let toc = build_toc(&[], &blocks);

        assert_eq!(
            lines(&toc),
            [
                ("1: Kubernetes primer".into(), 1, 1, 11),
                ("Important Kubernetes background".into(), 2, 3, 12),
                ("Orchestration".into(), 3, 5, 13),
                ("2: Principles of operation".into(), 1, 7, 18)
            ]
        );
    }

    #[test]
    fn an_outline_that_mostly_misses_the_headings_is_replaced_by_them() {
        let outline = [item("Cover", 1, 1), item("Part A", 1, 11), item("Part B", 1, 12), item("Part C", 1, 14)];

        let titles: Vec<String> = build_toc(&outline, &book()).into_iter().map(|entry| entry.title).collect();

        assert_eq!(titles[0], "1: Kubernetes primer");
        assert_eq!(titles.len(), 4);
    }

    #[test]
    fn a_book_with_no_headings_and_no_outline_has_an_empty_toc() {
        assert!(build_toc(&[], &blocks(&[paragraph("Just text.", 1)])).is_empty());
    }
}
