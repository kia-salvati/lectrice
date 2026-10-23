use std::collections::HashSet;
use std::path::Path;

use lectrice_domain::{AppError, AppResult, BoundingBox, ErrorName, OutlineItem, PdfContent, PdfPage as PageInfo, PdfTextSource, TextRun};
use pdfium_render::prelude::*;

use super::run_builder::{CharInfo, RunBuilder, strip_subset_prefix};

const TOP_OUTLINE_LEVEL: u8 = 1;
/// Limits what a huge or broken bookmark tree can cost.
const MAX_OUTLINE_ITEMS: usize = 20_000;
/// Deeper bookmarks are dropped; the cleaner caps heading levels at 4 anyway.
const MAX_OUTLINE_DEPTH: u8 = 16;

/// Reads text runs, page labels and the outline with PDFium (pdfium-render).
pub struct PdfiumTextSource {
    pdfium: Pdfium,
}

impl PdfiumTextSource {
    /// Binds `libpdfium.so` at `library_path`. Fails with `Internal` when the library can't be loaded.
    pub fn new(library_path: &Path) -> AppResult<Self> {
        let bindings = Pdfium::bind_to_library(library_path).map_err(|error| {
            AppError::new(ErrorName::Internal).with_detail(format!("cannot load pdfium from {}", library_path.display())).with_source(error)
        })?;
        Ok(Self { pdfium: Pdfium::new(bindings) })
    }
}

impl PdfTextSource for PdfiumTextSource {
    fn read_runs(&self, pdf: &[u8]) -> AppResult<PdfContent> {
        let document = self.pdfium.load_pdf_from_byte_slice(pdf, None).map_err(load_error)?;
        let mut pages = Vec::new();
        let mut runs = Vec::new();
        for (zero_based, page) in document.pages().iter().enumerate() {
            let index = page_number(zero_based);
            runs.extend(read_page_runs(&page, index)?);
            pages.push(PageInfo {
                index,
                width: page.width().value,
                height: page.height().value,
                label: page.label().map(str::trim).filter(|label| !label.is_empty()).map(String::from),
            });
        }
        if runs.is_empty() {
            return Err(AppError::new(ErrorName::PdfNoText).with_detail(format!("no text runs on {} pages", pages.len())));
        }
        let outline = read_outline(&document);
        let title = document.metadata().get(PdfDocumentMetadataTagType::Title).map(|tag| tag.value().trim().to_string()).filter(|title| !title.is_empty());
        Ok(PdfContent { title, pages, runs, outline })
    }
}

fn load_error(error: PdfiumError) -> AppError {
    let name = match error {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError) => ErrorName::PdfEncrypted,
        _ => ErrorName::PdfInvalid,
    };
    AppError::new(name).with_detail("pdfium could not open the document").with_source(error)
}

fn page_number(zero_based: usize) -> u32 {
    u32::try_from(zero_based + 1).unwrap_or(u32::MAX)
}

fn read_page_runs(page: &PdfPage<'_>, index: u32) -> AppResult<Vec<TextRun>> {
    let text =
        page.text().map_err(|error| AppError::new(ErrorName::PdfInvalid).with_detail(format!("cannot read the text of page {index}")).with_source(error))?;
    let mut builder = RunBuilder::new(index);
    for character in text.chars().iter() {
        if let Some(char_info) = char_info(&character) {
            builder.push(char_info);
        }
    }
    Ok(builder.finish())
}

fn char_info(character: &PdfPageTextChar<'_>) -> Option<CharInfo> {
    let bounds = character.loose_bounds().or_else(|_| character.tight_bounds()).ok()?;
    Some(CharInfo {
        unicode: character.unicode_value(),
        generated: character.is_generated().unwrap_or(false),
        hyphen: character.is_hyphen().unwrap_or(false),
        font_name: strip_subset_prefix(&character.font_name()).to_string(),
        font_size: character.scaled_font_size().value,
        bounds: BoundingBox { left: bounds.left().value, bottom: bounds.bottom().value, right: bounds.right().value, top: bounds.top().value },
        baseline: character.origin_y().map_or(bounds.bottom().value, |origin| origin.value),
    })
}

/// Walks the bookmark tree depth first without recursion, so a deep or cyclic tree in a broken PDF
/// can't overflow the stack. Bookmarks already seen are skipped (cycles), children below
/// `MAX_OUTLINE_DEPTH` are dropped, and at most `MAX_OUTLINE_ITEMS` are kept.
fn read_outline(document: &PdfDocument<'_>) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut pending: Vec<(PdfBookmark<'_>, u8)> = document.bookmarks().root().map(|root| (root, TOP_OUTLINE_LEVEL)).into_iter().collect();
    while let Some((bookmark, level)) = pending.pop() {
        if items.len() >= MAX_OUTLINE_ITEMS {
            break;
        }
        if !seen.insert(bookmark.clone()) {
            continue;
        }
        items.push(outline_item(&bookmark, level));
        if let Some(sibling) = bookmark.next_sibling() {
            pending.push((sibling, level));
        }
        if level < MAX_OUTLINE_DEPTH
            && let Some(child) = bookmark.first_child()
        {
            pending.push((child, level + 1));
        }
    }
    items
}

fn outline_item(bookmark: &PdfBookmark<'_>, level: u8) -> OutlineItem {
    let title = bookmark.title().map(|title| title.trim().to_string()).unwrap_or_default();
    let pdf_page =
        bookmark.destination().and_then(|destination| destination.page_index().ok()).and_then(|zero_based| usize::try_from(zero_based).ok()).map(page_number);
    OutlineItem { title, level, pdf_page }
}
