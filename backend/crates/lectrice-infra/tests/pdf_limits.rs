//! Import limits: a PDF past the page, character or time budget fails with `PdfTooComplex`
//! instead of holding the import thread or the memory.

mod pdf_support;

use std::fmt::Write as _;
use std::time::Duration;

use lectrice_domain::{ErrorName, PdfTextSource};
use lectrice_infra::PdfLimits;
use pdf_support::text_source;

const FIRST_PAGE_OBJECT: usize = 3;

/// `pages` pages that share one content stream showing `line`.
fn pdf_with_pages(pages: usize, line: &str) -> Vec<u8> {
    let contents_id = FIRST_PAGE_OBJECT + pages;
    let font_id = contents_id + 1;
    let kids: String = (0..pages).map(|page| format!("{} 0 R ", FIRST_PAGE_OBJECT + page)).collect();
    let mut objects = vec!["<< /Type /Catalog /Pages 2 0 R >>".to_string(), format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>")];
    for _ in 0..pages {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {contents_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >>"
        ));
    }
    let stream = format!("BT /F1 12 Tf 72 720 Td ({line}) Tj ET");
    objects.push(format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n{body}\nendobj\n", index + 1).unwrap();
    }
    let xref = pdf.len();
    write!(pdf, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).unwrap();
    for offset in offsets {
        writeln!(pdf, "{offset:010} 00000 n ").unwrap();
    }
    write!(pdf, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).unwrap();
    pdf.into_bytes()
}

fn limits(max_pages: usize, max_chars: usize, max_duration: Duration) -> PdfLimits {
    PdfLimits { max_pages, max_chars, max_duration }
}

const NO_HURRY: Duration = Duration::from_secs(60);

#[test]
fn a_pdf_within_the_limits_is_read() {
    let content = text_source().read_runs_within(&pdf_with_pages(3, "Hello"), &limits(3, 15, NO_HURRY)).unwrap();
    assert_eq!(content.pages.len(), 3);
    assert_eq!(text_source().read_runs(&pdf_with_pages(3, "Hello")).unwrap().pages.len(), 3, "the default limits allow a normal PDF");
}

#[test]
fn too_many_pages_is_refused_before_any_page_is_read() {
    let error = text_source().read_runs_within(&pdf_with_pages(4, "Hello"), &limits(3, usize::MAX, NO_HURRY)).unwrap_err();
    assert_eq!(error.primary(), ErrorName::PdfTooComplex);
    assert!(error.detail().unwrap().contains("4 pages"), "{error}");
}

#[test]
fn too_many_characters_is_refused() {
    let error = text_source().read_runs_within(&pdf_with_pages(3, "Hello"), &limits(10, 14, NO_HURRY)).unwrap_err();
    assert_eq!(error.primary(), ErrorName::PdfTooComplex);
    let one_big_page = pdf_with_pages(1, &"word ".repeat(400));
    let error = text_source().read_runs_within(&one_big_page, &limits(10, 1000, NO_HURRY)).unwrap_err();
    assert_eq!(error.primary(), ErrorName::PdfTooComplex);
}

#[test]
fn a_read_past_its_time_budget_stops() {
    let error = text_source().read_runs_within(&pdf_with_pages(50, "Hello"), &limits(100, usize::MAX, Duration::ZERO)).unwrap_err();
    assert_eq!(error.primary(), ErrorName::PdfTooComplex);
    assert!(error.detail().unwrap().contains("longer than"), "{error}");
}

#[test]
fn the_error_is_operational_with_its_own_message() {
    let spec = ErrorName::PdfTooComplex.spec();
    assert_eq!((spec.code, spec.status, spec.operational), (1010, 422, true));
}
