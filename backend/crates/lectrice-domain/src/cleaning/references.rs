//! Reference lists inside body pages: a `References` / `Bibliography` / `Works cited` heading whose section opens
//! with numbered entries (`[1] …`, `1. …`) is shown but not spoken, up to the next heading of the same or a higher
//! level. Whole bibliography pages are caught earlier by the page roles.

use std::sync::LazyLock;

use regex::Regex;

use super::page::PageRole;
use super::page_roles::role_for_title;
use crate::cleaned::CleanedBlock;
use crate::document::BlockKind;

static NUMBERED_ENTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\[\d{1,4}\]|\d{1,4}\.)\s").expect("valid regex"));

pub fn silence_reference_lists(blocks: &mut [CleanedBlock]) {
    let mut index = 0;
    while index < blocks.len() {
        let Some(level) = reference_heading_level(&blocks[index]) else {
            index += 1;
            continue;
        };
        let section_end = blocks[index + 1..]
            .iter()
            .position(|block| block.kind == BlockKind::Heading && block.level.is_some_and(|next_level| next_level <= level))
            .map_or(blocks.len(), |offset| index + 1 + offset);
        let section = &mut blocks[index..section_end];
        let first_entry = section.iter().skip(1).find(|block| block.kind != BlockKind::Heading);
        if first_entry.is_some_and(|block| NUMBERED_ENTRY.is_match(shown_text(block).trim_start())) {
            section.iter_mut().flat_map(|block| block.spans.iter_mut()).for_each(|span| span.spoken = false);
            index = section_end;
        } else {
            index += 1;
        }
    }
}

fn reference_heading_level(block: &CleanedBlock) -> Option<u8> {
    let is_reference_heading = block.kind == BlockKind::Heading && role_for_title(&shown_text(block)) == Some(PageRole::Bibliography);
    block.level.filter(|_| is_reference_heading)
}

fn shown_text(block: &CleanedBlock) -> String {
    block.spans.iter().map(|span| span.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaned::TextSpan;

    fn block(kind: BlockKind, level: Option<u8>, text: &str) -> CleanedBlock {
        CleanedBlock { kind, level, page_index: 1, spans: vec![TextSpan { text: text.into(), page_index: 1, spoken: true }] }
    }

    fn spoken(blocks: &[CleanedBlock]) -> Vec<bool> {
        blocks.iter().map(|block| block.spans.iter().any(|span| span.spoken)).collect()
    }

    #[test]
    fn a_numbered_reference_list_is_silent_up_to_the_next_heading_of_its_level() {
        let mut blocks = vec![
            block(BlockKind::Heading, Some(1), "References"),
            block(BlockKind::Paragraph, None, "[1] Ada Inventa. A Made-Up Book. Nowhere Press, 1950."),
            block(BlockKind::Heading, Some(2), "Further reading"),
            block(BlockKind::ListItem, None, "2. Bo Fictio. Another Title. 1951."),
            block(BlockKind::Heading, Some(1), "Appendix A"),
            block(BlockKind::Paragraph, None, "Back to the story."),
        ];
        silence_reference_lists(&mut blocks);
        assert_eq!(spoken(&blocks), [false, false, false, false, true, true]);
    }

    #[test]
    fn a_references_heading_over_prose_stays_spoken() {
        let mut blocks = vec![block(BlockKind::Heading, Some(2), "7 Bibliography"), block(BlockKind::Paragraph, None, "This chapter lists the sources.")];
        silence_reference_lists(&mut blocks);
        assert_eq!(spoken(&blocks), [true, true]);
        let mut other = vec![block(BlockKind::Heading, Some(1), "Results"), block(BlockKind::Paragraph, None, "[1] is spoken here.")];
        silence_reference_lists(&mut other);
        assert_eq!(spoken(&other), [true, true]);
    }
}
