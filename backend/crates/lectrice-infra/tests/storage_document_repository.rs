use std::fs;
use std::path::Path;

use lectrice_domain::{
    Block, BlockKind, Document, DocumentId, DocumentMeta, DocumentReader, DocumentWriter, ErrorName, ListOptions, PageLabel, ReadingPosition, Sentence,
    SentenceId, Timestamp,
};
use lectrice_infra::FsDocumentRepository;
use tempfile::TempDir;

const PDF_BYTES: &[u8] = b"%PDF-1.7 fake";

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_timestamp(seconds, 0).unwrap()
}

fn id_for(n: u8) -> DocumentId {
    DocumentId::parse(&format!("{n:064x}")).unwrap()
}

fn sentence(id: u32, text: &str) -> Sentence {
    Sentence { id: SentenceId(id), text: text.to_string(), page_index: 3, speech_text: None }
}

fn document(id: DocumentId) -> Document {
    Document {
        id,
        title: "A Book".into(),
        source_file_name: "a-book.pdf".into(),
        page_count: 12,
        cleaner_version: 1,
        pages: vec![PageLabel { index: 3, label: "1".into() }],
        blocks: vec![
            Block {
                id: 0,
                kind: BlockKind::Paragraph,
                level: None,
                page_index: 3,
                spoken: true,
                text: None,
                sentences: vec![sentence(0, "One."), sentence(1, "Two.")],
                cue: None,
            },
            Block {
                id: 1,
                kind: BlockKind::Code,
                level: None,
                page_index: 3,
                spoken: false,
                text: Some("x = 1".into()),
                sentences: vec![sentence(2, "x = 1")],
                cue: Some(sentence(3, "Code example.")),
            },
        ],
    }
}

fn meta(opened: i64) -> DocumentMeta {
    DocumentMeta { added_at: at(1), last_opened_at: at(opened), last_sentence_id: None }
}

fn file_names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

#[test]
fn saved_document_round_trips() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));

    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();

    assert_eq!(repository.find(&saved.id).unwrap(), Some(saved.clone()));
    assert_eq!(repository.find_original_pdf(&saved.id).unwrap().as_deref(), Some(PDF_BYTES));
    let summary = repository.find_summary(&saved.id).unwrap().unwrap();
    assert_eq!(summary.title, "A Book");
    assert_eq!(summary.source_file_name, "a-book.pdf");
    assert_eq!(summary.page_count, 12);
    assert_eq!(summary.sentence_count, 4);
    assert_eq!(summary.added_at, at(1));
    assert_eq!(summary.last_opened_at, at(5));
    assert_eq!(summary.last_sentence_id, None);
}

#[test]
fn unknown_document_is_none() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());

    assert_eq!(repository.find(&id_for(9)).unwrap(), None);
    assert_eq!(repository.find_summary(&id_for(9)).unwrap(), None);
    assert_eq!(repository.find_original_pdf(&id_for(9)).unwrap(), None);
}

#[test]
fn library_layout_has_only_the_three_files() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));

    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();
    repository.save_position(&saved.id, ReadingPosition { sentence_id: SentenceId(2) }).unwrap();
    repository.mark_opened(&saved.id, at(8)).unwrap();

    let directory = data_dir.path().join("library").join(saved.id.as_str());
    assert_eq!(file_names(&directory), ["document.json", "meta.json", "original.pdf"]);
}

#[test]
fn failed_write_leaves_no_partial_or_temp_files() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));
    let directory = data_dir.path().join("library").join(saved.id.as_str());
    fs::create_dir_all(directory.join("document.json")).unwrap();

    let error = repository.save(&saved, PDF_BYTES, &meta(5)).unwrap_err();

    assert_eq!(error.primary(), ErrorName::StorageFailed);
    assert_eq!(file_names(&directory), ["document.json", "original.pdf"]);
    assert!(directory.join("document.json").is_dir());
    assert_eq!(repository.find_summary(&saved.id).unwrap(), None);
}

#[test]
fn position_and_opened_at_are_saved() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));
    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();

    repository.save_position(&saved.id, ReadingPosition { sentence_id: SentenceId(3) }).unwrap();
    repository.mark_opened(&saved.id, at(42)).unwrap();

    let summary = FsDocumentRepository::new(data_dir.path()).find_summary(&saved.id).unwrap().unwrap();
    assert_eq!(summary.last_sentence_id, Some(SentenceId(3)));
    assert_eq!(summary.last_opened_at, at(42));
    assert_eq!(summary.added_at, at(1));
}

#[test]
fn updating_meta_of_unknown_document_is_not_found() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());

    let error = repository.mark_opened(&id_for(7), at(1)).unwrap_err();

    assert_eq!(error.primary(), ErrorName::DocumentNotFound);
}

#[test]
fn list_recent_is_empty_without_a_library() {
    let data_dir = TempDir::new().unwrap();
    let page = FsDocumentRepository::new(data_dir.path()).list_recent(ListOptions::default()).unwrap();

    assert!(page.items.is_empty());
    assert_eq!((page.total, page.limit, page.offset), (0, 20, 0));
}

#[test]
fn list_recent_orders_by_last_opened_and_paginates() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    for (n, opened) in [(1, 10), (2, 30), (3, 20), (4, 40), (5, 5)] {
        repository.save(&document(id_for(n)), PDF_BYTES, &meta(opened)).unwrap();
    }
    repository.mark_opened(&id_for(5), at(50)).unwrap();
    fs::create_dir_all(data_dir.path().join("library").join("not-a-document")).unwrap();

    let first = repository.list_recent(ListOptions { limit: Some(2), offset: None }).unwrap();
    let second = repository.list_recent(ListOptions { limit: Some(2), offset: Some(2) }).unwrap();
    let last = repository.list_recent(ListOptions { limit: Some(2), offset: Some(4) }).unwrap();

    let ids = |page: &lectrice_domain::Page<lectrice_domain::DocumentSummary>| page.items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&first), [id_for(5), id_for(4)]);
    assert_eq!(ids(&second), [id_for(2), id_for(3)]);
    assert_eq!(ids(&last), [id_for(1)]);
    assert_eq!((first.total, first.limit, first.offset), (5, 2, 0));
    assert_eq!((last.total, last.offset), (5, 4));
}

#[test]
fn document_without_meta_is_not_in_the_library() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));
    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();
    fs::remove_file(data_dir.path().join("library").join(saved.id.as_str()).join("meta.json")).unwrap();

    assert_eq!(repository.find_summary(&saved.id).unwrap(), None);
    assert_eq!(repository.list_recent(ListOptions::default()).unwrap().total, 0);
}

/// Review L5: re-cleaning read the meta, worked for seconds, then saved it whole, losing a position saved meanwhile.
#[test]
fn saving_again_keeps_a_position_and_open_recorded_meanwhile() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let saved = document(id_for(1));
    let read_before_recleaning = meta(5);
    repository.save(&saved, PDF_BYTES, &read_before_recleaning).unwrap();

    repository.save_position(&saved.id, ReadingPosition { sentence_id: SentenceId(2) }).unwrap();
    repository.mark_opened(&saved.id, at(42)).unwrap();
    repository.save(&saved, PDF_BYTES, &read_before_recleaning).unwrap();

    let summary = repository.find_summary(&saved.id).unwrap().unwrap();
    assert_eq!((summary.last_sentence_id, summary.last_opened_at, summary.added_at), (Some(SentenceId(2)), at(42), at(1)));
}

#[test]
fn saving_again_drops_a_saved_position_the_new_document_lacks() {
    let data_dir = TempDir::new().unwrap();
    let repository = FsDocumentRepository::new(data_dir.path());
    let mut saved = document(id_for(1));
    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();
    repository.save_position(&saved.id, ReadingPosition { sentence_id: SentenceId(3) }).unwrap();

    saved.blocks.truncate(1);
    repository.save(&saved, PDF_BYTES, &meta(5)).unwrap();

    assert_eq!(repository.find_summary(&saved.id).unwrap().unwrap().last_sentence_id, None);
}
