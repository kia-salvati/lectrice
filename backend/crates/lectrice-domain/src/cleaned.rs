use crate::document::{BlockKind, PageLabel};

/// The cleaner's output: blocks of text without sentence ids yet.
/// Import splits each block's text into sentences and numbers them into a [`crate::Document`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CleanedDocument {
    pub title: Option<String>,
    pub page_count: u32,
    pub pages: Vec<PageLabel>,
    pub blocks: Vec<CleanedBlock>,
}

/// The block's shown text is its spans joined in order. `page_index` is the page it starts on.
#[derive(Debug, Clone, PartialEq)]
pub struct CleanedBlock {
    pub kind: BlockKind,
    pub level: Option<u8>,
    pub page_index: u32,
    pub spans: Vec<TextSpan>,
}

/// A stretch of shown text on one page. `spoken: false` marks text that is shown but skipped
/// in speech (citation markers, URLs, emoji).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    pub text: String,
    pub page_index: u32,
    pub spoken: bool,
}
