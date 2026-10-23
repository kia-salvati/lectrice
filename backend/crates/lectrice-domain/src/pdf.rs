use serde::{Deserialize, Serialize};

/// Everything the cleaner needs from a PDF, with no PDF library types.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfContent {
    pub title: Option<String>,
    pub pages: Vec<PdfPage>,
    pub runs: Vec<TextRun>,
    pub outline: Vec<OutlineItem>,
}

/// PDF page size in points. `label` is the embedded page label, if the PDF has one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPage {
    pub index: u32,
    pub width: f32,
    pub height: f32,
    pub label: Option<String>,
}

/// A piece of text in one font and size on one baseline, in PDFium character order.
/// `font_size` is the effective (scaled) size. `line_end_hyphen` marks a run whose last char is a
/// hyphen PDFium found at a line end (PDFium removes the line break after it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub pdf_page: u32,
    pub text: String,
    pub font_name: String,
    pub font_size: f32,
    pub bbox: BoundingBox,
    pub baseline: f32,
    pub line_end_hyphen: bool,
}

/// PDF coordinates in points: the origin is the bottom-left corner of the page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

/// One bookmark. `level` starts at 1 for top-level items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem {
    pub title: String,
    pub level: u8,
    pub pdf_page: Option<u32>,
}
