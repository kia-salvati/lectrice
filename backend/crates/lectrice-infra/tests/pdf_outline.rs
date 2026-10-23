//! Broken bookmark trees must not crash the import (review H1: a recursive walk overflowed the stack).

mod pdf_support;

use std::fmt::Write as _;

use lectrice_domain::PdfTextSource;
use pdf_support::text_source;

const FIRST_OUTLINE_OBJECT: usize = 5;

/// A one-page PDF with "Hello" on it and the given outline item dictionaries, numbered from
/// `FIRST_OUTLINE_OBJECT`. The outline root's `/First` is the first item.
fn pdf_with_outline(items: &[String]) -> Vec<u8> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents CONTENTS 0 R /Resources << /Font << /F1 FONT 0 R >> >> >>".to_string(),
        format!("<< /Type /Outlines /First {FIRST_OUTLINE_OBJECT} 0 R /Last {FIRST_OUTLINE_OBJECT} 0 R /Count 1 >>"),
    ];
    objects.extend(items.iter().cloned());
    let contents_id = objects.len() + 1;
    let stream = "BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
    objects.push(format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()));
    let font_id = objects.len() + 1;
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    objects[2] = objects[2].replace("CONTENTS", &contents_id.to_string()).replace("FONT", &font_id.to_string());

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

#[test]
fn a_bookmark_that_is_its_own_first_child_is_read_once() {
    let looping = format!("<< /Title (Loop) /Parent 4 0 R /First {id} 0 R /Last {id} 0 R /Count 1 /Dest [3 0 R /Fit] >>", id = FIRST_OUTLINE_OBJECT);
    let content = text_source().read_runs(&pdf_with_outline(&[looping])).unwrap();
    assert_eq!(content.outline.len(), 1);
    assert_eq!(content.outline[0].title, "Loop");
    assert_eq!(content.outline[0].pdf_page, Some(1));
}

#[test]
fn a_bookmark_that_is_its_own_next_sibling_is_read_once() {
    let looping = format!("<< /Title (Loop) /Parent 4 0 R /Next {id} 0 R /Dest [3 0 R /Fit] >>", id = FIRST_OUTLINE_OBJECT);
    let content = text_source().read_runs(&pdf_with_outline(&[looping])).unwrap();
    assert_eq!(content.outline.len(), 1);
}

#[test]
fn a_very_deep_bookmark_chain_is_cut_without_overflowing_the_stack() {
    const CHAIN: usize = 5_000;
    let items: Vec<String> = (0..CHAIN)
        .map(|index| {
            let id = FIRST_OUTLINE_OBJECT + index;
            let child = if index + 1 < CHAIN { format!("/First {next} 0 R /Last {next} 0 R /Count 1", next = id + 1) } else { String::new() };
            format!("<< /Title (Level {index}) /Parent {parent} 0 R {child} >>", parent = if index == 0 { 4 } else { id - 1 })
        })
        .collect();
    let pdf = pdf_with_outline(&items);
    // A small stack, like a tokio blocking thread's, would have overflowed with the recursive walk.
    let outline = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || text_source().read_runs(&pdf).unwrap().outline).unwrap().join().unwrap();
    assert_eq!(outline.len(), 16);
    assert_eq!(outline.last().unwrap().level, 16);
    assert!(outline.windows(2).all(|pair| pair[1].level == pair[0].level + 1));
}
