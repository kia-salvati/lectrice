//! B2: the pure PDF cleaning rules (`PdfContent` -> `CleanedDocument`) and the `DocumentCleaner`
//! implementation. Rule numbers refer to `research/2026-10-21-pdf-survey.md` §4.

mod assemble;
mod bands;
mod block_text;
mod code;
mod fonts;
mod footnotes;
mod headings;
mod hyphenation;
mod junk;
mod line_kinds;
mod lines;
mod normalize;
mod page;
mod page_labels;
mod page_roles;
mod profile;
mod speech_filters;
mod tables;
mod toc;

pub use toc::build_toc;

use std::collections::HashMap;

use assemble::{Assembler, PageContent, PlacedLine};
use bands::mark_running_headers;
use fonts::FontStyle;
use footnotes::{Footnote, take_footnotes};
use hyphenation::Vocabulary;
use junk::mark_junk_lines;
use line_kinds::{Column, LineKind, classify_line, demote_continuations, demote_lone_inline_code};
use lines::{Line, build_lines};
use page::{Page, PageRole};
use page_labels::{page_labels, resolve_labels};
use page_roles::classify_pages;
use profile::Profile;
use tables::mark_table_rows;

use crate::cleaned::CleanedDocument;
use crate::error::{AppError, AppResult, ErrorName};
use crate::pdf::{PdfContent, TextRun};
use crate::ports::DocumentCleaner;

/// Raise this whenever a rule changes what a cleaned document looks like.
pub const CLEANER_VERSION: u32 = 3;

/// The survey's v1 pipeline (rules 1–20).
#[derive(Debug, Default, Clone, Copy)]
pub struct RuleBasedCleaner;

impl DocumentCleaner for RuleBasedCleaner {
    fn version(&self) -> u32 {
        CLEANER_VERSION
    }

    fn clean(&self, content: PdfContent) -> AppResult<CleanedDocument> {
        let cleaned = clean_content(&content);
        if cleaned.blocks.is_empty() {
            return Err(AppError::new(ErrorName::PdfNoText).with_detail(format!("no blocks left after cleaning {} pages", cleaned.page_count)));
        }
        Ok(cleaned)
    }
}

pub fn clean_content(content: &PdfContent) -> CleanedDocument {
    let mut pages = build_pages(content);
    let labels = resolve_labels(&pages);
    mark_running_headers(&mut pages, &labels);
    pages.iter_mut().for_each(|page| mark_junk_lines(&mut page.lines));
    classify_pages(&mut pages, &content.outline);
    let kept_pages: Vec<&Page> = pages.iter().filter(|page| !page.role.is_dropped()).collect();
    let profile = Profile::from_pages(&kept_pages);
    let footnotes: Vec<Vec<Footnote>> =
        pages.iter_mut().map(|page| if page.role == PageRole::Body { take_footnotes(page, &profile) } else { Vec::new() }).collect();
    let vocabulary = Vocabulary::from_lines(pages.iter().flat_map(Page::content_lines));

    let contents: Vec<PageContent> =
        pages.iter().zip(footnotes).filter(|(page, _)| !page.role.is_dropped()).map(|(page, footnotes)| page_content(page, footnotes, &profile)).collect();
    let blocks = Assembler::new(&profile, &vocabulary, &content.outline).assemble(contents);

    CleanedDocument {
        title: content.title.clone(),
        page_count: u32::try_from(pages.len()).unwrap_or(u32::MAX),
        pages: page_labels(&pages, &labels),
        outline: content.outline.clone(),
        blocks,
    }
}

fn build_pages(content: &PdfContent) -> Vec<Page> {
    let mut runs_by_page: HashMap<u32, Vec<&TextRun>> = HashMap::new();
    for run in &content.runs {
        runs_by_page.entry(run.pdf_page).or_default().push(run);
    }
    content
        .pages
        .iter()
        .map(|info| Page {
            index: info.index,
            width: info.width,
            height: info.height,
            embedded_label: info.label.clone(),
            lines: runs_by_page.get(&info.index).map(|runs| build_lines(info.index, runs)).unwrap_or_default(),
            role: PageRole::Body,
        })
        .collect()
}

/// A prose line set mostly in inline code (`run tool build and tool check commands.`) takes the size of
/// its prose, so paragraph joining sees one size across the paragraph.
fn prose_sized(line: &Line, kind: LineKind, profile: &Profile) -> Line {
    let mut line = line.clone();
    if kind != LineKind::Code && profile.is_inline_code_style(&line) {
        let prose_sizes = line.pieces.iter().filter(|piece| !piece.superscript && !FontStyle::parse(&piece.font).monospace).map(|piece| piece.size);
        line.size = prose_sizes.reduce(f32::max).unwrap_or(profile.body_size);
    }
    line
}

fn page_content(page: &Page, footnotes: Vec<Footnote>, profile: &Profile) -> PageContent {
    let lines: Vec<&Line> = page.content_lines().collect();
    let column = Column::of(lines.iter().copied(), profile);
    let mut kinds: Vec<LineKind> = lines.iter().map(|line| classify_line(line, profile, column)).collect();
    demote_lone_inline_code(&lines, &mut kinds, profile);
    demote_continuations(&lines, &mut kinds, profile);
    mark_table_rows(&lines, &mut kinds);
    let placed = lines.into_iter().zip(kinds).map(|(line, kind)| PlacedLine::new(prose_sized(line, kind, profile), kind, column)).collect();
    PageContent { silent: page.role.is_silent(), lines: placed, footnotes }
}
