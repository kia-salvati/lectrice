use super::lines::Line;

/// Rule 6 and 7: what a whole page is. Dropped pages produce no blocks; silent pages are shown but not spoken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRole {
    Body,
    Empty,
    TableOfContents,
    Index,
    Bibliography,
    Copyright,
}

impl PageRole {
    pub const fn is_dropped(self) -> bool {
        matches!(self, Self::Empty | Self::TableOfContents | Self::Index)
    }

    pub const fn is_silent(self) -> bool {
        matches!(self, Self::Bibliography | Self::Copyright)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub index: u32,
    pub width: f32,
    pub height: f32,
    pub embedded_label: Option<String>,
    pub lines: Vec<Line>,
    pub role: PageRole,
}

impl Page {
    pub fn content_lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter().filter(|line| line.is_content())
    }

    pub fn content_char_count(&self) -> usize {
        self.content_lines().map(Line::char_count).sum()
    }
}
