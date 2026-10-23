//! The export job state machine with fakes: queued → running → done, cancel, failure, restart, reuse of cached clips.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lectrice_domain::{
    AudioCache, AudioEncoder, AudioExport, AudioKey, DocumentId, ErrorName, ExportFormat, ExportId, ExportRecord, ExportRecordStore, ExportStatus, SentenceId,
    SpeechRequest, VoiceId,
};

use super::fakes::{CountingDecoder, Event, FakeExporter, MemoryRecordStore};
use super::*;
use crate::documents::fakes::{Library, at, id_for};
use crate::speech::fakes::{CountingEncoder, ENGINE_ID, GatedEngine, MemoryCache, SAMPLE_RATE};
use crate::speech::{SpeechJob, SpeechPriority, SpeechWorker};
use crate::{RequestContext, UseCase};

const WAIT_LIMIT: Duration = Duration::from_secs(5);
/// `GatedEngine` makes 100 samples per character.
const SAMPLES_PER_CHAR: usize = 100;
/// The fake book's playback order with `readSkipped` off: heading, two sentences, the code cue.
const SPOKEN: [&str; 4] = ["Intro", "One.", "Two.", "Code example."];

struct Setup {
    library: Library,
    engine: Arc<GatedEngine>,
    cache: Arc<MemoryCache>,
    exporter: Arc<FakeExporter>,
    store: Arc<MemoryRecordStore>,
    worker: Arc<SpeechWorker>,
    jobs: Arc<ExportJobs>,
    start: StartAudioExport,
}

fn setup() -> Setup {
    setup_with(GatedEngine::open(), Arc::default())
}

fn setup_with(engine: GatedEngine, store: Arc<MemoryRecordStore>) -> Setup {
    let library = Library::new(1);
    let engine = Arc::new(engine);
    let cache = Arc::new(MemoryCache::default());
    let exporter = Arc::new(FakeExporter::default());
    let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
    let deps = ExportDeps {
        worker: worker.clone(),
        engine: engine.clone(),
        reader: library.reader(),
        cache: cache.clone(),
        decoder: Arc::new(CountingDecoder),
        exporter: exporter.clone(),
        store: store.clone(),
        clock: library.clock(),
    };
    let settings = ExportSettings::default();
    let jobs = Arc::new(ExportJobs::start(deps, settings).unwrap());
    let start = StartAudioExport::new(jobs.clone(), engine.clone(), library.reader(), exporter.clone(), settings);
    Setup { library, engine, cache, exporter, store, worker, jobs, start }
}

fn ctx() -> RequestContext {
    RequestContext::new("test")
}

fn start(setup: &Setup, document_id: &DocumentId, format: ExportFormat) -> lectrice_domain::AppResult<AudioExport> {
    let args = StartAudioExportArgs { document_id: document_id.clone(), voice_id: VoiceId::new("af_heart"), format, read_skipped: false };
    setup.start.run(&ctx(), args)
}

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < WAIT_LIMIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_status(setup: &Setup, id: &ExportId, status: ExportStatus) -> AudioExport {
    wait_for(&format!("{status:?}"), || setup.jobs.get(id).is_some_and(|export| export.status == status));
    setup.jobs.get(id).unwrap()
}

fn appended_for(texts: &[&str]) -> Vec<Event> {
    texts.iter().map(|text| Event::Append(text.len() * SAMPLES_PER_CHAR)).collect()
}

fn key_of(id: &DocumentId, sentence: u32) -> AudioKey {
    AudioKey { document_id: id.clone(), cleaner_version: 1, engine_id: ENGINE_ID.into(), voice_id: VoiceId::new("af_heart"), sentence_id: SentenceId(sentence) }
}

#[test]
fn a_job_goes_from_queued_to_done_and_streams_every_sentence_with_its_chapter() {
    let setup = setup();
    let id = setup.library.add_book();

    let queued = start(&setup, &id, ExportFormat::M4b).unwrap();
    assert_eq!((queued.status, queued.sentences_done, queued.sentences_total, queued.estimated_seconds_left), (ExportStatus::Queued, 0, 4, None));

    let done = wait_for_status(&setup, &queued.id, ExportStatus::Done);
    assert_eq!((done.sentences_done, done.estimated_seconds_left), (4, Some(0)));
    let mut expected = vec![Event::Begin { title: "Fake Book".into(), artist: "Heart".into(), sample_rate: SAMPLE_RATE }, Event::Chapter("Intro".into())];
    expected.extend(appended_for(&SPOKEN));
    expected.push(Event::Finish);
    assert_eq!(setup.exporter.events(), expected);
    assert_eq!(setup.engine.spoken(), SPOKEN);
    assert_eq!(setup.cache.puts(), 0, "export audio goes to the encoder, not the cache");

    let saved = setup.store.get(&queued.id).unwrap();
    assert_eq!(saved.export, done);
    assert!(saved.file.unwrap().ends_with("af_heart.m4b"));
    let download = GetAudioExportFile::new(setup.jobs.clone(), setup.library.reader(), setup.engine.clone()).run(&ctx(), queued.id).unwrap();
    assert_eq!((download.title.as_str(), download.voice_name.as_str(), download.format), ("Fake Book", "Heart", ExportFormat::M4b));
}

/// W finding: cached sentences finish at once, so they must not make the time left look tiny.
#[test]
fn the_time_left_is_unknown_until_a_sentence_was_synthesized() {
    let setup = setup_with(GatedEngine::new("Two."), Arc::default());
    let id = setup.library.add_book();
    let cached = CountingEncoder.encode(&lectrice_domain::Audio { samples: vec![0.0; 777], sample_rate: SAMPLE_RATE }).unwrap();
    for sentence in [0, 1] {
        setup.cache.put(&key_of(&id, sentence), &cached).unwrap();
    }

    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();
    wait_for("the cached sentences", || setup.jobs.get(&export.id).is_some_and(|export| export.sentences_done == 2));

    assert_eq!(setup.jobs.get(&export.id).unwrap().estimated_seconds_left, None);
    setup.engine.release();
    assert_eq!(wait_for_status(&setup, &export.id, ExportStatus::Done).estimated_seconds_left, Some(0));
}

#[test]
fn cached_clips_are_reused_instead_of_synthesized() {
    let setup = setup();
    let id = setup.library.add_book();
    let cached = CountingEncoder.encode(&lectrice_domain::Audio { samples: vec![0.0; 777], sample_rate: SAMPLE_RATE }).unwrap();
    setup.cache.put(&key_of(&id, 1), &cached).unwrap();

    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(setup.engine.spoken(), ["Intro", "Two.", "Code example."]);
    assert!(setup.exporter.events().contains(&Event::Append(777)));
}

#[test]
fn read_skipped_exports_the_code_instead_of_its_cue() {
    let setup = setup();
    let id = setup.library.add_book();
    let args = StartAudioExportArgs { document_id: id, voice_id: VoiceId::new("af_heart"), format: ExportFormat::Mp3, read_skipped: true };

    let export = setup.start.run(&ctx(), args).unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(setup.engine.spoken(), ["Intro", "One.", "Two.", "let x = 1;"]);
}

#[test]
fn cancelling_a_running_job_stops_it_without_waiting_for_the_engine() {
    let setup = setup_with(GatedEngine::new("Two."), Arc::default());
    let id = setup.library.add_book();
    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();
    wait_for("running", || setup.jobs.get(&export.id).unwrap().status == ExportStatus::Running);

    assert_eq!(CancelAudioExport::new(setup.jobs.clone()).run(&ctx(), export.id.clone()).unwrap(), ExportStatus::Cancelled);

    wait_for("the writer to be abandoned", || setup.exporter.events().contains(&Event::Abandoned));
    let cancelled = setup.jobs.get(&export.id).unwrap();
    assert_eq!((cancelled.status, cancelled.estimated_seconds_left), (ExportStatus::Cancelled, None));
    assert_eq!(setup.store.get(&export.id).unwrap().export.status, ExportStatus::Cancelled);
    setup.engine.release();
    let ticket = setup.worker.submit(SpeechJob { key: key_of(&id, 99), request: speech("after"), priority: SpeechPriority::Now });
    ticket.wait().unwrap();
    assert!(!setup.engine.spoken().contains(&"Code example.".to_string()), "queued export sentences were withdrawn");
    assert!(!setup.exporter.events().contains(&Event::Finish));
}

#[test]
fn cancelling_a_queued_job_means_it_never_starts() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();
    let first = start(&setup, &id, ExportFormat::Mp3).unwrap();
    let second = start(&setup, &id, ExportFormat::M4b).unwrap();
    setup.engine.wait_until_blocked();

    assert_eq!(setup.jobs.get(&second.id).unwrap().status, ExportStatus::Queued);
    CancelAudioExport::new(setup.jobs.clone()).run(&ctx(), second.id.clone()).unwrap();
    setup.engine.release();

    wait_for_status(&setup, &first.id, ExportStatus::Done);
    assert_eq!(setup.jobs.get(&second.id).unwrap().status, ExportStatus::Cancelled);
    assert_eq!(setup.exporter.begins(), 1);
    assert_eq!(CancelAudioExport::new(setup.jobs.clone()).run(&ctx(), first.id).unwrap(), ExportStatus::Done, "finished jobs stay as they are");
}

#[test]
fn an_encoder_failure_fails_the_job() {
    let setup = setup();
    setup.exporter.fail_appends.store(true, Ordering::SeqCst);
    let id = setup.library.add_book();

    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();

    let failed = wait_for_status(&setup, &export.id, ExportStatus::Failed);
    assert_eq!(failed.estimated_seconds_left, None);
    assert!(setup.exporter.events().contains(&Event::Abandoned));
    let file = GetAudioExportFile::new(setup.jobs.clone(), setup.library.reader(), setup.engine.clone()).run(&ctx(), export.id);
    assert_eq!(file.unwrap_err().primary(), ErrorName::ExportNotReady);
}

fn saved(id: &str, status: ExportStatus) -> ExportRecord {
    ExportRecord {
        export: AudioExport {
            id: ExportId::new(id),
            document_id: id_for(1),
            voice_id: VoiceId::new("af_heart"),
            format: ExportFormat::Mp3,
            status,
            sentences_done: 2,
            sentences_total: 4,
            estimated_seconds_left: Some(9),
        },
        read_skipped: false,
        created_at: at(50),
        file: None,
    }
}

#[test]
fn after_a_restart_interrupted_jobs_are_failed_and_finished_ones_are_still_listed() {
    let store = Arc::new(MemoryRecordStore::default());
    for (id, status) in [("a", ExportStatus::Done), ("b", ExportStatus::Running), ("c", ExportStatus::Queued), ("d", ExportStatus::Cancelled)] {
        store.save(&saved(id, status)).unwrap();
    }

    let setup = setup_with(GatedEngine::open(), store);
    setup.library.add_book();

    let listed = ListAudioExports::new(setup.jobs.clone(), setup.library.reader()).run(&ctx(), id_for(1)).unwrap();
    let statuses: Vec<(&str, ExportStatus)> = listed.iter().map(|export| (export.id.as_str(), export.status)).collect();
    assert_eq!(statuses, [("d", ExportStatus::Cancelled), ("c", ExportStatus::Failed), ("b", ExportStatus::Failed), ("a", ExportStatus::Done)]);
    assert_eq!(setup.store.get(&ExportId::new("b")).unwrap().export.estimated_seconds_left, None);
    assert_eq!(setup.store.get(&ExportId::new("c")).unwrap().export.status, ExportStatus::Failed);
    assert_eq!(setup.exporter.begins(), 0, "interrupted jobs are not resumed");
}

#[test]
fn starting_the_same_export_twice_returns_the_running_job() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();

    let first = start(&setup, &id, ExportFormat::Mp3).unwrap();
    let again = start(&setup, &id, ExportFormat::Mp3).unwrap();

    assert_eq!(again.id, first.id);
    setup.engine.release();
}

#[test]
fn a_newer_export_of_the_same_file_replaces_the_older_finished_one() {
    let setup = setup();
    let id = setup.library.add_book();
    let first = start(&setup, &id, ExportFormat::Mp3).unwrap();
    wait_for_status(&setup, &first.id, ExportStatus::Done);
    setup.library.clock.set(at(200));

    let second = start(&setup, &id, ExportFormat::Mp3).unwrap();
    wait_for_status(&setup, &second.id, ExportStatus::Done);

    let listed = ListAudioExports::new(setup.jobs.clone(), setup.library.reader()).run(&ctx(), id).unwrap();
    assert_eq!(listed.iter().map(|export| export.id.clone()).collect::<Vec<_>>(), [second.id]);
    assert!(setup.store.get(&first.id).is_none());
}

#[test]
fn exports_are_listed_newest_first() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();
    let mp3 = start(&setup, &id, ExportFormat::Mp3).unwrap();
    let m4b = start(&setup, &id, ExportFormat::M4b).unwrap();

    let listed = ListAudioExports::new(setup.jobs.clone(), setup.library.reader()).run(&ctx(), id).unwrap();

    assert_eq!(listed.iter().map(|export| export.id.clone()).collect::<Vec<_>>(), [m4b.id, mp3.id]);
    setup.engine.release();
}

#[test]
fn bad_requests_are_refused_before_a_job_is_made() {
    let setup = setup();
    let id = setup.library.add_book();
    let unknown_voice = StartAudioExportArgs { document_id: id.clone(), voice_id: VoiceId::new("zz_nobody"), format: ExportFormat::Mp3, read_skipped: false };

    assert_eq!(setup.start.run(&ctx(), unknown_voice).unwrap_err().primary(), ErrorName::VoiceUnknown);
    assert_eq!(start(&setup, &id_for(9), ExportFormat::Mp3).unwrap_err().primary(), ErrorName::DocumentNotFound);
    setup.exporter.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(start(&setup, &id, ExportFormat::Mp3).unwrap_err().primary(), ErrorName::ExporterUnavailable);

    assert!(setup.jobs.list(&id).is_empty());
    let missing = ExportId::new("nope");
    assert_eq!(GetAudioExport::new(setup.jobs.clone()).run(&ctx(), missing.clone()).unwrap_err().primary(), ErrorName::ExportNotFound);
    assert_eq!(CancelAudioExport::new(setup.jobs.clone()).run(&ctx(), missing).unwrap_err().primary(), ErrorName::ExportNotFound);
    let unknown_document = ListAudioExports::new(setup.jobs.clone(), setup.library.reader()).run(&ctx(), id_for(9));
    assert_eq!(unknown_document.unwrap_err().primary(), ErrorName::DocumentNotFound);
}

fn speech(text: &str) -> SpeechRequest {
    SpeechRequest { text: text.into(), voice_id: VoiceId::new("af_heart") }
}

#[test]
fn playback_goes_before_queued_export_sentences() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();
    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();

    let listener = setup.worker.submit(SpeechJob { key: key_of(&id, 50), request: speech("Listener."), priority: SpeechPriority::Now });
    setup.engine.release();
    listener.wait().unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(setup.engine.spoken(), ["Intro", "Listener.", "One.", "Two.", "Code example."]);
}
