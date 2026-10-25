use lectrice_domain::{
    AudioExport, ChapterMark, DocumentId, ErrorName, ExportFormat, ExportId, ExportPart, ExportRange, ExportRecord, ExportRecordStore, ExportSpeed,
    ExportStage, ExportStatus, PauseReason, SentenceId, Timestamp, VoiceId,
};
use lectrice_infra::FsExportRecordStore;
use tempfile::TempDir;

fn record(id: &str, status: ExportStatus) -> ExportRecord {
    ExportRecord {
        export: AudioExport {
            id: ExportId::new(id),
            document_id: DocumentId::parse(&format!("{:064x}", 3)).unwrap(),
            voice_id: VoiceId::new("af_heart"),
            format: ExportFormat::M4b,
            speed: ExportSpeed::from_percent(150).unwrap(),
            engine_id: "engine-a".into(),
            range: Some(ExportRange { from_sentence: SentenceId(2), to_sentence: SentenceId(8) }),
            range_label: Some("Ch. 2".into()),
            title: "Book".into(),
            status,
            paused_by: Some(PauseReason::StorageFull),
            stage: ExportStage::Assemble,
            error: Some(ErrorName::StorageFull),
            sentences_done: 4,
            sentences_total: 9,
            parts_done: 0,
            parts_total: 2,
            estimated_seconds_left: None,
            file_bytes: None,
            location: None,
            waiting_for_engine: false,
        },
        read_skipped: true,
        created_at: Timestamp::from_timestamp(1_000, 0).unwrap(),
        file: None,
        parts: Vec::new(),
        chapters: Vec::new(),
        source_samples: 12_000,
    }
}

#[test]
fn records_round_trip_and_can_be_replaced_and_deleted() {
    let data_dir = TempDir::new().unwrap();
    let store = FsExportRecordStore::new(data_dir.path());
    assert!(store.load_all().unwrap().is_empty());

    store.save(&record("a1", ExportStatus::Running)).unwrap();
    store.save(&record("b2", ExportStatus::Queued)).unwrap();
    let mut done = record("a1", ExportStatus::Done);
    done.file = Some(data_dir.path().join("exports/x/af_heart.m4b"));
    store.save(&done).unwrap();

    let mut loaded = store.load_all().unwrap();
    loaded.sort_by(|a, b| a.export.id.as_str().cmp(b.export.id.as_str()));
    assert_eq!(loaded, [done, record("b2", ExportStatus::Queued)]);
    assert!(data_dir.path().join("exports/jobs/a1.json").is_file());

    store.delete(&ExportId::new("b2")).unwrap();
    store.delete(&ExportId::new("never-saved")).unwrap();
    assert_eq!(store.load_all().unwrap().len(), 1);
}

#[test]
fn a_paused_record_keeps_its_parts_and_chapters() {
    let data_dir = TempDir::new().unwrap();
    let store = FsExportRecordStore::new(data_dir.path());
    let mut paused = record("p1", ExportStatus::Paused);
    paused.export.parts_done = 1;
    paused.parts = vec![ExportPart { index: 0, path: data_dir.path().join("exports/parts/p1/part-0000.wav"), samples: 48_000, sample_rate: 24_000 }];
    paused.chapters = vec![ChapterMark { title: "Chapter 1".into(), start_ms: 0 }];

    store.save(&paused).unwrap();

    assert_eq!(store.load_all().unwrap(), [paused]);
}

/// Records written by v0.1 have no part fields.
#[test]
fn records_from_before_parts_still_load() {
    let data_dir = TempDir::new().unwrap();
    let jobs = data_dir.path().join("exports/jobs");
    std::fs::create_dir_all(&jobs).unwrap();
    let id = format!("{:064x}", 3);
    let old = format!(
        r#"{{"export":{{"id":"old","documentId":"{id}","voiceId":"af_heart","format":"m4b","status":"done","sentencesDone":9,"sentencesTotal":9,"estimatedSecondsLeft":0}},"readSkipped":false,"createdAt":"1970-01-01T00:16:40Z"}}"#
    );
    std::fs::write(jobs.join("old.json"), old).unwrap();

    let loaded = FsExportRecordStore::new(data_dir.path()).load_all().unwrap();

    assert_eq!((loaded[0].export.parts_done, loaded[0].export.parts_total, loaded[0].parts.len()), (0, 0, 0));
}

#[test]
fn path_like_ids_are_refused() {
    let data_dir = TempDir::new().unwrap();
    let store = FsExportRecordStore::new(data_dir.path());

    assert_eq!(store.save(&record("../evil", ExportStatus::Queued)).unwrap_err().primary(), ErrorName::StorageFailed);
    assert_eq!(store.delete(&ExportId::new("a/b")).unwrap_err().primary(), ErrorName::StorageFailed);
}
