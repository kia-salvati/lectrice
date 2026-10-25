//! The export job state machine with fakes: queued → running → done, pause and resume by parts, cancel, failure,
//! restart, reuse of cached clips.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lectrice_domain::{
    AudioCache, AudioEncoder, AudioExport, AudioKey, Block, BlockKind, Document, DocumentId, DocumentMeta, DocumentWriter, ErrorName, ExportFormat, ExportId,
    ExportLocation, ExportRange, ExportRecord, ExportRecordStore, ExportSpeed, ExportStage, ExportStatus, PageLabel, PauseReason, Sentence, SentenceId,
    SpeechRequest, TocEntry, VoiceId,
};

use super::fakes::{CountingDecoder, Event, FakeExporter, MemoryRecordStore, ShorteningStretch};
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
    stretch: Arc<ShorteningStretch>,
    worker: Arc<SpeechWorker>,
    jobs: Arc<ExportJobs>,
    start: StartAudioExport,
    estimate: EstimateAudioExport,
}

/// Every sentence is a part of its own: the fake book becomes four parts, the first opened by the "Intro" mark.
const ONE_SENTENCE_PARTS: ExportSettings = ExportSettings { max_sentences: None, part_minutes: 0, short_file_names: false };

fn setup() -> Setup {
    setup_with(GatedEngine::open(), Arc::default())
}

fn setup_with(engine: GatedEngine, store: Arc<MemoryRecordStore>) -> Setup {
    setup_with_settings(engine, store, ExportSettings::default())
}

fn setup_with_settings(engine: GatedEngine, store: Arc<MemoryRecordStore>, settings: ExportSettings) -> Setup {
    let library = Library::new(1);
    let engine = Arc::new(engine);
    let cache = Arc::new(MemoryCache::default());
    let exporter = Arc::new(FakeExporter::default());
    let stretch = Arc::new(ShorteningStretch::default());
    let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
    let deps = ExportDeps {
        speech: Some(ExportSpeech { worker: worker.clone(), engine: engine.clone() }),
        reader: library.reader(),
        cache: cache.clone(),
        decoder: Arc::new(CountingDecoder),
        exporter: exporter.clone(),
        store: store.clone(),
        clock: library.clock(),
        time_stretch: Some(stretch.clone()),
    };
    let jobs = Arc::new(ExportJobs::start(deps, settings).unwrap());
    let start_deps = StartAudioExportDeps {
        jobs: jobs.clone(),
        engine: engine.clone(),
        reader: library.reader(),
        exporter: exporter.clone(),
        time_stretch: Some(stretch.clone()),
    };
    let start = StartAudioExport::new(start_deps, settings);
    let estimate = EstimateAudioExport::new(engine.clone(), library.reader(), cache.clone(), settings);
    Setup { library, engine, cache, exporter, store, stretch, worker, jobs, start, estimate }
}

fn ctx() -> RequestContext {
    RequestContext::new("test")
}

fn args(document_id: &DocumentId, format: ExportFormat) -> StartAudioExportArgs {
    StartAudioExportArgs {
        document_id: document_id.clone(),
        voice_id: VoiceId::new("af_heart"),
        format,
        read_skipped: false,
        speed: ExportSpeed::NORMAL,
        engine_id: None,
        range: None,
        range_label: None,
    }
}

fn start(setup: &Setup, document_id: &DocumentId, format: ExportFormat) -> lectrice_domain::AppResult<AudioExport> {
    setup.start.run(&ctx(), args(document_id, format))
}

fn speed(percent: u16) -> ExportSpeed {
    ExportSpeed::from_percent(percent).unwrap()
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
    assert_eq!((queued.parts_done, queued.parts_total), (0, 1));

    let done = wait_for_status(&setup, &queued.id, ExportStatus::Done);
    assert_eq!((done.sentences_done, done.estimated_seconds_left, done.parts_done), (4, Some(0), 1));
    let mut expected = vec![Event::BeginPart { index: 0, sample_rate: SAMPLE_RATE }];
    expected.extend(appended_for(&SPOKEN));
    expected.push(Event::FinishPart(0));
    expected.push(Event::Assemble { title: "Fake Book".into(), artist: "Heart".into(), parts: vec![0], chapters: vec![("Intro".into(), 0)] });
    expected.push(Event::Discard);
    assert_eq!(setup.exporter.events(), expected);
    assert_eq!(setup.engine.spoken(), SPOKEN);
    assert_eq!(setup.cache.puts(), 0, "export audio goes to the encoder, not the cache");

    assert!(setup.stretch.speeds().is_empty(), "1x needs no stretcher");

    let saved = setup.store.get(&queued.id).unwrap();
    assert_eq!(saved.export, done);
    assert!(saved.parts.is_empty() && saved.chapters.is_empty(), "the parts are deleted once the file is made");
    assert!(saved.file.unwrap().ends_with("af_heart.fake-engine.100.m4b"));
    assert_eq!((done.file_bytes, done.title.as_str(), done.engine_id.as_str()), (Some(1), "Fake Book", ENGINE_ID));
    assert!(matches!(done.location, Some(ExportLocation::AppStorage { .. })));
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
    let export = setup.start.run(&ctx(), StartAudioExportArgs { read_skipped: true, ..args(&id, ExportFormat::Mp3) }).unwrap();
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

    wait_for("the parts to be deleted", || setup.exporter.events().contains(&Event::Discard));
    assert!(setup.exporter.events().contains(&Event::Abandoned(0)));
    let cancelled = setup.jobs.get(&export.id).unwrap();
    assert_eq!((cancelled.status, cancelled.estimated_seconds_left), (ExportStatus::Cancelled, None));
    assert_eq!(setup.store.get(&export.id).unwrap().export.status, ExportStatus::Cancelled);
    setup.engine.release();
    let ticket = setup.worker.submit(SpeechJob { key: key_of(&id, 99), request: speech("after"), priority: SpeechPriority::Now });
    ticket.wait().unwrap();
    assert!(!setup.engine.spoken().contains(&"Code example.".to_string()), "queued export sentences were withdrawn");
    assert!(!setup.exporter.events().iter().any(|event| matches!(event, Event::Assemble { .. })));
}

/// Gated on "Two.", the third of four one-sentence parts: parts 0 and 1 are finished when the engine blocks.
fn paused_after_two_parts() -> (Setup, AudioExport) {
    let setup = setup_with_settings(GatedEngine::new("Two."), Arc::default(), ONE_SENTENCE_PARTS);
    let id = setup.library.add_book();
    let export = start(&setup, &id, ExportFormat::M4b).unwrap();
    assert_eq!(export.parts_total, 4);
    setup.engine.wait_until_blocked();

    let paused = PauseAudioExport::new(setup.jobs.clone()).run(&ctx(), PauseAudioExportArgs { id: export.id.clone(), reason: PauseReason::System }).unwrap();

    assert_eq!(paused.status, ExportStatus::Paused);
    wait_for("the open part to be abandoned", || setup.exporter.events().contains(&Event::Abandoned(2)));
    let saved = setup.store.get(&export.id).unwrap();
    assert_eq!((saved.export.status, saved.export.parts_done), (ExportStatus::Paused, 2));
    assert_eq!(saved.parts.iter().map(|part| part.index).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(saved.chapters.iter().map(|chapter| chapter.title.as_str()).collect::<Vec<_>>(), ["Intro"]);
    assert!(!setup.exporter.events().contains(&Event::Discard), "pausing keeps the parts");
    setup.engine.release();
    setup.exporter.clear();
    (setup, export)
}

#[test]
fn a_resumed_export_continues_at_the_first_unfinished_part() {
    let (setup, export) = paused_after_two_parts();

    let active = ResumeAudioExports::new(setup.jobs.clone()).run(&ctx(), ()).unwrap();

    assert_eq!((active.len(), &active[0].id), (1, &export.id));
    let done = wait_for_status(&setup, &export.id, ExportStatus::Done);
    assert_eq!((done.sentences_done, done.parts_done), (4, 4));
    let events = setup.exporter.events();
    assert_eq!(events[0], Event::BeginPart { index: 2, sample_rate: SAMPLE_RATE }, "{events:?}");
    let assembled = Event::Assemble { title: "Fake Book".into(), artist: "Heart".into(), parts: vec![0, 1, 2, 3], chapters: vec![("Intro".into(), 0)] };
    assert!(events.contains(&assembled), "{events:?}");
    assert_eq!(setup.engine.spoken().iter().filter(|text| text.as_str() == "One.").count(), 1, "finished parts are not made again");
}

#[test]
fn starting_a_paused_export_again_continues_the_same_job() {
    let (setup, export) = paused_after_two_parts();

    let again = start(&setup, &export.document_id, ExportFormat::M4b).unwrap();

    assert_eq!((again.id.clone(), again.status), (export.id.clone(), ExportStatus::Queued));
    assert_eq!(wait_for_status(&setup, &export.id, ExportStatus::Done).parts_done, 4);
    assert_eq!(setup.exporter.events()[0], Event::BeginPart { index: 2, sample_rate: SAMPLE_RATE });
}

#[test]
fn cancelling_a_paused_export_deletes_its_parts() {
    let (setup, export) = paused_after_two_parts();

    assert_eq!(CancelAudioExport::new(setup.jobs.clone()).run(&ctx(), export.id.clone()).unwrap(), ExportStatus::Cancelled);

    assert_eq!(setup.exporter.events(), [Event::Discard]);
    let saved = setup.store.get(&export.id).unwrap();
    assert_eq!(saved.export.status, ExportStatus::Cancelled);
    assert!(saved.parts.is_empty());
    assert!(ResumeAudioExports::new(setup.jobs.clone()).run(&ctx(), ()).unwrap().is_empty());
}

#[test]
fn a_shutdown_pauses_the_running_export_and_keeps_its_parts() {
    let setup = setup_with_settings(GatedEngine::new("Two."), Arc::default(), ONE_SENTENCE_PARTS);
    let id = setup.library.add_book();
    let export = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();
    let Setup { jobs, start, store, engine, .. } = setup;

    drop(start);
    drop(jobs);

    let saved = store.get(&export.id).unwrap();
    assert_eq!((saved.export.status, saved.parts.len()), (ExportStatus::Paused, 2));
    engine.release();
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
    assert!(setup.exporter.events().contains(&Event::Abandoned(0)));
    let file = GetAudioExportFile::new(setup.jobs.clone(), setup.library.reader(), setup.engine.clone()).run(&ctx(), export.id);
    assert_eq!(file.unwrap_err().primary(), ErrorName::ExportNotReady);
}

/// A record as v0.1 / early v0.2 saved it: no engine id, no pause reason, no location.
fn saved(id: &str, status: ExportStatus) -> ExportRecord {
    ExportRecord {
        export: AudioExport {
            id: ExportId::new(id),
            document_id: id_for(1),
            voice_id: VoiceId::new("af_heart"),
            format: ExportFormat::Mp3,
            speed: ExportSpeed::NORMAL,
            engine_id: String::new(),
            range: None,
            range_label: None,
            title: String::new(),
            status,
            paused_by: None,
            stage: ExportStage::Parts,
            error: None,
            sentences_done: 2,
            sentences_total: 4,
            parts_done: 0,
            parts_total: 1,
            estimated_seconds_left: Some(9),
            file_bytes: None,
            location: None,
            waiting_for_engine: false,
        },
        read_skipped: false,
        created_at: at(50),
        file: (status == ExportStatus::Done).then(|| "/exports/old/af_heart.mp3".into()),
        parts: Vec::new(),
        chapters: Vec::new(),
        source_samples: 0,
    }
}

#[test]
fn after_a_restart_interrupted_jobs_are_paused_and_finished_ones_are_still_listed() {
    let store = Arc::new(MemoryRecordStore::default());
    for (id, status) in [("a", ExportStatus::Done), ("b", ExportStatus::Running), ("c", ExportStatus::Queued), ("d", ExportStatus::Cancelled)] {
        store.save(&saved(id, status)).unwrap();
    }

    let setup = setup_with(GatedEngine::open(), store);
    setup.library.add_book();

    let listed = ListAudioExports::new(setup.jobs.clone(), setup.library.reader()).run(&ctx(), id_for(1)).unwrap();
    let statuses: Vec<(&str, ExportStatus)> = listed.iter().map(|export| (export.id.as_str(), export.status)).collect();
    assert_eq!(statuses, [("d", ExportStatus::Cancelled), ("c", ExportStatus::Paused), ("b", ExportStatus::Paused), ("a", ExportStatus::Done)]);
    assert!(listed.iter().all(|export| export.engine_id == ENGINE_ID), "old records get the loaded engine");
    assert!(listed[1..3].iter().all(|export| export.paused_by == Some(PauseReason::System)));
    assert_eq!(listed[3].location, Some(ExportLocation::AppStorage { path: "/exports/old/af_heart.mp3".into() }));
    assert_eq!(setup.store.get(&ExportId::new("b")).unwrap().export.estimated_seconds_left, None);
    assert_eq!(setup.store.get(&ExportId::new("c")).unwrap().export.status, ExportStatus::Paused);
    assert_eq!(setup.exporter.begins(), 0, "interrupted jobs wait for a resume");
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
    let unknown_voice = StartAudioExportArgs { voice_id: VoiceId::new("zz_nobody"), ..args(&id, ExportFormat::Mp3) };

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

const CHAPTERED_TEXTS: [&str; 6] = ["Alpha", "Aaaa.", "Beta", "Bbbbbb.", "Gamma", "Cc."];

/// Three chapters (a heading and a paragraph each) with a TOC, saved as document 2.
fn chaptered_book(library: &Library) -> DocumentId {
    let id = id_for(2);
    let sentence = |index: usize| Sentence { id: SentenceId(index as u32), text: CHAPTERED_TEXTS[index].into(), page_index: 1, speech_text: None };
    let block = |index: usize, kind| Block {
        id: index as u32,
        kind,
        level: None,
        page_index: 1,
        spoken: true,
        text: None,
        sentences: vec![sentence(index)],
        cue: None,
    };
    let toc = |index: usize| TocEntry {
        id: (index / 2) as u32,
        title: CHAPTERED_TEXTS[index].into(),
        level: 1,
        sentence_id: SentenceId(index as u32),
        page_index: 1,
    };
    let document = Document {
        id: id.clone(),
        title: "Chapter Book".into(),
        source_file_name: "chapters.pdf".into(),
        page_count: 1,
        cleaner_version: 1,
        pages: vec![PageLabel { index: 1, label: "ix".into() }],
        toc: vec![toc(0), toc(2), toc(4)],
        blocks: (0..CHAPTERED_TEXTS.len()).map(|index| block(index, if index % 2 == 0 { BlockKind::Heading } else { BlockKind::Paragraph })).collect(),
    };
    let now = at(100);
    library.store.save(&document, b"%PDF-", &DocumentMeta { added_at: now, last_opened_at: now, last_sentence_id: None }).unwrap();
    id
}

fn start_with(setup: &Setup, args: StartAudioExportArgs) -> AudioExport {
    setup.start.run(&ctx(), args).unwrap()
}

/// C10: chapter marks are timed from the 1x audio and divided by the speed; the stretched output is what the parts hold.
#[test]
fn at_two_times_the_chapter_marks_are_halved_and_the_parts_shorter() {
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), ONE_SENTENCE_PARTS);
    let id = chaptered_book(&setup.library);
    let normal = start(&setup, &id, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &normal.id, ExportStatus::Done);

    let fast = start_with(&setup, StartAudioExportArgs { speed: speed(200), ..args(&id, ExportFormat::M4b) });
    assert_ne!(fast.id, normal.id, "another speed is another file");
    wait_for_status(&setup, &fast.id, ExportStatus::Done);

    let chapters = setup.exporter.chapters();
    assert_eq!(chapters[0].iter().map(|(title, _)| title.as_str()).collect::<Vec<_>>(), ["Alpha", "Beta", "Gamma"]);
    for (one, two) in chapters[0].iter().zip(&chapters[1]) {
        assert!(two.1.abs_diff(one.1 / 2) <= 10, "{one:?} vs {two:?}");
    }
    assert!(chapters[0][1].1 > 0);
    assert_eq!(setup.stretch.speeds(), [200; 6], "one stretcher per part");
    let target = setup.exporter.assembled().pop().unwrap();
    assert_eq!((target.file_stem.as_str(), target.speed), ("af_heart.fake-engine.200", speed(200)));
    let half_alpha = "Alpha".len() * SAMPLES_PER_CHAR / 2;
    assert!(setup.exporter.events().contains(&Event::Append(half_alpha)));
}

/// Marks follow what the earlier parts really hold, so a stretcher's error per part never adds up over a book.
#[test]
fn chapter_marks_follow_the_real_length_of_the_earlier_parts() {
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), ONE_SENTENCE_PARTS);
    setup.stretch.flush_tail.store(SAMPLE_RATE as usize / 10, Ordering::SeqCst);
    let id = chaptered_book(&setup.library);

    let fast = start_with(&setup, StartAudioExportArgs { speed: speed(200), ..args(&id, ExportFormat::M4b) });
    wait_for_status(&setup, &fast.id, ExportStatus::Done);

    let mut part_starts_ms = Vec::new();
    let mut written = 0;
    for event in setup.exporter.events() {
        match event {
            Event::BeginPart { .. } => part_starts_ms.push((written as f64 * 1000.0 / f64::from(SAMPLE_RATE)).round() as u64),
            Event::Append(samples) => written += samples,
            _ => {}
        }
    }
    let marks: Vec<u64> = setup.exporter.chapters()[0].iter().map(|(_, start_ms)| *start_ms).collect();
    assert_eq!(marks.len(), 3);
    for mark in &marks {
        assert!(part_starts_ms.contains(mark), "{mark} ms is not a part start {part_starts_ms:?}");
    }
    assert!(marks[2] >= 400, "four 100 ms tails come before the third chapter");
}

/// A newer export of the same target doesn't drop one the user moved to shared storage: that is another file.
#[test]
fn an_export_moved_to_shared_storage_is_not_replaced() {
    let setup = setup();
    let id = setup.library.add_book();
    let first = start(&setup, &id, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &first.id, ExportStatus::Done);
    setup.jobs.mark_saved(&first.id, "content://media/3".into()).unwrap();
    setup.library.clock.set(at(200));

    let second = start(&setup, &id, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &second.id, ExportStatus::Done);

    assert_eq!(setup.jobs.list(&id).len(), 2);
    assert!(setup.store.get(&first.id).is_some());
}

/// C10: a resumed export takes its speed from the record.
#[test]
fn a_resumed_export_keeps_its_speed() {
    let setup = setup_with_settings(GatedEngine::new("Beta"), Arc::default(), ONE_SENTENCE_PARTS);
    let id = chaptered_book(&setup.library);
    let export = start_with(&setup, StartAudioExportArgs { speed: speed(150), ..args(&id, ExportFormat::M4b) });
    setup.engine.wait_until_blocked();
    let pause = PauseAudioExportArgs { id: export.id.clone(), reason: PauseReason::User };
    PauseAudioExport::new(setup.jobs.clone()).run(&ctx(), pause).unwrap();
    wait_for("the open part to be abandoned", || setup.exporter.events().contains(&Event::Abandoned(2)));
    setup.engine.release();

    let same = start_with(&setup, StartAudioExportArgs { speed: speed(150), ..args(&id, ExportFormat::M4b) });
    assert_eq!(same.id, export.id, "the same file continues the paused job");
    let done = wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(done.speed, speed(150));
    assert!(setup.stretch.speeds().iter().all(|percent| *percent == 150));
    let begins: Vec<u32> =
        setup.exporter.events().iter().filter_map(|event| if let Event::BeginPart { index, .. } = event { Some(*index) } else { None }).collect();
    assert_eq!(begins, [0, 1, 2, 2, 3, 4, 5], "part 2 is made again, the finished ones are not");
}

/// C10 same-target rule: another speed while one is paused is a new job.
#[test]
fn another_speed_of_a_paused_export_is_a_new_job() {
    let setup = setup_with_settings(GatedEngine::new("Beta"), Arc::default(), ONE_SENTENCE_PARTS);
    let id = chaptered_book(&setup.library);
    let first = start_with(&setup, StartAudioExportArgs { speed: speed(150), ..args(&id, ExportFormat::M4b) });
    setup.engine.wait_until_blocked();
    setup.jobs.pause(&first.id, PauseReason::User).unwrap();

    let other = start_with(&setup, StartAudioExportArgs { speed: speed(125), ..args(&id, ExportFormat::M4b) });

    assert_ne!(other.id, first.id);
    assert_eq!(setup.jobs.get(&first.id).unwrap().status, ExportStatus::Paused);
    setup.engine.release();
}

#[test]
fn a_baked_in_speed_needs_a_time_stretch() {
    let setup = setup();
    let id = setup.library.add_book();
    let deps = StartAudioExportDeps {
        jobs: setup.jobs.clone(),
        engine: setup.engine.clone(),
        reader: setup.library.reader(),
        exporter: setup.exporter.clone(),
        time_stretch: None,
    };
    let start = StartAudioExport::new(deps, ExportSettings::default());

    let error = start.run(&ctx(), StartAudioExportArgs { speed: speed(150), ..args(&id, ExportFormat::M4b) }).unwrap_err();

    assert_eq!(error.primary(), ErrorName::RequestInvalid);
    assert!(start.run(&ctx(), args(&id, ExportFormat::M4b)).is_ok());
}

#[test]
fn short_file_names_keep_the_1_0_name_for_whole_book_1x_exports() {
    let settings = ExportSettings { short_file_names: true, ..ExportSettings::default() };
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), settings);
    let id = setup.library.add_book();

    let export = start(&setup, &id, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(setup.exporter.assembled()[0].file_stem, "af_heart");
}

/// C12: an export for another engine waits in the queue; the loaded engine's exports run.
#[test]
fn an_export_for_another_engine_waits() {
    let setup = setup();
    let id = setup.library.add_book();

    let other = start_with(&setup, StartAudioExportArgs { engine_id: Some("other-engine".into()), ..args(&id, ExportFormat::M4b) });
    let loaded = start(&setup, &id, ExportFormat::M4b).unwrap();

    assert!(other.waiting_for_engine && !loaded.waiting_for_engine);
    wait_for_status(&setup, &loaded.id, ExportStatus::Done);
    let waiting = setup.jobs.get(&other.id).unwrap();
    assert_eq!((waiting.status, waiting.waiting_for_engine, waiting.engine_id.as_str()), (ExportStatus::Queued, true, "other-engine"));
    assert_eq!(setup.exporter.begins(), 1);
    assert!(!setup.store.get(&other.id).unwrap().export.waiting_for_engine, "never stored");
}

/// C24: out of space while writing a part pauses the export and keeps the finished parts; only the user resumes it.
#[test]
fn out_of_space_while_writing_pauses_and_keeps_the_finished_parts() {
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), ONE_SENTENCE_PARTS);
    *setup.exporter.full_from_part.lock().unwrap() = Some(2);
    let id = setup.library.add_book();

    let export = start(&setup, &id, ExportFormat::M4b).unwrap();

    let paused = wait_for_status(&setup, &export.id, ExportStatus::Paused);
    assert_eq!(
        (paused.paused_by, paused.error, paused.stage, paused.parts_done),
        (Some(PauseReason::StorageFull), Some(ErrorName::StorageFull), ExportStage::Parts, 2)
    );
    assert_eq!(setup.store.get(&export.id).unwrap().parts.iter().map(|part| part.index).collect::<Vec<_>>(), [0, 1]);
    assert!(!setup.exporter.events().contains(&Event::Discard));
    assert!(ResumeAudioExports::new(setup.jobs.clone()).run(&ctx(), ()).unwrap().is_empty(), "a full disk waits for room");

    *setup.exporter.full_from_part.lock().unwrap() = None;
    setup.exporter.clear();
    let resumed = ResumeAudioExport::new(setup.jobs.clone()).run(&ctx(), export.id.clone()).unwrap();
    assert_eq!((resumed.status, resumed.paused_by, resumed.error), (ExportStatus::Queued, None, None));
    wait_for_status(&setup, &export.id, ExportStatus::Done);
    assert_eq!(setup.exporter.events()[0], Event::BeginPart { index: 2, sample_rate: SAMPLE_RATE });
}

/// C24: out of space while joining the parts pauses at the assemble stage; resuming only joins them.
#[test]
fn out_of_space_while_assembling_pauses_and_resuming_only_assembles() {
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), ONE_SENTENCE_PARTS);
    setup.exporter.full_when_assembling.store(true, Ordering::SeqCst);
    let id = setup.library.add_book();

    let export = start(&setup, &id, ExportFormat::M4b).unwrap();

    let paused = wait_for_status(&setup, &export.id, ExportStatus::Paused);
    assert_eq!((paused.paused_by, paused.stage, paused.parts_done), (Some(PauseReason::StorageFull), ExportStage::Assemble, 4));
    assert!(!setup.exporter.events().contains(&Event::Discard));

    setup.exporter.full_when_assembling.store(false, Ordering::SeqCst);
    setup.exporter.clear();
    setup.jobs.resume(&export.id).unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);
    let events = setup.exporter.events();
    assert_eq!(setup.exporter.begins(), 0, "{events:?}");
    assert!(matches!(&events[0], Event::Assemble { parts, .. } if parts == &[0, 1, 2, 3]));
}

/// C24: user pauses wait for the user; system pauses continue with "resume what you can".
#[test]
fn only_system_and_engine_switch_pauses_resume_by_themselves() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();
    let by_user = start(&setup, &id, ExportFormat::Mp3).unwrap();
    let by_system = start(&setup, &id, ExportFormat::M4b).unwrap();
    setup.engine.wait_until_blocked();

    setup.jobs.pause(&by_user.id, PauseReason::User).unwrap();
    let paused = PauseAudioExports::new(setup.jobs.clone()).run(&ctx(), PauseReason::System).unwrap();
    assert_eq!(paused.iter().map(|export| export.id.as_str()).collect::<Vec<_>>(), [by_system.id.as_str()]);

    let active = ResumeAudioExports::new(setup.jobs.clone()).run(&ctx(), ()).unwrap();

    assert_eq!(active.iter().map(|export| export.id.as_str()).collect::<Vec<_>>(), [by_system.id.as_str()]);
    assert_eq!(setup.jobs.get(&by_user.id).unwrap().paused_by, Some(PauseReason::User));
    setup.engine.release();
}

/// C30: a range export reads only its sentences, opens with its chapter and names the file and title after it.
#[test]
fn a_range_export_covers_only_its_sentences() {
    let setup = setup();
    let id = chaptered_book(&setup.library);
    let range = ExportRange { from_sentence: SentenceId(3), to_sentence: SentenceId(4) };

    let export = start_with(&setup, StartAudioExportArgs { range: Some(range), range_label: Some("Ch. 2–3".into()), ..args(&id, ExportFormat::M4b) });
    assert_eq!((export.sentences_total, export.range), (2, Some(range)));
    wait_for_status(&setup, &export.id, ExportStatus::Done);

    assert_eq!(setup.engine.spoken(), ["Bbbbbb.", "Gamma"]);
    let target = setup.exporter.assembled().pop().unwrap();
    assert_eq!((target.title.as_str(), target.file_stem.as_str()), ("Chapter Book (Ch. 2–3)", "af_heart.fake-engine.100.s3-4"));
    let gamma_ms = ("Bbbbbb.".len() * SAMPLES_PER_CHAR) as u64 * 1000 / u64::from(SAMPLE_RATE);
    assert_eq!(setup.exporter.chapters()[0], [("Beta".to_string(), 0), ("Gamma".to_string(), gamma_ms)]);

    let whole = start(&setup, &id, ExportFormat::M4b).unwrap();
    assert_ne!(whole.id, export.id, "the whole book is another file");
    for bad in
        [ExportRange { from_sentence: SentenceId(4), to_sentence: SentenceId(3) }, ExportRange { from_sentence: SentenceId(0), to_sentence: SentenceId(6) }]
    {
        let refused = setup.start.run(&ctx(), StartAudioExportArgs { range: Some(bad), ..args(&id, ExportFormat::M4b) });
        assert_eq!(refused.unwrap_err().primary(), ErrorName::RequestInvalid);
    }
}

/// C18 + C30: sizes from the text, the cached part, and cumulative offsets per TOC entry and per part.
#[test]
fn an_estimate_sizes_the_export_without_making_audio() {
    let setup = setup_with_settings(GatedEngine::open(), Arc::default(), ONE_SENTENCE_PARTS);
    let id = chaptered_book(&setup.library);
    let clip = CountingEncoder.encode(&lectrice_domain::Audio { samples: vec![0.0; 10], sample_rate: SAMPLE_RATE }).unwrap();
    setup.cache.put(&AudioKey { document_id: id.clone(), ..key_of(&id, 1) }, &clip).unwrap();
    let ms = |texts: &[&str]| (texts.concat().len() as f64 / 18.0 * 1000.0).round() as u64;
    let estimate_args = EstimateAudioExportArgs {
        document_id: id.clone(),
        voice_id: VoiceId::new("af_heart"),
        engine_id: None,
        read_skipped: false,
        speed: speed(200),
        range: Some(ExportRange { from_sentence: SentenceId(0), to_sentence: SentenceId(3) }),
    };

    let estimate = setup.estimate.run(&ctx(), estimate_args).unwrap();

    assert_eq!((estimate.sentences, estimate.parts), (4, 4));
    assert_eq!(estimate.audio_ms_1x, ms(&CHAPTERED_TEXTS[..4]));
    assert_eq!(estimate.output_ms, (estimate.audio_ms_1x as f64 / 2.0).round() as u64);
    assert_eq!(estimate.cached_ms_1x, ms(&["Aaaa."]));
    assert_eq!(estimate.total_audio_ms_1x, ms(&CHAPTERED_TEXTS));
    let toc: Vec<(u32, u32, u64)> = estimate.toc_offsets.iter().map(|offset| (offset.toc_index, offset.first_sentence.0, offset.audio_ms_1x_before)).collect();
    assert_eq!(toc, [(0, 0, 0), (1, 2, ms(&CHAPTERED_TEXTS[..2])), (2, 4, ms(&CHAPTERED_TEXTS[..4]))]);
    assert_eq!(estimate.part_offsets.len(), 6);
    assert_eq!((estimate.part_offsets[3].first_sentence, estimate.part_offsets[3].page_label.as_str()), (SentenceId(3), "ix"));
    assert_eq!(estimate.part_offsets[3].audio_ms_1x_before, ms(&CHAPTERED_TEXTS[..3]));
    assert_eq!(setup.cache.refreshes(), 1, "the cache index is re-read first");
    assert!(setup.engine.spoken().is_empty());
}

/// C19: finished exports can be deleted (file and record); unfinished ones must be cancelled first.
#[test]
fn a_finished_export_is_deleted_with_its_file_and_an_unfinished_one_is_refused() {
    let setup = setup_with(GatedEngine::new("Intro"), Arc::default());
    let id = setup.library.add_book();
    let running = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();

    assert_eq!(DeleteAudioExport::new(setup.jobs.clone()).run(&ctx(), running.id.clone()).unwrap_err().primary(), ErrorName::RequestInvalid);
    setup.engine.release();
    wait_for_status(&setup, &running.id, ExportStatus::Done);

    DeleteAudioExport::new(setup.jobs.clone()).run(&ctx(), running.id.clone()).unwrap();

    assert_eq!(setup.exporter.deleted().len(), 1);
    assert!(setup.jobs.get(&running.id).is_none() && setup.store.get(&running.id).is_none());
}

/// C19: a file moved to shared storage is only a handle afterwards; deleting the record leaves it to the shell.
#[test]
fn a_saved_export_keeps_only_its_shared_handle() {
    let setup = setup();
    let id = setup.library.add_book();
    let export = start(&setup, &id, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &export.id, ExportStatus::Done);
    let mark = MarkExportSaved::new(setup.jobs.clone());

    let saved = mark.run(&ctx(), MarkExportSavedArgs { id: export.id.clone(), uri: "content://media/7".into() }).unwrap();

    assert_eq!(saved.location, Some(ExportLocation::Shared { uri: "content://media/7".into() }));
    assert!(setup.store.get(&export.id).unwrap().file.is_none());
    let file = GetAudioExportFile::new(setup.jobs.clone(), setup.library.reader(), setup.engine.clone()).run(&ctx(), export.id.clone());
    assert_eq!(file.unwrap_err().primary(), ErrorName::ExportNotFound);
    DeleteAudioExport::new(setup.jobs.clone()).run(&ctx(), export.id).unwrap();
    assert!(setup.exporter.deleted().is_empty());
    let blank = mark.run(&ctx(), MarkExportSavedArgs { id: ExportId::new("x"), uri: " ".into() });
    assert_eq!(blank.unwrap_err().primary(), ErrorName::RequestInvalid);
}

#[test]
fn every_export_is_listed_across_documents() {
    let setup = setup();
    let first = setup.library.add_book();
    let second = chaptered_book(&setup.library);
    let a = start(&setup, &first, ExportFormat::Mp3).unwrap();
    wait_for_status(&setup, &a.id, ExportStatus::Done);
    let b = start(&setup, &second, ExportFormat::M4b).unwrap();
    wait_for_status(&setup, &b.id, ExportStatus::Done);

    let all = ListAllAudioExports::new(setup.jobs.clone()).run(&ctx(), ()).unwrap();

    assert_eq!(all.iter().map(|export| export.id.clone()).collect::<Vec<_>>(), [b.id, a.id]);
}

/// C28: purging a document cancels its unfinished exports and keeps the finished ones unless asked.
#[test]
fn purging_a_document_cancels_its_unfinished_exports_and_keeps_finished_ones() {
    let store = Arc::new(MemoryRecordStore::default());
    store.save(&saved("old-done", ExportStatus::Done)).unwrap();
    store.save(&saved("old-failed", ExportStatus::Failed)).unwrap();
    let setup = setup_with(GatedEngine::new("Intro"), store);
    let id = setup.library.add_book();
    let running = start(&setup, &id, ExportFormat::Mp3).unwrap();
    setup.engine.wait_until_blocked();
    let queued = start(&setup, &id, ExportFormat::M4b).unwrap();

    let purged = setup.jobs.purge_document(&id, false).unwrap();

    assert_eq!((purged.cancelled, purged.deleted, purged.kept), (2, 0, 2));
    assert!(setup.jobs.get(&queued.id).is_none() && setup.store.get(&queued.id).is_none());
    assert_eq!(setup.store.get(&running.id).unwrap().export.status, ExportStatus::Cancelled, "a crash now can't resume it");
    setup.engine.release();
    wait_for("the running export to be dropped", || setup.jobs.get(&running.id).is_none());
    assert!(setup.store.get(&running.id).is_none());
    assert!(setup.exporter.events().contains(&Event::Discard), "its parts are deleted");
    assert_eq!(setup.jobs.purge_document(&id, true).unwrap().deleted, 2);
    assert_eq!(setup.exporter.deleted(), [std::path::PathBuf::from("/exports/old/af_heart.mp3")]);
    assert!(setup.jobs.list_all().is_empty());
}
