use std::fs;
use std::sync::Arc;

use lectrice_domain::{Block, BlockKind, Document, DocumentId, DocumentMeta, DocumentReader, DocumentWriter, Sentence, SentenceId, Timestamp};
use lectrice_infra::{CachedLibrary, FsDocumentRepository};
use tempfile::TempDir;

const PDF_BYTES: &[u8] = b"%PDF-1.7 fake";

fn id_for(n: u8) -> DocumentId {
    DocumentId::parse(&format!("{n:064x}")).unwrap()
}

fn document(id: DocumentId, title: &str) -> Document {
    let sentence = Sentence { id: SentenceId(0), text: "One.".into(), page_index: 1, speech_text: None };
    let block = Block { id: 0, kind: BlockKind::Paragraph, level: None, page_index: 1, spoken: true, text: None, sentences: vec![sentence], cue: None };
    Document {
        id,
        title: title.into(),
        source_file_name: "a.pdf".into(),
        page_count: 1,
        cleaner_version: 1,
        pages: Vec::new(),
        toc: Vec::new(),
        blocks: vec![block],
    }
}

fn meta() -> DocumentMeta {
    let now = Timestamp::from_timestamp(1, 0).unwrap();
    DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None }
}

fn document_json(dir: &TempDir, id: &DocumentId) -> std::path::PathBuf {
    dir.path().join("library").join(id.as_str()).join("document.json")
}

#[test]
fn a_found_document_is_served_from_memory() {
    let dir = TempDir::new().unwrap();
    let library = CachedLibrary::new(Arc::new(FsDocumentRepository::new(dir.path())), 2);
    let id = id_for(1);
    library.save(&document(id.clone(), "First"), PDF_BYTES, &meta()).unwrap();
    assert_eq!(library.find(&id).unwrap().unwrap().title, "First");

    fs::remove_file(document_json(&dir, &id)).unwrap();

    assert_eq!(library.find(&id).unwrap().unwrap().title, "First");
}

#[test]
fn saving_replaces_the_cached_copy() {
    let dir = TempDir::new().unwrap();
    let library = CachedLibrary::new(Arc::new(FsDocumentRepository::new(dir.path())), 2);
    let id = id_for(1);
    library.save(&document(id.clone(), "First"), PDF_BYTES, &meta()).unwrap();
    library.find(&id).unwrap();

    library.save(&document(id.clone(), "Re-cleaned"), PDF_BYTES, &meta()).unwrap();

    assert_eq!(library.find(&id).unwrap().unwrap().title, "Re-cleaned");
}

#[test]
fn only_the_most_recent_documents_are_kept() {
    let dir = TempDir::new().unwrap();
    let library = CachedLibrary::new(Arc::new(FsDocumentRepository::new(dir.path())), 1);
    let (first, second) = (id_for(1), id_for(2));
    library.save(&document(first.clone(), "First"), PDF_BYTES, &meta()).unwrap();
    library.save(&document(second.clone(), "Second"), PDF_BYTES, &meta()).unwrap();
    library.find(&first).unwrap();
    library.find(&second).unwrap();

    fs::remove_file(document_json(&dir, &first)).unwrap();
    fs::remove_file(document_json(&dir, &second)).unwrap();

    assert!(library.find(&first).unwrap().is_none());
    assert_eq!(library.find(&second).unwrap().unwrap().title, "Second");
}
