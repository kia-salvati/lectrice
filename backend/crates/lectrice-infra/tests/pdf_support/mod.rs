#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use lectrice_infra::pdf::PdfiumTextSource;

/// The user's books on the Desktop, read in place. Tests that need them are `#[ignore]`.
pub struct Book {
    pub key: &'static str,
    pub file_name: &'static str,
}

pub const BOOKS: [Book; 5] = [
    Book { key: "clean-architecture", file_name: "Clean_Architecture_A_Craftsman's_Guide_to_Software_Robert_Martin.pdf" },
    Book { key: "designing-distributed-systems", file_name: "EN-CNTNT-eBook-DesigningDistributedSystems.pdf" },
    Book { key: "design-patterns", file_name: "Gang_of_Four_Design_Patterns_Elements_of_Reusable_Object_Oriented.pdf" },
    Book { key: "linkedin-posts", file_name: "Linkedin_Posts_2024_Blue.pdf" },
    Book { key: "kubernetes-book", file_name: "Poulton N. The Kubernetes Book 2024 Edition.pdf" },
];

pub fn book(key: &str) -> &'static Book {
    BOOKS.iter().find(|book| book.key == key).expect("known book key")
}

impl Book {
    pub fn path(&self) -> PathBuf {
        std::env::home_dir().expect("home dir").join("Desktop").join(self.file_name)
    }

    pub fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.path()).expect("the book is on the Desktop")
    }
}

/// One shared source: PDFium can be bound only once at a time per process, and tests run in parallel.
pub fn text_source() -> &'static PdfiumTextSource {
    static SOURCE: OnceLock<PdfiumTextSource> = OnceLock::new();
    SOURCE.get_or_init(|| PdfiumTextSource::new(&pdfium_library()).expect("libpdfium loads"))
}

pub fn pdfium_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/lib/libpdfium.so")
}

/// The cleaner fixtures hold book text, so they live in the gitignored `backend/test-data/books/`.
pub fn book_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data/books")
}
