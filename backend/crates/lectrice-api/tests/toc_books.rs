//! The table of contents of each of the user's five books, end to end (PDFium → cleaner → sentences → toc):
//! `cargo test --release -p lectrice-api --test toc_books -- --ignored --nocapture` prints the first entries.
//! Reads the PDFs from `~/Desktop` in place and skips a book that isn't there; nothing from them is stored.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lectrice_app::documents::DocumentPipeline;
use lectrice_domain::cleaning::{RuleBasedCleaner, build_toc};
use lectrice_domain::{ContentHasher, Document, TocEntry};
use lectrice_infra::{PdfiumTextSource, SentencexSplitter, Sha256ContentHasher};

const PRINTED_ENTRIES: usize = 30;
const FALLBACK_PRINTED_ENTRIES: usize = 12;

struct Expected {
    file_name: &'static str,
    entries: usize,
    first: [(&'static str, u8); 4],
    /// An entry further in, with its level.
    also: (&'static str, u8),
}

const BOOKS: [Expected; 5] = [
    Expected {
        file_name: "Clean_Architecture_A_Craftsman's_Guide_to_Software_Robert_Martin.pdf",
        entries: 222,
        first: [("Dedication", 1), ("Foreword", 1), ("Preface", 1), ("Acknowledgments", 1)],
        also: ("Chapter 34 The Missing Chapter", 2),
    },
    Expected {
        file_name: "EN-CNTNT-eBook-DesigningDistributedSystems.pdf",
        entries: 83,
        first: [("Preface", 1), ("Who Should Read This Book", 2), ("Why I Wrote This Book", 2), ("The World of Distributed Systems Today", 2)],
        also: ("Chapter 2. The Sidecar Pattern", 2),
    },
    Expected {
        file_name: "Gang_of_Four_Design_Patterns_Elements_of_Reusable_Object_Oriented.pdf",
        entries: 390,
        first: [("Preface to CD", 1), ("Preface to Book", 1), ("Foreword", 1), ("Guide to Readers", 1)],
        also: ("Should Communication be Encapsulated or Distributed?", 3),
    },
    Expected {
        file_name: "Linkedin_Posts_2024_Blue.pdf",
        entries: 198,
        first: [
            ("Big Endian vs Little Endian", 1),
            ("How do we incorporate Event Sourcing into the systems?", 1),
            ("How can Cache Systems go wrong", 1),
            ("Linux file system explained", 1),
        ],
        also: ("Encoding vs Encryption vs Tokenization", 1),
    },
    Expected {
        file_name: "Poulton N. The Kubernetes Book 2024 Edition.pdf",
        entries: 125,
        first: [
            ("0: Preface", 1),
            ("Editions Paperbacks, hardbacks, eBooks, audio, and translations", 2),
            ("The sample app and GitHub repo", 2),
            ("Windows users", 2),
        ],
        also: ("6: Kubernetes Deployments", 1),
    },
];

#[test]
#[ignore = "reads the user's books from ~/Desktop"]
fn every_book_has_a_table_of_contents_that_starts_at_its_first_chapter() {
    let books: Vec<(&Expected, Vec<u8>)> = BOOKS
        .iter()
        .filter_map(|book| {
            let bytes = std::fs::read(book_path(book.file_name)).ok();
            if bytes.is_none() {
                println!("skipped, not on this machine: {}", book.file_name);
            }
            bytes.map(|bytes| (book, bytes))
        })
        .collect();
    if books.is_empty() {
        return;
    }
    // PDFium is loaded only when there is a book to read: a fresh clone has neither.
    let pipeline = pipeline();
    for (book, bytes) in &books {
        let document = &document(&pipeline, book.file_name, bytes);
        print_toc(document);
        let first: Vec<(&str, u8)> = document.toc.iter().take(book.first.len()).map(|entry| (entry.title.as_str(), entry.level)).collect();
        assert_eq!(first, book.first, "{}", book.file_name);
        assert_eq!(document.toc.len(), book.entries, "{}", book.file_name);
        assert!(document.toc.iter().any(|entry| (entry.title.as_str(), entry.level) == book.also), "{}: {:?}", book.file_name, book.also);
        assert_well_formed(document);

        let from_headings = Document { toc: build_toc(&[], &document.blocks), ..document.clone() };
        println!("\n{} without its outline: {} entries from headings", document.title, from_headings.toc.len());
        from_headings.toc.iter().take(FALLBACK_PRINTED_ENTRIES).for_each(|entry| println!("{}", line(document, entry)));
        assert!(from_headings.toc.len() > book.entries / 2, "{}", book.file_name);
        assert_well_formed(&from_headings);
    }
}

fn assert_well_formed(document: &Document) {
    assert_eq!(document.toc.iter().map(|entry| entry.level).min(), Some(1));
    assert!(document.toc.iter().all(|entry| (1..=3).contains(&entry.level) && !entry.title.is_empty()));
    assert!(document.toc.windows(2).all(|pair| pair[0].sentence_id < pair[1].sentence_id && pair[1].level <= pair[0].level + 1));
    let sentence_count = document.sentence_count();
    assert!(document.toc.iter().all(|entry| entry.sentence_id.0 < sentence_count));
}

fn print_toc(document: &Document) {
    println!("\n{} ({} entries)", document.title, document.toc.len());
    for entry in document.toc.iter().take(PRINTED_ENTRIES) {
        println!("{}", line(document, entry));
    }
}

fn line(document: &Document, entry: &TocEntry) -> String {
    let label =
        document.pages.iter().find(|page| page.index == entry.page_index).map_or_else(|| format!("pdf {}", entry.page_index), |page| page.label.clone());
    format!("{}{} · p. {label} · s{}", "  ".repeat(usize::from(entry.level - 1)), entry.title, entry.sentence_id)
}

fn pipeline() -> DocumentPipeline {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/lib/libpdfium.so");
    let source = PdfiumTextSource::new(&library).expect("libpdfium loads");
    DocumentPipeline::new(Arc::new(source), Arc::new(RuleBasedCleaner), Arc::new(SentencexSplitter))
}

/// The books are the user's own and never committed.
fn book_path(file_name: &str) -> PathBuf {
    std::env::home_dir().expect("home dir").join("Desktop").join(file_name)
}

fn document(pipeline: &DocumentPipeline, file_name: &str, bytes: &[u8]) -> Document {
    let id = Sha256ContentHasher.document_id(bytes).expect("hashable");
    pipeline.process(&id, file_name, bytes).expect("the book cleans")
}
