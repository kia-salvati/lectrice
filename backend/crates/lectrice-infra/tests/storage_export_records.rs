use lectrice_domain::{AudioExport, DocumentId, ErrorName, ExportFormat, ExportId, ExportRecord, ExportRecordStore, ExportStatus, Timestamp, VoiceId};
use lectrice_infra::FsExportRecordStore;
use tempfile::TempDir;

fn record(id: &str, status: ExportStatus) -> ExportRecord {
    ExportRecord {
        export: AudioExport {
            id: ExportId::new(id),
            document_id: DocumentId::parse(&format!("{:064x}", 3)).unwrap(),
            voice_id: VoiceId::new("af_heart"),
            format: ExportFormat::M4b,
            status,
            sentences_done: 4,
            sentences_total: 9,
            estimated_seconds_left: None,
        },
        read_skipped: true,
        created_at: Timestamp::from_timestamp(1_000, 0).unwrap(),
        file: None,
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
fn path_like_ids_are_refused() {
    let data_dir = TempDir::new().unwrap();
    let store = FsExportRecordStore::new(data_dir.path());

    assert_eq!(store.save(&record("../evil", ExportStatus::Queued)).unwrap_err().primary(), ErrorName::StorageFailed);
    assert_eq!(store.delete(&ExportId::new("a/b")).unwrap_err().primary(), ErrorName::StorageFailed);
}
