//! A block's shown text with, per byte, the page it comes from and whether it is spoken.
//! Cut into [`TextSpan`]s at every page or spoken/silent change.

use std::ops::Range;

use super::speech_filters::merge;
use crate::cleaned::TextSpan;

#[derive(Debug, Default)]
pub struct BlockText {
    text: String,
    /// `(start byte, page)`, sorted by start.
    page_starts: Vec<(usize, u32)>,
    silent: Vec<Range<usize>>,
}

impl BlockText {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn push(&mut self, text: &str, page: u32) {
        if self.page_starts.last().is_none_or(|(_, last_page)| *last_page != page) {
            self.page_starts.push((self.text.len(), page));
        }
        self.text.push_str(text);
    }

    pub fn push_silent(&mut self, text: &str, page: u32) {
        let start = self.text.len();
        self.push(text, page);
        self.silent.push(start..self.text.len());
    }

    /// Drops the last char (a line-end hyphen that turned out to be a syllable break).
    pub fn pop_char(&mut self) {
        self.text.pop();
    }

    pub fn ends_with_space(&self) -> bool {
        self.text.ends_with(' ')
    }

    pub fn silence(&mut self, ranges: impl IntoIterator<Item = Range<usize>>) {
        self.silent.extend(ranges);
    }

    pub fn into_spans(self, all_silent: bool) -> Vec<TextSpan> {
        let silent = merge(self.silent);
        let mut cuts: Vec<usize> = self.page_starts.iter().map(|(start, _)| *start).chain(silent.iter().flat_map(|range| [range.start, range.end])).collect();
        cuts.push(self.text.len());
        cuts.sort_unstable();
        cuts.dedup();

        let mut spans: Vec<TextSpan> = Vec::new();
        for window in cuts.windows(2) {
            let range = window[0]..window[1];
            if range.is_empty() {
                continue;
            }
            let page_index = self.page_starts.iter().rev().find(|(start, _)| *start <= range.start).map_or(0, |(_, page)| *page);
            let spoken = !all_silent && !silent.iter().any(|silent_range| silent_range.start <= range.start && range.end <= silent_range.end);
            let text = &self.text[range];
            match spans.last_mut() {
                Some(last) if last.page_index == page_index && last.spoken == spoken => last.text.push_str(text),
                _ => spans.push(TextSpan { text: text.to_string(), page_index, spoken }),
            }
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, page_index: u32, spoken: bool) -> TextSpan {
        TextSpan { text: text.into(), page_index, spoken }
    }

    #[test]
    fn spans_split_at_page_and_silence_changes() {
        let mut block = BlockText::default();
        block.push("a box in the garden,", 23);
        block.push_silent("¹", 23);
        block.push(" but it", 23);
        block.push(" continues", 24);
        assert_eq!(
            block.into_spans(false),
            [span("a box in the garden,", 23, true), span("¹", 23, false), span(" but it", 23, true), span(" continues", 24, true)]
        );
    }

    #[test]
    fn silenced_ranges_and_silent_blocks() {
        let mut block = BlockText::default();
        block.push("see [12] here", 5);
        block.silence(std::iter::once(3..8));
        assert_eq!(block.into_spans(false), [span("see", 5, true), span(" [12]", 5, false), span(" here", 5, true)]);
        let mut silent_block = BlockText::default();
        silent_block.push("ISBN 978", 4);
        assert_eq!(silent_block.into_spans(true), [span("ISBN 978", 4, false)]);
    }
}
