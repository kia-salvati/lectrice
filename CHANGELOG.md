# Changelog

> Versions were renumbered to 0.x while Lectrice is in beta (2026-10-24): `1.0.0` is now `0.1.0` and `1.1.0` is
> now `0.2.0` (tags `v0.1.0`, `v0.2.0`).

All notable changes to Lectrice are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The version in `backend/Cargo.toml`
(`[workspace.package]`) equals the release tag.

"Core" is the shared Rust library (`lectrice-domain`, `lectrice-app`, `lectrice-infra`) that the Android app
pulls by tag. While in beta (0.x), a change to a core trait, public type or saved format raises the minor version.

## [Unreleased]

## [0.3.0] - 2026-10-25

Prepares the core and the web app for the desktop app (`lectrice-desktop`). No trait or saved format changes.

### Core

#### Changed
- `assemble()` no longer purges removed books. A shell assembles again on every engine reload, so a book removed just
  before a model switch lost its undo window. Shells call the new `lectrice_app::services::purge_removed_at_start`
  once, after the first `assemble` (the web server, the desktop app and the Android FFI do).

#### Fixed
- `FfmpegExporter` starts ffmpeg with `CREATE_NO_WINDOW` on Windows, so an export from a GUI app no longer flashes a
  console window.
- `lectrice-api` compiles for Windows: SIGTERM is only listened for on Unix; Windows stops on Ctrl+C. The core
  crates and `lectrice-api` pass `cargo xwin clippy --target x86_64-pc-windows-msvc -- -D warnings`.

### Web app
- A `platform` module (`src/platform/`): `web` (the default) or `desktop`, picked by `VITE_PLATFORM` at build time.
  It gives the API base URL and the export formats. `npm run build -- --mode desktop` (`.env.desktop`) builds for
  the desktop app: the API on the `lectrice` scheme through `@tauri-apps/api` (2.12.4), M4B only. The desktop code
  is loaded with a dynamic `import()`, so the web build contains none of it.
- Media keys and the browser's media controls (`navigator.mediaSession`): play, pause, next and previous sentence,
  with the book title and the playing state.
- `importPdf` sends the multipart body as bytes (`ArrayBuffer` + its `Content-Type` with the boundary) instead of
  a `FormData` holding the `File`. WebKitGTK crashed when a custom-scheme request body held a File or Blob; the web
  app sends the same bytes as before.

## [0.2.0] - 2026-10-24

### Core

#### Added
- A table of contents per document (`toc`: title, level 1–3, first sentence, page), from the PDF outline when it
  matches the headings, otherwise from the detected headings. Cleaner version 3, so saved books are cleaned again.
- `lectrice_app::services`: `Adapters`, `Settings`, `Services`, `assemble()` and the `DocumentServices`,
  `SpeechServices` and `ExportServices` structs, moved from `lectrice-api` so every shell wires the core the same way.
- Resumable exports. An export is written as numbered parts of about 10 listening-minutes and the finished parts are
  recorded in its export record. New `paused` state; `PauseAudioExport` keeps the finished parts,
  `ResumeAudioExports` queues paused exports again, and starting the same export again continues it.
  `CancelAudioExport` deletes the parts.
- `AudioExport` gains `partsDone` and `partsTotal`; `ExportRecord` gains `parts` and `chapters`.
- `lectrice_app::speech::playback_order(document, read_skipped) -> Vec<SentenceId>`: the one playback order for
  the player, the read-ahead and exports.
- `lectrice-infra` features: `pdfium`, `kokoro`, `sherpa-static` and `ffmpeg-export` (all on by default).
- `KokoroSettings::provider` (the onnxruntime execution provider: `cpu`, or `xnnpack` / `nnapi` on Android).
- Error names for both apps: `ModelMissing` (4001), `ModelDownloadFailed` (4002), `ModelChecksumMismatch` (4003),
  `StorageFull` (4004), `ExportPaused` (4005) and `FileUnreadable` (4006).
- Exports with a baked-in speed (75–200 %, `ExportSpeed`): a new `TimeStretch` / `TimeStretcher` port and its adapter
  `SonicTimeStretch` (a Rust port of Sonic's speed path, Apache-2.0; feature `time-stretch`, on by default). Chapter
  marks are timed from the 1x audio and divided by the speed.
- `KokoroSettings::variant` (`Fp32` keeps the id `kokoro-v1.0`, `Int8` is `kokoro-v1.0-int8`, so their clips never mix).
- Exports tied to an engine (`AudioExport.engineId`, `StartAudioExportArgs.engine_id`); a job for another engine waits
  in the queue (`waitingForEngine`).
- Partial exports (`ExportRange`, `range_label`) and `EstimateAudioExport` (length, cached part, cumulative offsets per
  TOC entry and per part).
- Pause reasons (`PauseReason`: user, system, storage full, engine switch), `stage` (parts / assemble) and `error` on an
  export; `PauseAudioExports` (all), `ResumeAudioExport` (one, any reason).
- `ListAllAudioExports`, `DeleteAudioExport`, `MarkExportSaved`; `AudioExport` gains `title`, `fileBytes`, `location`.
- Read-ahead control: `StopReadAhead`, `StopAllReadAhead`, `SetReadAheadMinutes`, `SpeakSentenceArgs::plan_ahead`, and
  `SpokenSentence::ready_ahead_ms`.
- `SpeakVoiceSample` (one fixed sentence, `VOICE_SAMPLE_TEXT`, kept on disk through a new `VoiceSampleCache` port;
  adapter `FsVoiceSampleCache`) and `GetSpeechStats` (median real-time factor of the last 50 sentences).
- Removing books: `RemoveDocument` (hidden at once), `RestoreDocument`, `PurgeRemovedDocuments` (also run by
  `assemble` at start), `ClearDocumentAudio`; importing a removed, not yet purged PDF restores it.
- `GetStorageUsage` (library, cache and exports, per book) and `ClearAudioCache`; `lectrice_app::services::LibraryServices`
  (`Services.library`) holds these and works without a speech engine.
- `DocumentSummary.cleaner_version` and `DocumentServices::current_cleaner_version()`.
- `FsAudioCache` can live in its own folder (`cache_dir`); a missing, empty or unreadable clip is a miss, and `refresh`
  re-reads the folder.
- A full disk is raised as `StorageFull` (with `StorageFailed` behind it) by every file adapter and by the ffmpeg
  exporter.
- `lectrice_app::CORE_VERSION`: the core's version (the workspace version), for a shell to report (the Android FFI's
  `core_version()`).

#### Changed
- **Breaking:** `AudioExporter` now has `begin_part`, `assemble` and `discard_parts` instead of `begin`, and
  `ExportWriter::finish` returns an `ExportPart`; chapter marks go to `assemble` instead of `start_chapter`.
  `ExportTarget` gains `export_id`.
- **Breaking:** `export_plan` takes `part_minutes`; `ExportSettings` gains `part_minutes` (default 10).
- **Breaking:** the old `playback_order` (with block indexes) is now crate-private as `playable_in_order`.
- `FfmpegExporter` writes 16-bit mono WAV parts and joins them with one ffmpeg concat (MP3, or M4B with chapters).
- An export a stop interrupted is now `paused` (it was `failed`); the desktop server resumes it at start.
- `ExporterUnavailable` now reads "Audio export isn't available."; the ffmpeg detail stays in the log.
- The workspace `sherpa-onnx` dependency no longer turns on `static`; `lectrice-infra`'s `sherpa-static` does.
- `PdfiumTextSource::new` documents that a bare library name lets the system linker find pdfium.
- **Breaking:** `AudioExporter` also has `parts_bytes` and `delete_file`; `ExportTarget` gains `engine_id`, `speed`,
  `range` and `file_stem` (the exporter names the file `<file_stem>.<ext>`).
- **Breaking:** `AudioRetention` also has `usage`, `clear`, `remove_document` and `refresh`; `DocumentReader` has
  `list_removed` and `library_usage`; `DocumentWriter` has `set_removed` and `delete`. A removed document is invisible
  to `find`, `find_summary` and `list_recent`.
- **Breaking:** `StartAudioExportArgs` gains `speed`, `engine_id`, `range`, `range_label`; `PauseAudioExport` takes
  `PauseAudioExportArgs { id, reason }`; `ResumeAudioExports` only resumes system and engine-switch pauses.
- **Breaking:** `ExportDeps` takes `speech: Option<ExportSpeech>` and `time_stretch`; `ExportSettings` gains
  `short_file_names`; `StartAudioExport::new` takes `StartAudioExportDeps`; `ReadAhead::new` takes `ReadAheadDeps`;
  `SpeakSentenceDeps.read_ahead` is an `Arc`; `export_plan` takes `PlanOptions`; `Adapters` gains
  `speech_engine_load_ms`, `voice_samples` and `time_stretch`; `Services` gains `library`.
- A `StorageFull` at any export step (writing a part, saving the record, assembling) pauses the export and keeps its
  parts; other errors still fail it.
- Export chapter marks come from the TOC entries of level 1–2 (titles cut at 120 characters), or from the headings when
  a book has no TOC (C23). When a part and its first chapter start at the same sentence, the chapter's title is kept.
- Same-file rule for exports: document, voice, format, engine, speed and range.
- `FsAudioCache::open` takes the cache folder (the desktop passes its data folder, so nothing moves).
- `ImportPdf` no longer logs the file name (it can be a book title); the import log has the request id and the size.

#### Migration
- Export records saved by 0.1 or an earlier 0.2 build load as they are: no engine id means the loaded engine, a paused
  one without a reason is a system pause, a done one gets its file as `location`, the speed is 100.
- `meta.json` files without `cleanerVersion` read it from `document.json`; without `removedAt` the book isn't removed.
- An `AudioExporter` adapter implements `begin_part`, `assemble`, `discard_parts`, `parts_bytes` and `delete_file` and
  names the file from `ExportTarget::file_stem`; see `FfmpegExporter`.
- An `AudioRetention` adapter implements `usage`, `clear`, `remove_document` and `refresh`; see `FsAudioCache`.
- A shell builds `lectrice_app::services::Adapters` and `Settings` and calls `assemble`, then calls
  `exports.resume` when it is ready for paused exports to continue. A shell with one engine sets
  `ExportSettings::short_file_names` to keep 0.1's export file names.
- `Settings::upload_max_bytes` is now `import_max_bytes`.

### Desktop server
- New `[speech] provider` config key (default `"cpu"`).
- Optional `[storage] cache_dir` (default: `data_dir`) for the sentence audio and voice samples.
- The startup log (`Config loaded`) includes `core_version`.
- `backend/build-release.sh`: a release build with `--remap-path-prefix`, so the binary has no `/home/<user>/...`
  paths (Cargo's `trim-paths` isn't stable yet).
- The `clean` dev command logs the document id and the output folder instead of the PDF's file name and output paths.
- `DELETE /api/documents/{id}` removes a book with its position and audio (export files stay), and
  `GET /api/voices/{voiceId}/sample` plays a voice's sample.
- `AudioExport` JSON gains `speedPercent`, `engineId`, `range`, `rangeLabel`, `title`, `pausedBy`, `stage`, `error`,
  `fileBytes`, `location` and `waitingForEngine` (and `status` may be `paused`); `DocumentSummary` gains `cleanerVersion`.

### Web app
- Chapters: a closable whole-book outline rail on wide screens, a bottom sheet elsewhere, and
  "Ch. 7 · Title · p. 213" in the player pill.
- The API types follow the new export and summary fields; a paused export shows as "stopped", and "Try again"
  continues it from its finished parts.

## [0.1.0] - 2026-10-23

The first release: PDF → cleaned reading view → Kokoro `af_heart` reading sentence by sentence, with read-ahead,
an audio cache, reading positions, recent documents and MP3/M4B export.

[Unreleased]: https://github.com/kia-salvati/lectrice/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/kia-salvati/lectrice/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kia-salvati/lectrice/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/kia-salvati/lectrice/releases/tag/v0.1.0
