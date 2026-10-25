//! Removing, restoring and purging books, and the storage numbers, with fakes and without a speech engine.

use std::sync::Arc;

use lectrice_domain::{
    AudioCache, AudioExport, AudioKey, DocumentId, DocumentWriter, ErrorName, ExportFormat, ExportId, ExportLocation, ExportRecord, ExportRecordStore,
    ExportSpeed, ExportStage, ExportStatus, SentenceId, VoiceId,
};

use super::*;
use crate::documents::fakes::{FakeHasher, FakeSplitter, Library, at, id_for};
use crate::documents::{GetDocument, GetDocumentArgs};
use crate::exports::fakes::{CountingDecoder, FakeExporter, MemoryRecordStore};
use crate::exports::{ExportDeps, ExportJobs, ExportSettings};
use crate::services::{Adapters, Settings, assemble, purge_removed_at_start};
use crate::speech::ReadAheadSettings;
use crate::speech::fakes::{CountingEncoder, MemoryCache};
use crate::{RequestContext, UseCase};

struct Setup {
    library: Library,
    cache: Arc<MemoryCache>,
    exporter: Arc<FakeExporter>,
    jobs: Arc<ExportJobs>,
}

fn setup_with(store: Arc<MemoryRecordStore>) -> Setup {
    let library = Library::new(1);
    let cache = Arc::new(MemoryCache::default());
    let exporter = Arc::new(FakeExporter::default());
    let deps = ExportDeps {
        speech: None,
        reader: library.reader(),
        cache: cache.clone(),
        decoder: Arc::new(CountingDecoder),
        exporter: exporter.clone(),
        store,
        clock: library.clock(),
        time_stretch: None,
    };
    let jobs = Arc::new(ExportJobs::start(deps, ExportSettings::default()).unwrap());
    Setup { library, cache, exporter, jobs }
}

fn setup() -> Setup {
    setup_with(Arc::default())
}

fn ctx() -> RequestContext {
    RequestContext::new("test")
}

fn clip_key(document_id: &DocumentId, sentence: u32) -> AudioKey {
    AudioKey {
        document_id: document_id.clone(),
        cleaner_version: 1,
        engine_id: "engine".into(),
        voice_id: VoiceId::new("af_heart"),
        sentence_id: SentenceId(sentence),
    }
}

fn record(id: &str, document_id: DocumentId, status: ExportStatus, file_bytes: Option<u64>) -> ExportRecord {
    let path = format!("/exports/{id}.m4b");
    let done = status == ExportStatus::Done;
    ExportRecord {
        export: AudioExport {
            id: ExportId::new(id),
            document_id,
            voice_id: VoiceId::new("af_heart"),
            format: ExportFormat::M4b,
            speed: ExportSpeed::NORMAL,
            engine_id: "engine".into(),
            range: None,
            range_label: None,
            title: "Fake Book".into(),
            status,
            paused_by: None,
            stage: ExportStage::Parts,
            error: None,
            sentences_done: 0,
            sentences_total: 4,
            parts_done: 0,
            parts_total: 1,
            estimated_seconds_left: None,
            file_bytes,
            location: done.then(|| ExportLocation::AppStorage { path: path.clone().into() }),
            waiting_for_engine: false,
        },
        read_skipped: false,
        created_at: at(10),
        file: done.then(|| path.into()),
        parts: Vec::new(),
        chapters: Vec::new(),
        source_samples: 0,
    }
}

fn remove(setup: &Setup, id: &DocumentId) -> lectrice_domain::AppResult<()> {
    RemoveDocument::new(setup.library.reader(), setup.library.writer(), setup.library.clock(), None).run(&ctx(), id.clone())
}

fn purge(setup: &Setup, delete_exports_for: Vec<DocumentId>) -> PurgeReport {
    let service = PurgeRemovedDocuments::new(setup.library.reader(), setup.library.writer(), setup.cache.clone(), setup.jobs.clone());
    service.run(&ctx(), PurgeRemovedDocumentsArgs { delete_exports_for }).unwrap()
}

#[test]
fn a_removed_book_is_hidden_until_it_is_restored_with_its_position() {
    let setup = setup();
    let id = setup.library.add_book();
    setup.library.store.set_position(&id, 2);

    remove(&setup, &id).unwrap();

    let reader = setup.library.reader();
    assert_eq!(reader.list_recent(Default::default()).unwrap().total, 0);
    let get = GetDocument::new(setup.library.pipeline(), reader.clone(), setup.library.writer(), setup.library.clock());
    assert_eq!(get.run(&ctx(), GetDocumentArgs { id: id.clone() }).unwrap_err().primary(), ErrorName::DocumentNotFound);
    assert_eq!(remove(&setup, &id).unwrap_err().primary(), ErrorName::DocumentNotFound, "already removed");

    let restored = RestoreDocument::new(reader.clone(), setup.library.writer()).run(&ctx(), id.clone()).unwrap();

    assert_eq!(restored.last_sentence_id, Some(SentenceId(2)));
    assert_eq!(reader.list_recent(Default::default()).unwrap().total, 1);
    let unknown = RestoreDocument::new(reader, setup.library.writer()).run(&ctx(), id_for(9));
    assert_eq!(unknown.unwrap_err().primary(), ErrorName::DocumentNotFound);
}

#[test]
fn a_purge_deletes_the_book_and_its_audio_and_keeps_its_finished_exports() {
    let store = Arc::new(MemoryRecordStore::default());
    store.save(&record("kept", id_for(1), ExportStatus::Done, Some(10))).unwrap();
    store.save(&record("unfinished", id_for(1), ExportStatus::Paused, None)).unwrap();
    let setup = setup_with(store.clone());
    let id = setup.library.add_book();
    let other = id_for(7);
    for key in [clip_key(&id, 0), clip_key(&id, 1), clip_key(&other, 0)] {
        setup.cache.put(&key, b"clip").unwrap();
    }
    remove(&setup, &id).unwrap();

    let report = purge(&setup, Vec::new());

    assert_eq!(report, PurgeReport { documents: 1, failed: 0, exports_cancelled: 1, exports_deleted: 0, clips: 2 });
    assert!(!setup.library.store.contains(&id));
    assert_eq!(setup.cache.keys(), [clip_key(&other, 0)]);
    let listed = setup.jobs.list_all();
    assert_eq!(listed.iter().map(|export| (export.id.as_str(), export.title.as_str())).collect::<Vec<_>>(), [("kept", "Fake Book")]);
    assert!(store.get(&ExportId::new("unfinished")).is_none());
    assert_eq!(purge(&setup, Vec::new()).documents, 0, "nothing left to purge");
}

#[test]
fn a_purge_deletes_finished_exports_only_when_asked() {
    let store = Arc::new(MemoryRecordStore::default());
    store.save(&record("done", id_for(1), ExportStatus::Done, Some(10))).unwrap();
    let setup = setup_with(store);
    let id = setup.library.add_book();
    remove(&setup, &id).unwrap();

    let report = purge(&setup, vec![id]);

    assert_eq!(report.exports_deleted, 1);
    assert_eq!(setup.exporter.deleted(), [std::path::PathBuf::from("/exports/done.m4b")]);
    assert!(setup.jobs.list_all().is_empty());
}

#[test]
fn clearing_a_books_audio_keeps_the_book_and_other_books_audio() {
    let setup = setup();
    let id = setup.library.add_book();
    let other = id_for(7);
    for key in [clip_key(&id, 0), clip_key(&other, 0)] {
        setup.cache.put(&key, b"clip").unwrap();
    }

    let eviction = ClearDocumentAudio::new(setup.cache.clone()).run(&ctx(), id.clone()).unwrap();

    assert_eq!(eviction.evicted_clips(), 1);
    assert_eq!(setup.cache.keys(), [clip_key(&other, 0)]);
    assert!(setup.library.store.contains(&id));
}

#[test]
fn storage_usage_counts_books_their_audio_and_exports_in_app_storage() {
    let store = Arc::new(MemoryRecordStore::default());
    store.save(&record("in-app", id_for(1), ExportStatus::Done, Some(300))).unwrap();
    let mut shared = record("shared", id_for(1), ExportStatus::Done, Some(900));
    shared.export.location = Some(ExportLocation::Shared { uri: "content://x".into() });
    store.save(&shared).unwrap();
    let setup = setup_with(store);
    let id = setup.library.add_book();
    setup.cache.put(&clip_key(&id, 0), b"twelve bytes").unwrap();
    setup.cache.put(&clip_key(&id_for(7), 0), b"abc").unwrap();

    let usage = GetStorageUsage::new(setup.library.reader(), setup.cache.clone(), setup.jobs.clone(), setup.exporter.clone()).run(&ctx(), ()).unwrap();

    let pdf_bytes = crate::documents::fakes::PDF_BYTES.len() as u64;
    assert_eq!((usage.library_documents, usage.library_bytes), (1, pdf_bytes));
    assert_eq!((usage.cache_bytes, usage.export_files, usage.export_bytes), (15, 1, 300));
    assert_eq!(usage.documents, [DocumentUsage { document_id: id.clone(), library_bytes: pdf_bytes, cache_bytes: 12 }]);
    assert_eq!(setup.cache.refreshes(), 1);

    remove(&setup, &id).unwrap();
    let after = GetStorageUsage::new(setup.library.reader(), setup.cache.clone(), setup.jobs.clone(), setup.exporter.clone()).run(&ctx(), ()).unwrap();
    assert!(after.documents.is_empty(), "removed books are left out");
}

#[test]
fn clearing_the_cache_deletes_every_clip() {
    let setup = setup();
    setup.cache.put(&clip_key(&id_for(1), 0), b"clip").unwrap();
    setup.cache.put(&clip_key(&id_for(2), 0), b"clip").unwrap();

    let eviction = ClearAudioCache::new(setup.cache.clone()).run(&ctx(), ()).unwrap();

    assert_eq!((eviction.evicted_clips(), eviction.freed_bytes), (2, 8));
    assert!(setup.cache.keys().is_empty());
}

/// Without a speech engine, exports are still loaded and listed; queued ones wait for an engine.
#[test]
fn without_an_engine_exports_are_kept_and_listed() {
    let store = Arc::new(MemoryRecordStore::default());
    let mut queued = record("queued", id_for(1), ExportStatus::Paused, None);
    queued.export.paused_by = Some(lectrice_domain::PauseReason::System);
    store.save(&queued).unwrap();
    let setup = setup_with(store);

    let active = setup.jobs.resume_all().unwrap();

    assert_eq!((active.len(), active[0].status, active[0].waiting_for_engine), (1, ExportStatus::Queued, true));
}

/// A book removed just before the app stopped is purged when the app starts, with its exports kept; assembling alone
/// (an engine reload) keeps it, so its undo still works.
#[test]
fn removed_books_are_purged_at_app_start_but_not_by_assembling() {
    let library = Library::new(1);
    let id = library.add_book();
    library.store.set_removed(&id, Some(at(5))).unwrap();
    let store = Arc::new(MemoryRecordStore::default());
    store.save(&record("kept", id.clone(), ExportStatus::Done, Some(1))).unwrap();
    let cache = Arc::new(MemoryCache::default());
    let adapters = Adapters {
        hasher: Arc::new(FakeHasher),
        pdf_source: library.source.clone(),
        cleaner: library.cleaner.clone(),
        splitter: Arc::new(FakeSplitter),
        reader: library.reader(),
        writer: library.writer(),
        clock: library.clock(),
        speech_engine: None,
        speech_engine_load_ms: None,
        audio_cache: cache.clone(),
        audio_retention: cache,
        audio_encoder: Arc::new(CountingEncoder),
        audio_decoder: Arc::new(CountingDecoder),
        voice_samples: Arc::new(NoSamples),
        exporter: Arc::new(FakeExporter::default()),
        export_records: store.clone(),
        time_stretch: None,
    };
    let settings = Settings {
        import_max_bytes: 1024,
        default_voice: VoiceId::new("af_heart"),
        read_ahead: ReadAheadSettings { read_ahead_minutes: 1, keep_minutes_per_document: 60 },
        export: ExportSettings::default(),
    };

    let services = assemble(adapters, settings).unwrap();
    assert!(library.store.contains(&id));

    purge_removed_at_start(&services).unwrap();

    assert!(!library.store.contains(&id));
    assert!(store.get(&ExportId::new("kept")).is_some());
    assert!(services.speech.is_none() && services.exports.is_none());
    assert_eq!(services.documents.current_cleaner_version(), 1);
    assert_eq!(services.library.list_all_exports.run(&ctx(), ()).unwrap().len(), 1);
}

struct NoSamples;

impl lectrice_domain::VoiceSampleCache for NoSamples {
    fn get(&self, _key: &lectrice_domain::VoiceSampleKey) -> lectrice_domain::AppResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn put(&self, _key: &lectrice_domain::VoiceSampleKey, _bytes: &[u8]) -> lectrice_domain::AppResult<()> {
        Ok(())
    }
}
