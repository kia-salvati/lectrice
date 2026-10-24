# Core v1.1.0, part 2: the Android core changes (C10–C30)

Branch `feature/core-v1.1` (local). Nothing committed, pushed, stashed or tagged. Spec: `docs/android-logic/19-core-and-ffi-changes.md`
(plus files 08, 10, 16, 20, 21). Part 1 (C1–C9) is in `docs/progress/core-v1.1.md`.

**No new crate dependencies** (`Cargo.lock` unchanged). The Sonic time-stretch is our own Rust port.

Not mine and left alone: `design/2026-10-23-android-quiet-v2.html` and `docs/android-logic/10-export.md` show as modified
in `git status`; another agent changed them. I only touched `docs/2026-10-21-design-spec.md`, `CHANGELOG.md`, this note,
plus a new root `NOTICE` (for the Sonic port) and `backend/README.md` (feature table).

## Per item

| # | State | What changed | Tests |
|---|---|---|---|
| **C10** speed (Sonic) | done | Domain: `ExportSpeed(u16)` (75/100/125/150/175/200 only, else `RequestInvalid`; JSON `speedPercent`, default 100), ports `TimeStretch` / `TimeStretcher`. Infra: `audio/sonic.rs` `SonicTimeStretch` behind new feature `time-stretch` (in `default`). App: the runner pushes each clip through one stretcher per part (flushed at the part's end); 1x builds no stretcher; chapter marks = 1x samples ÷ speed; `ExportPart.samples` counts output; `ExportRecord.source_samples` keeps the 1x count so a resume times later marks right; resume takes the speed from the record. File name `export_file_stem`: `<voice>.<ext>` for whole-book 1x when `ExportSettings::short_file_names` (desktop), else `<voice>.<engine>.<speed>[.s<from>-<to>]`. A speed without a time-stretch adapter is refused (`RequestInvalid`). | sonic: 1.5x of a 220 Hz sine = 0.667 s ±1 %, 220 Hz ±2 % (zero crossings); 75/125/175/200 lengths ±1 % and pitch; 100 passes through; 1 vs 50 pushes within one period; a stream shorter than a window. App: 1x builds no stretcher; marks at 2x are the 1x marks halved (±10 ms) with shorter appends; resume after pause at 150 keeps 150 (part 2 made again, 0–1 not); 125 while 150 paused = new job; speed without adapter refused. Domain: 0/76/250 refused. Desktop: ffmpeg test, 3 s at 1.5x → ffprobe 2.0 s ±2 %, file `af_heart.engine-a.150.m4b`. |
| C11 engine id per bundle | done | `KokoroSettings.variant: KokoroVariant { Fp32, Int8 }`; ids `kokoro-v1.0` / `kokoro-v1.0-int8`. Int8 loads `model.int8.onnx` (**UNVERIFIED** name, from sherpa's int8 bundle; the spike must confirm it). The desktop uses `Fp32`. | ids differ; int8 against the fp32 bundle is `SpeechEngineUnavailable` before sherpa-onnx runs. |
| C12 exports tied to a bundle | done | `AudioExport.engine_id` (records without one get the loaded engine's id at load: only the desktop has such records, made by its one engine; this keeps "Kokoro" out of the domain), `StartAudioExportArgs.engine_id` (`None` = loaded), runner takes only jobs of the loaded engine, `waiting_for_engine` computed on read and never stored. `ExportJobs` now also works **without** an engine (`ExportDeps.speech: Option<ExportSpeech>`): jobs load, list, delete; queued ones wait. | other-engine job waits while the loaded one runs (and isn't stored as waiting); old records get the engine id; jobs without engine resume to queued + waiting. |
| C13 read-ahead control | done | `ReadAhead::stop(doc)` / `stop_all()` cancel only **queued** jobs (new `CancelHandle::cancel_if_queued`), so the clip being made is still cached, and forget the plans; `SpeakSentenceArgs.plan_ahead` (`false` = no plan, no trim). Use cases `StopReadAhead`, `StopAllReadAhead`. HTTP route uses `plan_ahead: true`. | stop withdraws 59 of 60 and the running one is cached, next request plans again; stop-all over two voices; `plan_ahead = false` plans and trims nothing, a later `true` does; worker `cancel_if_queued` leaves a running job alone. |
| C14 minutes at runtime | done | `ReadAhead::set_read_ahead_minutes` (1..=60 and keep ≥ 1.5 × minutes), use case `SetReadAheadMinutes`; the minutes are part of the plan's anchor, so the next request re-plans and a shorter horizon cancels the queued jobs past it. | 0 / 41 / 61 refused (keep 60); 1→2 plans 120 keeping 60; 2→1 cancels the 60 extra. |
| C15 storage usage + clear | done | `AudioRetention`: `usage()` (`CacheUsage { bytes, clips, cap_bytes, documents }`), `clear()`. `DocumentReader::library_usage()` (the three files). `AudioExporter::parts_bytes()`. Use cases `GetStorageUsage` (refreshes first; per book, largest first; removed books left out; exports = done + in app storage) and `ClearAudioCache`. | infra: usage per document and clear; library bytes = the three files; parts bytes. App: usage numbers, shared-storage export not counted, removed book left out, refresh called; clear. |
| C16 voice sample | done | `SpeakVoiceSample` (VoiceId → `SpokenSentence`), `VOICE_SAMPLE_TEXT` / `VOICE_SAMPLE_VERSION = 1`, new port `VoiceSampleCache` + `FsVoiceSampleCache` at `<cache_dir>/samples/<engine>/<voice>.v1.wav` (empty file = miss). Made at priority `Now` through the worker (with a fixed all-zero document id in its key, which no PDF hashes to); a failed save is logged and the sample still plays. HTTP: `GET /api/voices/{voiceId}/sample`. | made once then read back; unsavable still plays; unknown voice refused; infra round trip per engine/voice/version and empty file; route: second call doesn't synthesize, file on disk. |
| C17 speech stats | done | `SpeechWorker::timings()` over the last 50 synthesized jobs (median synth ÷ audio, `None` below 10); `GetSpeechStats` → `SpeechStats { engine_id, rtf_median, samples, engine_load_ms }` (load time from the new `Adapters::speech_engine_load_ms`; the desktop measures it). | 9 jobs → none, 10 → a median; stats name engine and load time. |
| C18 estimate | done | `EstimateAudioExport` (args: document, voice, engine?, read_skipped, speed, range) → `ExportEstimate { sentences, parts, audio_ms_1x, output_ms, cached_ms_1x, total_audio_ms_1x, toc_offsets, part_offsets }`; refreshes the cache index first; no synthesis. Not timed on an 8 000-sentence book (it is two plans plus one index lookup per sentence). | sizes, output at 2x, cached part, TOC and part offsets (with page labels), refresh called, nothing spoken. |
| C19 delete / list all / size / location | done | `AudioExport` gains `file_bytes`, `location: ExportLocation { AppStorage { path }, Shared { uri } }` (old done records get `AppStorage` from `file`), `title` (stored at start). `ListAllAudioExports`, `DeleteAudioExport` (done/failed/cancelled; deletes an app-storage file through the new `AudioExporter::delete_file`, which only deletes inside the exports folder), `MarkExportSaved` (done only, else `ExportNotReady`; clears `file`). The download falls back to the stored title and answers `ExportNotFound` for a shared file. | delete refused while unfinished, then file + record go; saved export keeps only its handle and its record delete leaves the file; list all across documents; ffmpeg `delete_file` refuses paths outside exports. |
| C20 import progress | **skipped** | Conditional on the spike (M-h > 15 s on the S10+); no spike result yet. | – |
| C21 prepare jobs | **skipped** | Conditional on the spike (RTF amber/red); no spike result yet. | – |
| C22 `StorageFull` from the disk | done | `storage_failed` raises `StorageFull` then `StorageFailed` when the cause (or `hound::Error::IoError`) is `ErrorKind::StorageFull` or OS error 28; `FfmpegExporter` maps ffmpeg's "No space left on device". | kind, raw code and through hound; a fake ffmpeg script that prints ENOSPC → `[StorageFull, StorageFailed]`. |
| C23 TOC chapter marks | done (was "proposed"; the brief said all of C10–C30) | Marks from TOC entries of level ≤ 2 (titles trimmed, cut at 120 chars); headings only when the TOC is empty; when several start at one sentence, the last (deepest) one is kept. | TOC marks with the cut and level 3 ignored; heading fallback unchanged. |
| C24 pause reasons / out of space | done | `paused_by: PauseReason { User, System, StorageFull, EngineSwitch }` (old paused records load as `System`), `stage: ExportStage { Parts, Assemble }`, `error: Option<ErrorName>` (stored by name; unknown names read as `Internal`). `StorageFull` at any step pauses and keeps the parts: a part write (open part dropped), a record save (stops the run), assemble (stage `Assemble`; resume only assembles). `PauseAudioExport { id, reason }`; a user pause also overrides a system pause. `PauseAudioExports(reason)` for all; `ResumeAudioExports` resumes only System/EngineSwitch; `ResumeAudioExport(id)` resumes any. Shutdown and interruption are `System`. | full disk on part 3 → paused, parts 0–1 kept, no discard, not auto-resumed, resume starts at part 2; full during assemble → paused at Assemble with all parts, resume assembles only; user vs system pauses; old records load as System. |
| C25 ready ahead | done | `SpokenSentence.ready_ahead_ms`: the contiguous cached run after the sentence in playback order (index only); sentences with nothing to say don't break it. | planner (run stops at the first gap), ReadAhead (cache index), SpeakSentence answer. |
| C26 `FileUnreadable` | done | Registry 4006 / 422 / operational / "This file couldn't be opened." Count 26. | registered; names stored by name. |
| C27 cleaner version on summary | done | `DocumentSummary.cleaner_version`; `meta.json` stores `cleanerVersion` (older files read it from `document.json`, parsing only that field); `DocumentServices::current_cleaner_version()`. JSON `cleanerVersion`. | import summary; infra with and without the field in meta (find and list); route JSON. |
| **C28** remove a document | done | Ports: `DocumentWriter::set_removed` / `delete` (meta first), `DocumentReader::list_removed`; removed books are hidden from `find`, `find_summary`, `list_recent`; `save` keeps a book removed; `CachedLibrary` forgets on both. `AudioRetention::remove_document` (all cleaner versions/engines/voices; clips in use are deleted when released; the folder goes when nothing is in use). A read-ahead clip made for a removed book is not stored (the read-ahead checks `find_summary` before `put`). Use cases `RemoveDocument` (+ stop read-ahead), `RestoreDocument`, `PurgeRemovedDocuments { delete_exports_for }` (unfinished exports cancelled and dropped, a running one when it stops; finished ones kept unless listed; then audio, then folder; a failing book is counted and retried next time), `ClearDocumentAudio`. `assemble` purges at start (exports kept). `ImportPdf` restores a removed, unpurged book without cleaning. HTTP: `DELETE /api/documents/{id}` = remove + purge, export files kept. | hidden then restored with its position; purge deletes book and its clips only, keeps finished exports with title, drops unfinished; purge with `delete_exports_for` deletes the file; import restores; startup purge through `assemble`; infra repository hide/restore/delete, cache remove, cached library; read-ahead drops the clip; route deletes folder and audio. |
| **C29** cache root | done | `FsAudioCache::open(cache_dir, …)`; `get` treats an empty or unreadable clip as a miss and deletes it (unless a `put` replaced it meanwhile); `put` re-creates folders once more if they vanish; `refresh()` rescans. Config: optional `[storage] cache_dir` (default `data_dir`; see "Notes" for the env override). | deleted behind its back → miss, refresh drops usage, put works again; 0-byte clip; own folder; config resolves `~`. |
| **C30** partial export | done | `ExportRange { from, to }` (inclusive; `from > to` or past the end → `RequestInvalid`; an empty range plan is refused), `range_label` (only kept with a range; title tag `"{title} ({label})"`), on the record and target; the plan keeps only the range, and the first sentence opens with the mark of the chapter it is in; file name `.s<from>-<to>`; part of the same-file rule. | range plan + mark at 0 mid-chapter; range export (spoken sentences, title, file stem, marks from 0); whole book is another job; bad ranges refused. |
| C6b espeak-less model dir | **skipped** | Conditional on sherpa-onnx 2.x (spike M-k, M-l); 2.x isn't downloaded (metered). Unchanged from part 1, decision 9. | – |

## Commands and results (2026-10-24, in `backend/` unless noted)

| Command | Result |
|---|---|
| `cargo build --release` | ok |
| `cargo test` (workspace) | ok: **425 passed**, 0 failed, 12 ignored, 28 test binaries (part 1: 351) |
| `cargo clippy --all-targets -- -D warnings` | ok (4 findings on the way, fixed: two `cloned_ref_to_slice_refs`, `redundant_closure`, `manual_is_multiple_of`) |
| `cargo fmt --check` | ok |
| `cargo test -p lectrice-infra --test export_ffmpeg -- --include-ignored` (real ffmpeg + ffprobe) | ok: 9 passed (incl. the new 1.5x duration test) |
| `cargo clippy -p lectrice-infra --no-default-features --features "<f>" --all-targets -- -D warnings` for `""`, `pdfium,kokoro,time-stretch`, `pdfium,kokoro`, `time-stretch`, `ffmpeg-export` | ok for all five |
| `cargo test -p lectrice-infra --no-default-features` | ok |
| `cargo test -p lectrice-infra --no-default-features --features pdfium,kokoro,time-stretch` | ok |
| `cargo test -p lectrice-app -p lectrice-api --lib`, 6 times in a row | ok every time (threaded export/read-ahead tests are stable) |
| `grep` for Kokoro/sherpa names in domain, app and infra outside `src/speech/` | only the old feature doc lines in `lectrice-infra/src/lib.rs` and a test id |
| `frontend/`: `npx tsc -b`, `npx oxlint`, `npm test` | ok, ok, 21 passed |

Ignored tests not run: the book tests (`~/Desktop` PDFs) and the three real-Kokoro tests.

**Smoke test** (port 7878 was the user's running server, left alone): the new release binary on **7879** with a temporary
config (data and a separate `cache_dir` in my scratchpad):
- `/api/health` ready; uploaded the smallest `~/Desktop` book (431 pages, 8031 sentences, `cleanerVersion` 3).
- Sentence audio 200 `audio/wav`; clips appeared under `cache_dir/audio`, none under `data_dir`.
- `GET /api/voices/bf_emma/sample`: 200 in 2.1 s, then 1 ms from `cache_dir/samples/kokoro-v1.0/bf_emma.v1.wav`.
- M4B export (`max_sentences = 30`): new JSON fields present; `done` 30/30, 1 part, speed 100, engine `kokoro-v1.0`,
  location `appStorage`, file still `af_heart.m4b` (desktop names kept); download 200 `audio/mp4`, ffprobe aac, 168 s.
- `DELETE /api/documents/{id}` 204 → library empty, `GET` 404 `DocumentNotFound`, library and audio folders gone, the
  export file kept.
- Logs: 0 errors besides that expected 404; field names checked: ids, counts, durations only (no text fields).
- Server stopped (SIGTERM), temp data deleted. No book text went into any file.

## Notes and decisions

1. **No "Kokoro" in the core:** old export records get the loaded engine id at load instead of a `"kokoro-v1.0"` serde
   default, and the 1.0 file name comes from `ExportSettings::short_file_names` (the desktop sets it; Android leaves it
   off, so its files always name engine and speed and fp32/int8 files never collide).
2. **`ExportJobs` without an engine** (needed so purge, storage numbers, list-all and delete work before a model is
   installed). `Services.exports` is still `None` without an engine, so the web API's 503s are unchanged.
3. **`LibraryServices`** (`Services.library`) holds remove/restore/purge/clear-book-audio/storage-usage/clear-cache/
   list-all/delete/mark-saved.
4. **Removed-book clips:** the read-ahead checks `find_summary` (one small read) before storing a clip, rather than the
   cache keeping a removed-documents list (`ClearDocumentAudio` uses the same `remove_document` and must not block later
   clips).
5. **`storage.cache_dir` and env overrides:** overrides need the key in the file, and the shipped file only has it as a
   comment, so `LECTRICE_STORAGE_CACHE_DIR` is refused (unknown override). Set it in the file. Adding it uncommented with
   the same path as `data_dir` would stop following a `LECTRICE_STORAGE_DATA_DIR` override, so I left it commented.
6. **Web:** `paused` maps to the existing "stopped before it finished / Try again" view, and "Try again" starts the same
   export, which continues the paused job. It only happens on the desktop when the disk fills up. No new UI.
7. **C23 tie rule** (several TOC entries at one sentence): the last, deepest one names the mark.
8. `to_millis` now rounds instead of truncating (≤ 1 ms difference at 1x).

## Core API for the `lectrice-ffi` step

Wiring (`lectrice_app::services`): `Adapters { …, speech_engine, speech_engine_load_ms: Option<u32>, voice_samples:
Arc<dyn VoiceSampleCache>, time_stretch: Option<Arc<dyn TimeStretch>>, … }` → `assemble(adapters, Settings { …, export:
ExportSettings { max_sentences, part_minutes, short_file_names: false } })` → `Services { documents, library, speech,
exports }`. Adapters for Android: `FsDocumentRepository` / `CachedLibrary`, `FsAudioCache::open(cacheDir/lectrice, 1024 MB)`,
`FsVoiceSampleCache::new(cacheDir/lectrice)`, `FsExportRecordStore`, `SonicTimeStretch`, `KokoroSettings { variant,
provider, … }` (`KokoroVariant::{Fp32, Int8}`), and the ffi's own `AudioExporter` (must now also implement `parts_bytes`
and `delete_file` and name files from `ExportTarget::file_stem`).

| ffi (§3.22) | core |
|---|---|
| `speak_sentence(…, plan_ahead)` → `SpokenAudio { ready_ahead_ms }` | `speech.speak_sentence` (`SpeakSentenceArgs.plan_ahead`, `SpokenSentence.ready_ahead_ms`) |
| `speak_voice_sample(voice)` | `speech.voice_sample` (VoiceId → `SpokenSentence`) |
| `speech_stats()` | `speech.stats` → `SpeechStats` |
| `stop_read_ahead(doc)` / `stop_all_read_ahead()` | `speech.stop_read_ahead` / `speech.stop_all_read_ahead` |
| `set_read_ahead_minutes(m)` | `speech.set_read_ahead_minutes` |
| `start_export(doc, voice, variant, speed_percent, read_skipped, range?, range_label?)` | `exports.start` (`StartAudioExportArgs { format: M4b, speed: ExportSpeed::from_percent(p)?, engine_id: Some(variant.engine_id()), range, range_label, … }`) |
| `estimate_export(…)` | `exports.estimate` → `ExportEstimate { …, toc_offsets, part_offsets }` |
| `pause_export(id)` / `pause_exports_for_system()` | `exports.pause` (`PauseAudioExportArgs { id, reason: User }`) / `exports.pause_all` (`PauseReason::System`; `EngineSwitch` before `reload_engine`) |
| `resume_exports()` / `resume_export(id)` | `exports.resume` / `exports.resume_one` |
| `cancel_export(id)`, `get` | `exports.cancel`, `exports.get` |
| `list_all_exports()`, `delete_export(id)`, `mark_export_saved(id, uri)` | `library.list_all_exports`, `library.delete_export`, `library.mark_export_saved` (`MarkExportSavedArgs`) |
| `storage_usage()` / `clear_audio_cache()` | `library.storage_usage` → `StorageUsage { …, documents: [DocumentUsage] }` / `library.clear_audio_cache` |
| `remove_document`, `restore_document`, `purge_removed_documents(list)`, `clear_document_audio` | `library.remove`, `library.restore`, `library.purge_removed` (`PurgeRemovedDocumentsArgs`), `library.clear_document_audio` |
| `DocumentSummaryRecord.cleaner_version`, `current_cleaner_version()` | `DocumentSummary.cleaner_version`, `documents.current_cleaner_version()` |
| `AudioExportRecord` | `AudioExport` (all new fields above) |
| `CoreErrorInfo` | `ErrorName::ALL` / `spec()` now has 26 names (`FileUnreadable` 4006) |

Engine swap (`reload_engine`): `exports.pause_all(EngineSwitch)`, `speech.stop_all_read_ahead`, drop the old `Services`,
`assemble` again with the other `KokoroVariant`, then `exports.resume`.

## Left open

- C20, C21, C6b (conditional; need spike results or sherpa-onnx 2.x).
- The int8 model file name (`model.int8.onnx`) and that the int8 `voices.bin` has the same 28 voices: to be checked in the spike.
- No HTTP route for speed, range, estimate, pause/resume or storage (not needed by the web app).
- Tag `v1.1.0` after the user reviews and commits (not created).
