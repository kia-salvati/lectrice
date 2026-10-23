//! The book fixture rules again, on invented text. The pages copy the layout of the real books (page size,
//! fonts, sizes, positions, line pitch), but every word is made up, so these tests run on a public clone
//! without the local book data.

mod cleaning_support;

use cleaning_support::{block, blocks_on, has_text, is_silent, kinds_on, label, shown, spoken};
use lectrice_domain::cleaning::clean_content;
use lectrice_domain::{BlockKind, BoundingBox, CleanedDocument, OutlineItem, PdfContent, PdfPage, TextRun};

/// A rough glyph width as a share of the font size, enough to place the next run on a line.
const GLYPH_WIDTH: f32 = 0.47;
/// Monospace glyphs are exactly this wide, so code indents can be rebuilt from x.
const MONOSPACE_GLYPH_WIDTH: f32 = 0.6;

/// The font family, sizes and positions of one book layout.
struct Layout {
    width: f32,
    height: f32,
    left: f32,
    top: f32,
    body_font: &'static str,
    body_size: f32,
    pitch: f32,
    paragraph_gap: f32,
}

/// The Kubernetes Book: XeTeX, 504 × 655 pt, Crimson body, Open Sans headings, Source Code Pro code.
const XETEX: Layout =
    Layout { width: 504.0, height: 655.2, left: 54.0, top: 572.08, body_font: "CrimsonText-Roman", body_size: 9.96, pitch: 11.95, paragraph_gap: 5.9 };
/// Design Patterns: Letter pages, everything in Courier New, a bold page number in the footer.
const COURIER: Layout =
    Layout { width: 612.0, height: 792.0, left: 111.48, top: 671.04, body_font: "CourierNew", body_size: 8.4, pitch: 14.64, paragraph_gap: 13.08 };
/// Clean Architecture: a calibre e-book, Letter pages, Times body, no running headers.
const CALIBRE: Layout =
    Layout { width: 612.0, height: 792.0, left: 50.0, top: 727.5, body_font: "TimesNewRomanPSMT", body_size: 15.0, pitch: 17.25, paragraph_gap: 15.0 };
/// LinkedIn Posts: Google Docs export, Letter pages, Arial.
const GOOGLE_DOCS: Layout =
    Layout { width: 612.0, height: 792.0, left: 72.0, top: 704.99, body_font: "ArialMT", body_size: 10.5, pitch: 12.07, paragraph_gap: 10.0 };

struct Book {
    content: PdfContent,
}

impl Book {
    fn new() -> Self {
        Self { content: PdfContent { title: Some("An Invented Book".into()), pages: Vec::new(), runs: Vec::new(), outline: Vec::new() } }
    }

    fn outline(mut self, items: &[(&str, u8, u32)]) -> Self {
        self.content.outline = items.iter().map(|(title, level, page)| OutlineItem { title: (*title).into(), level: *level, pdf_page: Some(*page) }).collect();
        self
    }

    fn page<'a>(&'a mut self, layout: &'a Layout, label: Option<&str>) -> PageWriter<'a> {
        let index = u32::try_from(self.content.pages.len()).unwrap();
        self.content.pages.push(PdfPage { index, width: layout.width, height: layout.height, label: label.map(str::to_string) });
        PageWriter { book: self, layout, index, baseline: layout.top }
    }

    fn clean(&self) -> CleanedDocument {
        clean_content(&self.content)
    }
}

struct PageWriter<'a> {
    book: &'a mut Book,
    layout: &'a Layout,
    index: u32,
    baseline: f32,
}

impl PageWriter<'_> {
    /// One run at an exact place; returns its right edge.
    fn run_at(&mut self, left: f32, baseline: f32, size: f32, font: &str, text: &str, hyphen: bool) -> f32 {
        let monospace = ["Code", "Courier", "Mono"].iter().any(|name| font.contains(name));
        let glyph_width = if monospace { MONOSPACE_GLYPH_WIDTH } else { GLYPH_WIDTH };
        let right = left + text.chars().count() as f32 * size * glyph_width;
        let bbox = BoundingBox { left, bottom: baseline - size * 0.25, right, top: baseline + size * 0.8 };
        self.book.content.runs.push(TextRun {
            pdf_page: self.index,
            text: text.into(),
            font_name: font.into(),
            font_size: size,
            bbox,
            baseline,
            line_end_hyphen: hyphen,
        });
        right
    }

    /// Runs set one after another on the current line, then a line break.
    fn pieces(&mut self, left: f32, pieces: &[(&str, f32, &str)]) -> &mut Self {
        let mut x = left;
        for (font, size, text) in pieces {
            x = self.run_at(x, self.baseline, *size, font, text, false) + 2.4;
        }
        self.baseline -= self.layout.pitch;
        self
    }

    fn line(&mut self, text: &str) -> &mut Self {
        let (left, size, font) = (self.layout.left, self.layout.body_size, self.layout.body_font);
        self.pieces(left, &[(font, size, text)])
    }

    /// A body line whose last word is cut with a hyphen at the line end.
    fn hyphen_line(&mut self, text: &str) -> &mut Self {
        let (left, size, font) = (self.layout.left, self.layout.body_size, self.layout.body_font);
        self.run_at(left, self.baseline, size, font, text, true);
        self.baseline -= self.layout.pitch;
        self
    }

    fn lines(&mut self, lines: &[&str]) -> &mut Self {
        lines.iter().fold(self, |page, text| page.line(text)).gap()
    }

    fn indented(&mut self, indent: f32, text: &str) -> &mut Self {
        let (size, font) = (self.layout.body_size, self.layout.body_font);
        self.pieces(self.layout.left + indent, &[(font, size, text)])
    }

    fn gap(&mut self) -> &mut Self {
        self.baseline -= self.layout.paragraph_gap;
        self
    }

    fn heading(&mut self, font: &str, size: f32, text: &str) -> &mut Self {
        self.baseline -= self.layout.pitch;
        self.pieces(self.layout.left, &[(font, size, text)]);
        self.baseline -= self.layout.pitch;
        self
    }

    /// A code listing: `(indent in spaces, text)` lines in a smaller monospace font.
    fn code(&mut self, font: &str, size: f32, pitch: f32, lines: &[(usize, &str)]) -> &mut Self {
        for (indent, text) in lines {
            let left = self.layout.left + *indent as f32 * size * 0.6;
            self.run_at(left, self.baseline, size, font, text, false);
            self.baseline -= pitch;
        }
        self.gap().gap()
    }

    fn at(&mut self, baseline: f32) -> &mut Self {
        self.baseline = baseline;
        self
    }
}

const BOLD_HEADING: &str = "OpenSans-Bold";
const XETEX_CODE: &str = "SourceCodePro-Regular";
const XETEX_INLINE_CODE: &str = "SourceCodePro-Bold";
const XETEX_CODE_SIZE: f32 = 6.97;
const XETEX_CODE_PITCH: f32 = 10.04;
const XETEX_ITALIC: &str = "CrimsonText-Italic";
const XETEX_BOLD: &str = "CrimsonText-Bold";

/// A running header on the left and the page number on the right, on one baseline (K8s pages 12 and 60).
fn xetex_header(page: &mut PageWriter<'_>, chapter: &str, number: &str) {
    page.run_at(53.78, 603.63, 7.97, "CrimsonText-Roman", chapter, false);
    page.run_at(444.39, 603.63, 9.96, "CrimsonText-Roman", number, false);
}

/// Six pages in the Kubernetes Book layout: cover, contents, three chapter pages, index.
fn xetex_book() -> Book {
    let mut book = Book::new().outline(&[
        ("Contents", 1, 1),
        ("3: Garden basics", 1, 2),
        ("Raised beds", 2, 2),
        ("Watering", 2, 2),
        ("4: Working with Tools", 1, 3),
        ("tool exec", 2, 3),
        ("5: Plans and costs", 1, 4),
        ("6: Paths", 1, 5),
        ("Index", 1, 6),
    ]);
    book.page(&XETEX, None);

    let mut contents = book.page(&XETEX, Some("v"));
    contents.heading(BOLD_HEADING, 20.66, "Contents");
    for entry in ["3: Garden basics 1", "Raised beds 1", "Watering 2", "4: Working with Tools 3", "tool exec 4", "5: Plans and costs 5", "Index 9"] {
        contents.line(entry);
    }

    let mut basics = book.page(&XETEX, Some("4"));
    xetex_header(&mut basics, "3: Garden basics", "4");
    basics.lines(&[
        "A raised bed is a wooden box of soil that sits on the ground. Flat rows and raised beds",
        "both grow food, yet raised beds warm up sooner in spring and drain better after rain.",
        "The soil temperature in a bed rises early in the year.",
    ]);
    basics
        .line("Fortunately, a garden bed does not need much care, and many people build one of their")
        .hyphen_line("own in a weekend. For a small yard a raised bed is the first-")
        .line("rate pick, and a cheap one.")
        .gap();
    basics.heading(BOLD_HEADING, 11.96, "Raised beds");
    basics.lines(&["A good bed needs the following four parts:"]);
    for item in ["• Wooden frame", "• Soil mix", "• Drip line", "• Mulch"] {
        basics.indented(15.7, item);
    }
    basics.gap().heading(BOLD_HEADING, 11.96, "Watering");
    basics.pieces(54.0, &[(XETEX_ITALIC, 9.96, "Most of the water comes from the"), ("CrimsonText-Roman", 9.96, "Garden Water Alliance (GWA)")]);
    let marker_line = basics.baseline + XETEX.pitch;
    basics.run_at(330.0, marker_line + 3.62, 6.97, "CrimsonText-Roman", "1", false);
    basics.run_at(334.0, marker_line, 9.96, "CrimsonText-Roman", ".", false);
    basics.hyphen_line("Each bed has a small meter that measures the soil and its tempera-");
    basics.line("ture, so the water only runs when the plants need it.").gap();
    basics.run_at(68.07, 67.61, 5.98, "CrimsonText-Roman", "1", false);
    basics.run_at(71.93, 64.8, 7.97, "CrimsonText-Roman", "https://www.example.org/water", false);

    let mut tools = book.page(&XETEX, Some("52"));
    xetex_header(&mut tools, "4: Working with Tools", "52");
    tools.lines(&["The following file describes a small garden with two beds and a timer."]);
    tools.code(
        XETEX_CODE,
        XETEX_CODE_SIZE,
        XETEX_CODE_PITCH,
        &[(0, "kind: Garden"), (0, "version: v1"), (0, "metadata:"), (2, "name: demo"), (0, "beds:"), (2, "- name: north"), (4, "size: 12"), (0, "<Snip>")],
    );
    tools.lines(&["Wait before you start it: the garden is not built yet."]);
    tools.code(XETEX_CODE, XETEX_CODE_SIZE, XETEX_CODE_PITCH, &[(0, "$ tool logs demo --bed north")]);
    tools.heading(BOLD_HEADING, 11.96, "tool exec");
    tools.pieces(
        53.65,
        &[
            ("CrimsonText-Roman", 9.96, "The"),
            (XETEX_INLINE_CODE, 8.97, "tool exec"),
            ("CrimsonText-Roman", 9.96, "command runs a task inside the garden shed"),
        ],
    );
    tools.lines(&["while the timer is still running."]);
    tools.lines(&["Figure 4.2 shows the whole setup from above, with the north bed on the left."]);
    tools.lines(&["Visit http://localhost:9000/stats to see the meter readings."]);

    let mut plans = book.page(&XETEX, Some("220"));
    xetex_header(&mut plans, "5: Plans and costs", "220");
    let rows: [(&str, f32, [&str; 3]); 4] = [
        (XETEX_BOLD, 574.37, ["Season", "Task", "Common cost"]),
        ("CrimsonText-Roman", 562.96, ["spring", "plant", "20 seeds, 3 hours"]),
        ("CrimsonText-Roman", 551.94, ["summer", "water", "200 liters, 1 hour"]),
        ("CrimsonText-Roman", 540.92, ["autumn", "harvest", "10 baskets, 4 hours"]),
    ];
    for (font, baseline, cells) in rows {
        for (left, cell) in [99.58, 173.64, 237.44].into_iter().zip(cells) {
            plans.run_at(left, baseline, 8.97, font, cell, false);
        }
    }
    plans.at(508.74).lines(&["Every season has one main task and a cost in seeds, water or time."]);
    plans.lines(&["Figure 5.1 Path-based watering"]);
    plans.lines(&["The plan is cheap to run, and most of the cost is the time spent in the garden."]);

    let mut paths = book.page(&XETEX, Some("221"));
    xetex_header(&mut paths, "6: Paths", "221");
    paths.line("The path ends at a small shed named north-shed that listens on port");
    let code_end = paths.run_at(54.0, paths.baseline, 8.97, XETEX_INLINE_CODE, "8080", false);
    paths.run_at(code_end, paths.baseline, 9.96, "CrimsonText-Roman", ".", false);
    paths.at(paths.baseline - XETEX.pitch).gap();
    paths.line("Next, build a gate called front-gate and a second, smaller gate called");
    paths.pieces(54.0, &[(XETEX_BOLD, 9.96, "side-gate")]).gap();
    paths.lines(&["The last rule sends every visitor arriving on shop.acme.com"]);
    paths.lines(&["Visitors are counted at the gate, so the list stays short."]);

    let mut index = book.page(&XETEX, Some("300"));
    index.heading(BOLD_HEADING, 20.66, "Index");
    for entry in ["beds, 12", "costs, 220", "frames, 14-16", "mulch, 30", "watering, 7", "see also timers"] {
        index.line(entry);
    }
    book
}

#[test]
fn xetex_header_and_page_number_dropped_and_embedded_label_kept() {
    let document = xetex_book().clean();
    assert!(!has_text(&document, "Garden basics 4"));
    assert!(!has_text(&document, "3: Garden basics"));
    assert_eq!(label(&document, 2), Some("4"));
    assert!(blocks_on(&document, 2).iter().all(|block| shown(block).trim() != "4"));
}

#[test]
fn xetex_cover_toc_and_index_pages_have_no_blocks() {
    let document = xetex_book().clean();
    for page in [0, 1, 6] {
        assert!(blocks_on(&document, page).is_empty(), "page {page}: {:?}", blocks_on(&document, page).iter().map(|block| shown(block)).collect::<Vec<_>>());
    }
}

#[test]
fn xetex_real_compound_keeps_its_hyphen_and_syllable_break_is_joined() {
    let document = xetex_book().clean();
    assert!(has_text(&document, "the first-rate pick"));
    assert!(has_text(&document, "its temperature, so"));
    assert!(!has_text(&document, "tempera-"));
}

#[test]
fn xetex_headings_from_the_outline_and_bullets_stripped() {
    let document = xetex_book().clean();
    let headings = kinds_on(&document, 2, BlockKind::Heading);
    assert!(headings.contains(&"Raised beds".to_string()) && headings.contains(&"Watering".to_string()), "{headings:?}");
    let items = kinds_on(&document, 2, BlockKind::ListItem);
    assert_eq!(items, ["Wooden frame", "Soil mix", "Drip line", "Mulch"]);
}

#[test]
fn xetex_footnote_marker_is_silent_and_the_footnote_is_shown_only() {
    let document = xetex_book().clean();
    let paragraph = block(&document, "Garden Water Alliance (GWA)");
    assert!(spoken(paragraph).contains("Alliance (GWA)."), "{:?}", spoken(paragraph));
    assert!(shown(paragraph).contains("(GWA)¹."), "{:?}", shown(paragraph));
    let footnote = block(&document, "https://www.example.org/water");
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
}

#[test]
fn xetex_yaml_is_one_code_block_with_rebuilt_indent() {
    let document = xetex_book().clean();
    let yaml = block(&document, "kind: Garden");
    assert_eq!(yaml.kind, BlockKind::Code);
    let text = shown(yaml);
    assert!(text.contains("metadata:\n  name: demo"), "{text:?}");
    assert!(text.contains("\n    size: 12"), "{text:?}");
    assert!(text.ends_with("<Snip>"));
    assert_eq!(block(&document, "$ tool logs demo").kind, BlockKind::Code);
    assert_eq!(BlockKind::Code.cue_text(), Some("Code example."));
}

#[test]
fn xetex_inline_code_stays_in_its_paragraph_and_heading_in_code_font_is_a_heading() {
    let document = xetex_book().clean();
    assert_eq!(block(&document, "tool exec").kind, BlockKind::Heading);
    let paragraph = block(&document, "command runs a task");
    assert_eq!(paragraph.kind, BlockKind::Paragraph);
    assert!(spoken(paragraph).starts_with("The tool exec command"), "{:?}", spoken(paragraph));
    assert!(spoken(paragraph).ends_with("still running."));
}

#[test]
fn xetex_sentence_starting_with_figure_is_spoken_and_a_caption_is_not() {
    let document = xetex_book().clean();
    let sentence = block(&document, "Figure 4.2 shows");
    assert_eq!(sentence.kind, BlockKind::Paragraph);
    assert!(!is_silent(sentence));
    let caption = block(&document, "Figure 5.1 Path-based watering");
    assert_eq!(caption.kind, BlockKind::Caption);
    assert!(!BlockKind::Caption.is_spoken_by_default());
}

#[test]
fn xetex_urls_are_shown_but_not_spoken() {
    let document = xetex_book().clean();
    let with_url = block(&document, "http://localhost:9000/stats");
    assert!(!spoken(with_url).contains("localhost"));
    assert!(spoken(with_url).starts_with("Visit"));
}

#[test]
fn xetex_table_is_shown_with_a_cue_and_the_text_after_it_is_spoken() {
    let document = xetex_book().clean();
    let tables = kinds_on(&document, 4, BlockKind::Table);
    assert_eq!(tables.len(), 1, "{:?}", blocks_on(&document, 4).iter().map(|block| (block.kind, shown(block))).collect::<Vec<_>>());
    assert!(tables[0].starts_with("Season | Task | Common cost"));
    assert!(tables[0].contains("summer | water | 200 liters, 1 hour"));
    assert_eq!(BlockKind::Table.cue_text(), Some("Table."));
    assert_eq!(block(&document, "Every season has").kind, BlockKind::Paragraph);
}

#[test]
fn xetex_inline_code_wrapped_to_the_start_of_a_line_stays_in_its_paragraph() {
    let document = xetex_book().clean();
    let port = block(&document, "named north-shed that");
    assert_eq!(port.kind, BlockKind::Paragraph);
    assert!(spoken(port).ends_with("on port 8080."), "{:?}", spoken(port));
}

#[test]
fn xetex_bold_word_wrapped_onto_its_own_line_continues_the_sentence() {
    let document = xetex_book().clean();
    assert!(!kinds_on(&document, 5, BlockKind::Heading).iter().any(|text| text.contains("side-gate")));
    assert!(spoken(block(&document, "a gate called front-gate")).ends_with("smaller gate called side-gate"));
}

#[test]
fn xetex_short_bare_domains_are_spoken_so_the_sentence_stays_whole() {
    let document = xetex_book().clean();
    assert!(spoken(block(&document, "The last rule")).ends_with("arriving on shop.acme.com"));
}

/// Courier footer page number and a running header on every page (GoF pages 11, 12 and 100).
fn courier_page<'a>(book: &'a mut Book, number: &str) -> PageWriter<'a> {
    let mut page = book.page(&COURIER, None);
    page.run_at(187.44, 743.76, 8.4, "CourierNew,Bold", "An Invented Book: Patterns for Tidy Gardens", false);
    page.run_at(486.36, 63.36, 13.2, "CourierNew,Bold", number, false);
    page
}

/// Five pages in the Design Patterns layout: printed numbers 11–14 on PDF pages 11–14 (offset 0).
fn courier_book() -> Book {
    let mut book = Book::new().outline(&[("Contents", 1, 0), ("1. Introduction", 1, 11), ("What is a Garden Pattern?", 2, 12), ("Bibliography", 1, 14)]);
    let mut contents = book.page(&COURIER, None);
    for entry in ["Preface .... 5", "1. Introduction .... 11", "What is a Garden Pattern? .... 12", "Bibliography .... 14", "Index .... 15"] {
        contents.line(entry);
    }
    for _ in 1..11 {
        book.page(&COURIER, None);
    }
    let mut intro = courier_page(&mut book, "11");
    intro.heading("CourierNew,Bold", 15.0, "1. Introduction");
    intro.lines(&[
        "A garden takes work, and a garden that stays tidy all year takes a lot",
        "more. First find the sunny spots, then group the plants that like the same",
        "soil, and leave room for paths. An old saying puts it well: \"no plan survives",
        "the first summer\" [ABC+77]. Still, gardeners who know the patterns rarely",
        "start from nothing.",
    ]);
    intro.lines(&["They reuse layouts that worked before, such as Trellis (338) and Border (94)."]);

    let mut patterns = courier_page(&mut book, "12");
    patterns.heading("CourierNew,Bold", 13.2, "What is a Garden Pattern?");
    patterns.lines(&["A pattern names a problem, a solution and the cost of using it."]);
    patterns.code(
        "CourierNew",
        6.6,
        9.6,
        &[(0, "void Bed::Water (int liters) {"), (4, "int left = min(liters, capacity);"), (4, "pump->Run(left);"), (0, "}")],
    );
    patterns.lines(&["The pump stores theBedTimer with which it was started."]);

    let mut list = courier_page(&mut book, "13");
    list.heading("CourierNew,Bold", 13.2, "Applicability");
    list.lines(&["Pick raised beds in these cases:"]);
    list.run_at(128.4, list.baseline, 9.36, "Symbol", "·", false);
    list.indented(33.84, "the soil is poor or wet and the plants need a dry, warm and loose");
    list.indented(33.84, "place to grow.");
    list.gap();
    list.run_at(128.4, list.baseline, 9.36, "Symbol", "·", false);
    list.indented(33.84, "you want to keep the paths clean.");

    let mut bibliography = courier_page(&mut book, "14");
    for entry in [
        "[ABC+77] A. Gardener, B. Planter. A Book of Beds. 1977.",
        "[GH90] G. Hoe. Small Gardens. 1990.",
        "[JK94] J. Kale. Soil Notes. 1994.",
        "[LM99] L. Mint. Paths. 1999.",
    ] {
        bibliography.lines(&[entry]);
    }
    book
}

#[test]
fn courier_header_and_bold_page_number_dropped_and_label_inferred() {
    let document = courier_book().clean();
    assert!(!has_text(&document, "Patterns for Tidy Gardens"));
    assert_eq!(label(&document, 11), Some("11"));
    assert_eq!(kinds_on(&document, 11, BlockKind::Heading), ["1. Introduction"]);
    assert!(blocks_on(&document, 11).iter().all(|block| shown(block).trim() != "11" && !shown(block).ends_with(' ')));
}

#[test]
fn courier_citation_and_page_references_are_shown_but_not_spoken() {
    let document = courier_book().clean();
    let paragraph = block(&document, "[ABC+77]. Still");
    assert!(spoken(paragraph).contains("summer\". Still, gardeners"), "{:?}", spoken(paragraph));
    let references = block(&document, "Trellis (338)");
    assert!(spoken(references).contains("such as Trellis and Border."), "{:?}", spoken(references));
}

#[test]
fn courier_smaller_size_is_code_with_rebuilt_indent() {
    let document = courier_book().clean();
    let code = block(&document, "void Bed::Water");
    assert_eq!(code.kind, BlockKind::Code);
    let body_line = shown(code).lines().find(|line| line.contains("int left")).map(str::to_string).unwrap();
    assert!(body_line.len() - body_line.trim_start().len() >= 4, "{body_line:?}");
    assert_eq!(block(&document, "stores theBedTimer").kind, BlockKind::Paragraph);
}

#[test]
fn courier_symbol_bullet_item_spans_two_lines() {
    let document = courier_book().clean();
    let items = kinds_on(&document, 13, BlockKind::ListItem);
    assert_eq!(items[0], "the soil is poor or wet and the plants need a dry, warm and loose place to grow.", "{items:?}");
    assert_eq!(items[1], "you want to keep the paths clean.");
}

#[test]
fn courier_toc_dropped_and_bibliography_silent() {
    let document = courier_book().clean();
    assert!(blocks_on(&document, 0).is_empty());
    let bibliography = blocks_on(&document, 14);
    assert!(!bibliography.is_empty() && bibliography.iter().all(|block| is_silent(block)));
}

const TIMES_BOLD: &str = "TimesNewRomanPS-BoldMT";
const CALIBRE_CODE: &str = "CourierNewPSMT";

/// Pages in the Clean Architecture layout: copyright, an ALL CAPS chapter opening, footnotes, a code listing.
fn calibre_book() -> Book {
    let mut book = Book::new().outline(&[("Copyright Page", 1, 1), ("Chapter 1 What Is a Garden?", 1, 2), ("The Plan", 2, 3)]);
    book.page(&CALIBRE, None);
    let mut copyright = book.page(&CALIBRE, None);
    copyright.lines(&["Copyright © 2026 Invented Press", "All rights reserved.", "ISBN-13: 978-0-12-345678-9"]);

    let mut chapter = book.page(&CALIBRE, None);
    chapter.run_at(298.5, 630.26, 30.0, TIMES_BOLD, "1", false);
    let mut x = 55.27;
    for (size, text) in [(27.75, "W"), (24.75, "HAT"), (27.75, " I"), (24.75, "S A"), (27.75, " G"), (24.75, "ARDEN"), (27.75, "?")] {
        x = chapter.run_at(x, 596.51, size, TIMES_BOLD, text, false);
    }
    chapter.at(262.5);
    let first = chapter.run_at(50.0, chapter.baseline, 15.0, "TimesNewRomanPSMT", "Bring a gardener from a hundred years back into a yard today", false);
    chapter.run_at(first, chapter.baseline + 5.25, 11.25, "TimesNewRomanPSMT", "1", false);
    chapter.run_at(first + 9.4, chapter.baseline, 15.0, "TimesNewRomanPSMT", "and", false);
    chapter.at(chapter.baseline - CALIBRE.pitch).lines(&["point at a raised bed: the box will look familiar at once."]);
    chapter.run_at(51.0, 85.0, 12.75, "TimesNewRomanPSMT", "1. Wooden beds of this kind are far older than you might think, as most", false);
    chapter.run_at(51.0, 70.0, 12.75, "TimesNewRomanPSMT", "gardeners.", false);

    let mut plan = book.page(&CALIBRE, None);
    plan.heading(TIMES_BOLD, 18.0, "THE PLAN");
    plan.lines(&["Here is a short program:"]);
    plan.run_at(50.0, plan.baseline, 12.75, TIMES_BOLD, "Click here to view code image", false);
    plan.at(plan.baseline - 2.0 * CALIBRE.pitch);
    plan.code(
        CALIBRE_CODE,
        12.75,
        14.25,
        &[(0, "bed.h"), (0, ""), (0, "struct Bed;"), (0, "struct Bed* makeBed(double w, double h);"), (0, "double area (struct Bed *b);")],
    );
    plan.lines(&["Code that includes bed.h never sees the fields inside the struct."]);
    plan.lines(&["Sort the tasks into four groups:"]);
    plan.lines(&["1. Urgent and important", "2. Important but not urgent"]);
    book
}

#[test]
fn calibre_all_caps_chapter_heading_takes_the_outline_casing() {
    let document = calibre_book().clean();
    assert_eq!(kinds_on(&document, 2, BlockKind::Heading), ["Chapter 1 What Is a Garden?"]);
    assert_eq!(kinds_on(&document, 3, BlockKind::Heading), ["The Plan"]);
    assert_eq!(label(&document, 2), None);
}

#[test]
fn calibre_raised_marker_is_silent_and_the_footnote_is_shown_only() {
    let document = calibre_book().clean();
    let paragraph = block(&document, "a yard today");
    assert!(spoken(paragraph).contains("yard today and point at"), "{:?}", spoken(paragraph));
    let footnote = block(&document, "far older than");
    assert_eq!(footnote.kind, BlockKind::Note);
    assert!(is_silent(footnote));
    assert!(shown(footnote).ends_with("gardeners."));
}

#[test]
fn calibre_code_image_link_dropped_and_code_listing_kept() {
    let document = calibre_book().clean();
    assert!(!has_text(&document, "Click here to view code image"));
    let listing = block(&document, "struct Bed;");
    assert_eq!(listing.kind, BlockKind::Code);
    assert!(shown(listing).starts_with("bed.h\n"), "{:?}", shown(listing));
    assert_eq!(block(&document, "Code that includes bed.h").kind, BlockKind::Paragraph);
}

#[test]
fn calibre_copyright_page_is_silent_and_body_sized_numbered_list_is_not_a_footnote() {
    let document = calibre_book().clean();
    assert!(blocks_on(&document, 0).is_empty());
    let copyright = blocks_on(&document, 1);
    assert!(!copyright.is_empty() && copyright.iter().all(|block| is_silent(block)));
    let item = block(&document, "Urgent and important");
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(!is_silent(item));
}

/// Styled "mathematical bold" letters, as Google Docs posts use for emphasis.
fn math_bold(text: &str) -> String {
    text.chars()
        .map(|letter| if letter.is_ascii_lowercase() { char::from_u32(0x1D41A + u32::from(letter) - u32::from('a')).unwrap() } else { letter })
        .collect()
}

/// Pages in the LinkedIn Posts layout: an image-only page, then posts with emoji and bold bullets.
fn google_docs_book() -> Book {
    let mut book = Book::new().outline(&[("Sorting Socks vs Folding Shirts", 2, 1), ("Ten Ways to Save Water", 2, 2)]);
    book.page(&GOOGLE_DOCS, None);
    let mut post = book.page(&GOOGLE_DOCS, None);
    post.heading("Arial-BoldMT", 16.0, "Sorting Socks vs Folding Shirts");
    post.lines(&["Laundry rooms tend to follow one of two habits for keeping a drawer tidy.", "People who care about it call this the “drawer order”."]);
    post.indented(18.0, "● Socks first");
    post.indented(36.0, "Most people sort the socks before they fold anything else, so the pairs");
    post.indented(36.0, "stay together.");
    post.gap();
    let styled = format!("When we {} we keep the drawer neat.", math_bold("fold shirts"));
    post.lines(&[styled.as_str()]);
    post.indented(18.0, "🔹 What is a Clean Drawer?");
    post.indented(18.0, "▶\u{FE0F} What Socks need?");

    let mut list = book.page(&GOOGLE_DOCS, None);
    list.heading("Arial-BoldMT", 16.0, "Ten Ways to Save Water");
    list.lines(&["1 - Shorter Showers", "2 - Rain Barrels", "3 - Drip Lines"]);
    list.lines(&["4 - Full Loads: Start the washer only once the drum is full, whether the cycle is cold,", "warm, or hot, ideally in the evening."]);
    list.lines(&["Your turn: which tricks keep your own water bill low?"]);
    book
}

#[test]
fn google_docs_image_only_page_has_no_blocks_and_post_heading_kept() {
    let document = google_docs_book().clean();
    assert!(blocks_on(&document, 0).is_empty());
    assert_eq!(block(&document, "Sorting Socks vs Folding Shirts").kind, BlockKind::Heading);
    assert_eq!(block(&document, "Laundry rooms tend").kind, BlockKind::Paragraph);
    assert!(kinds_on(&document, 1, BlockKind::ListItem).contains(&"Socks first".to_string()), "{:?}", kinds_on(&document, 1, BlockKind::ListItem));
}

#[test]
fn google_docs_styled_letters_become_plain_text_and_emoji_bullets_are_stripped() {
    let document = google_docs_book().clean();
    assert!(has_text(&document, "When we fold shirts we keep"));
    let item = block(&document, "What is a Clean Drawer?");
    assert_eq!(item.kind, BlockKind::ListItem);
    assert!(shown(item).starts_with("What is a Clean Drawer?"));
    assert!(!has_text(&document, "\u{FE0F}"));
    assert_eq!(spoken(block(&document, "What Socks need?")), "What Socks need?");
}

#[test]
fn google_docs_dash_numbered_lines_are_list_items_and_carry_on_without_a_hanging_indent() {
    let document = google_docs_book().clean();
    let items = kinds_on(&document, 2, BlockKind::ListItem);
    assert!(items.contains(&"1 - Shorter Showers".to_string()) && items.contains(&"2 - Rain Barrels".to_string()), "{items:?}");
    let full_loads = block(&document, "Full Loads:");
    assert_eq!(full_loads.kind, BlockKind::ListItem);
    assert!(spoken(full_loads).ends_with("ideally in the evening."), "{:?}", spoken(full_loads));
}
