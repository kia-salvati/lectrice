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

### Core

#### Added
- `KokoroSpeechEngine::voice_catalog()`: the voices without loading a model (for a voice list before the engine is ready).
- Prepare jobs (C21): `SpeechServices.prepare` (`PrepareJobs`: `start`, `stop`, `cancel`, `status`) makes a range of a book
  into the sentence cache at export priority, one sentence at a time, skipping cached ones, and pins the range.
- `AudioRetention::pin` / `unpin` (default: no pins): pinned clips are neither counted in nor deleted by
  `retain_around`'s window. `FsAudioCache` implements them (in memory; the shell pins again after a restart).
- Audiobook source (Android spec 23, C35–C39): M4B exports carry a **Lectrice map** (a top-level `uuid` box with JSON:
  document id, cleaner version, engine, voice, speed, and every spoken sentence's id, text hash and start time).
  `lectrice_domain::audiobook` (`LectriceMap`, `text_hash`, `match_audiobook`: sentence follow-along for the same book,
  remapped by hash across cleaner versions, else chapter titles), `lectrice_infra::export::mp4_map` (`append_map`,
  `read_audiobook`: map, length, `chpl` chapters, title). New error names `AudiobookNotLectrice` 3101,
  `AudiobookOtherBook` 3102, `AudiobookUnreadable` 3103.
- New crate **`lectrice-link`** (Android spec 24, C40–C41; desktop helper): wire protocol v1 (`postcard` messages, u16
  framing, fragment joining, limits), SPAKE2 + Noise `XXpsk3` pairing on a 6-digit code (3 wrong codes close the window,
  10 per hour start a 10-minute wait, stalled attempts free their slot), Noise `IK` sessions with pinned keys, FLAC clips
  (`flacenc` / `claxon`, bit-exact with the WAV encoder's PCM16), "LAN only" address rule. Features: `client` (the phone:
  `pair`, `Session`), `server` (the computer: `Server` over shell-bound listeners, `EngineProvider` / `ClipEngine` /
  `PeerStore` ports, `CpuGate` priorities, per-phone item budget), `testing` (`FaultyLink` proxy). Tests: cacophony
  Noise vectors, a recorded SPAKE2 vector, end-to-end client↔server, link-drop tests, fuzz smoke (+ `fuzz/` targets).
- Computer helper in the core (Android spec 24, C42–C48, milestone H2):
  - `lectrice_domain::SpeechHelper` port (`state`, `open`, `make`, `cancel_all`, `close`; no network words) with
    `HelperEngine`, `HelperItem` (its `Debug` prints the text length only), `HelperState`, `HelperEvent`, `HelperEvents`.
  - `SpeechEngine::revision()` (default `""`); `KokoroSpeechEngine::REVISION` = `"sherpa-onnx-1.13.8/kokoro-1"` and
    `KokoroSpeechEngine::SAMPLE_RATE`. Clips from elsewhere are used only when id, revision and sample rate match.
  - `AudioCache::put_if_absent` (default: `get` then `put`); `FsAudioCache` stores without replacing in one atomic step
    (`persist_noclobber`), so of the phone and its computer the first writer wins.
  - `lectrice_app::speech::HelperFill`: plans 30 listening-minutes ahead (`HELPER_AHEAD_MINUTES`), asks for the
    uncached sentences nearest first within 12 items / 3 minutes in flight, stores clips through the normal encoder
    and cache, cancels the computer's queue on a jump (at most the item being made is wasted), and leaves refused items
    to the phone. `UrgentWait` and `HELPER_NOW_GRACE` (1.5 s): a miss while connected goes to the computer as urgent,
    then races the local engine. `lost_read_ahead_minutes` (green phones keep their setting, others take 30 minutes).
  - `ReadAhead::pause_for_helper` / `resume` / `is_paused`; `ReadAheadOutcome.paused`.
  - `SynthesisGate` port and `SpeechWorker::start_gated`: each job takes its turn at a shell's CPU gate with its own
    priority (the desktop ranks its `Now`, read-ahead and exports against the phones' items).
  - `services::assemble_with(adapters, settings, SpeechOptions { gate, helper, helper_grace })`; `assemble` is
    `assemble_with` with the defaults. `SpeechTicket::wait_timeout`.
  - `lectrice_infra::DeferredSpeechEngine` (+ `KokoroSpeechEngine::deferred`): id, revision, voices and sample rate
    without loading the model; loads on the first sentence or `preload`, can `unload`, refuses a model that isn't the
    promised one.
  - `lectrice_infra::helper::LinkSpeechHelper` (feature `helper-client`, off by default): the `SpeechHelper` over
    `lectrice-link`'s client with reconnects (1, 2, 5, 10, 30, then every 60 s; `retry_now`, `set_addresses`), the
    engine check on `Ready`, and `registry_name`.
  - `lectrice-link`: `client::SocketFactory` with `Session::connect_via` / `pair_via` (Android binds the socket to the
    Wi-Fi network; the link still checks the address before and the peer after), `DirectSockets`,
    `limits::reconnect_delay`, `LinkErrorName::HelperBusy` and `LinkErrorName::ALL`.
  - Prepare through the computer (file 24 §3.13): `PrepareJobs::start_on_helper` sends a pinned range to
    `HelperFill::prepare` (made after the listener's own plan, kept across a lost link; `stop_prepare`,
    `prepare_progress` → `HelperPrepareProgress`); `PrepareJobs::status` reports either kind, `on_helper` tells which.
    **Breaking:** `PrepareDeps` gains `helper`.
  - Registry names (C47): `HelperNotPaired` 5001, `HelperPairingFailed` 5002, `HelperUnreachable` 5003,
    `HelperModelMissing` 5004, `HelperIncompatible` 5005, `HelperBusy` 5006.
  - Tests: the link-drop simulator on a virtual clock (1 000 random runs, no gap, file 24 §13 assertions 1, 3, 4, 5,
    6) and the helper end to end over real sockets through `FaultyLink` (`lectrice-infra/tests/helper_link.rs`).

#### Changed
- **Breaking:** `SpeechServices` gains `prepare` and `helper`; `SpeakSentenceDeps` gains `helper` and `helper_grace`.
- `SpeakSentence` and the read-ahead store clips with `put_if_absent`; when another producer stored the sentence
  first, its clip is the one returned.
- The test profile builds `lectrice-app` optimised (the simulator's thousand runs).
- `AudioExporter` gains `attach_map` (default: adds nothing). `FfmpegExporter` implements it for M4B; a shell that wraps
  an exporter must forward it (the desktop's `SwappableExporter`) or its files carry no map.
- `ExportRecord` gains `sentences` and `marked_parts` (serde defaults; a job resumed from parts made before this
  version gets no map).

#### Fixed
- Chapter headings at the top of a page were dropped when each chapter took one page and started with its number
  (`2 Blossom` on PDF page 2, e.g. HTML printed to PDF by Chrome): the numbers were taken for printed page numbers.
  Page numbers are now inferred only from header/footer lines whose shape repeats across pages. Cleaner version 4, so
  saved books are cleaned again.

#### Security
- `mp4_map::read_audiobook` treats the file as untrusted (security review S13): box sizes are checked without overflow
  (a 64-bit size could wrap the offset), at most 200 000 top-level boxes are walked, `moov` is capped at 64 MB and
  the map at 8 MB before anything is allocated, only the 16-byte type of a foreign `uuid` box is read, the `mvhd`
  length can't overflow, and the title tag is cut to 1 KB without control characters.
- `LectriceMap::check` (new): a map from a file is refused (the file then reads as a foreign audiobook) unless its speed
  is 50–300 %, its sample rate and length are sane, it has at most `MAX_MAP_SENTENCES` (200 000) sentences, each id
  once, 8-hex-digit hashes, and start times that never go back or past the end. `match_audiobook` applies it too, so
  a timeline never has `end_ms < start_ms` or a zero speed.
- **Web server (`lectrice-api`), security review S2:** every request to the loopback server is checked before its
  route (`guard::LocalAccess`, `app::build_local_app`). `Host` must be `localhost`, `127.0.0.1` or `[::1]` with the
  server's port (or the configured page's own loopback host, for a dev server that proxies `/api`), so a page that
  reaches the server through DNS rebinding is refused. `Origin`, when sent, must be `server.allowed_origin` or the
  server itself; `POST`, `PUT` and `DELETE` need it, so a cross-site form or multipart upload is refused; a
  `Sec-Fetch-Site: cross-site` request without an allowed `Origin` is refused too. Refusals are `RequestForbidden`
  (1009, HTTP 403). **Scripts that change data with `curl` now need `-H 'Origin: http://localhost:5173'`**; reads
  (`curl localhost:7878/api/health`) work as before. `build_app` stays unguarded for shells that route requests in
  process (the desktop bridge checks its own host).
- **PDF import limits (`lectrice-infra`), security review S7:** `PdfLimits` (default 20 000 pages, 20 million
  characters, 5 minutes; `PdfiumTextSource::with_limits`, `read_runs_within`). A PDF past a limit fails with the new
  `PdfTooComplex` (1010, HTTP 422) instead of holding the import thread or the memory. The time limit is checked
  between pages and every 4 096 characters; a single PDFium call that never returns can't be interrupted in process.
- **`lectrice-link`, security review pass 3:**
  - Sample rates outside 8 000–48 000 Hz are refused in `Welcome`, `Ready` and `Clip` (every clip bound is
    `seconds × rate`: a paired computer that named 4 GHz could make the phone reserve gigabytes), and `decode_clip`
    never sizes its output beyond the longest clip the protocol allows.
  - The phone no longer queues what it didn't ask for: unasked `Ready` / `Refused` answers end the session, and at
    most 8 unread `Load` events are kept.
  - Per phone, the computer keeps at most 256 waiting items (`MAX_QUEUED_ITEMS_PER_PHONE`), stops making clips while
    8 MB of them wait unread (`CLIP_BACKLOG_BYTES`, answered `HelperBusy`), and ends a session with 16 MB unwritten
    (`MAX_OUTBOX_BYTES`): a phone that floods pings or asks for clips and never reads can't grow the computer's memory.
  - Pairing: the phone is saved **before** it is told it is paired and only while its window is still open, so the
    attempts that outlive the third wrong code can't pair, two right codes in one window save one phone, and a phone
    that hears "paired" can connect at once. Wrong codes from such late attempts still count towards the hourly limit.
    One attempt per address at a time.
  - Floods: at most 4 handshakes under way per address (`MAX_HALF_OPEN_PER_ADDRESS`); the per-address back-off list
    is capped at 1 024 entries and forgets old failures (it grew without limit, and three failures spread over days
    blocked an address). The back-off now stops sessions only: a phone the computer forgot can pair again at once.
  - Ids, versions, revisions, voice ids and error names with control or direction-changing characters are refused
    (they reach the other side's logs and screens).
  - `testing::RawPhone` / `RawComputer` (feature `testing`): hand-driven peers for malicious-peer tests
    (`tests/hostile.rs`: floods, never-reading phones, replayed frames and handshakes, garbage, lying clips).
- `LinkSpeechHelper` (`lectrice-infra`): a computer that takes the phone and drops it at once, again and again, is
  retried on the 1-2-5-10-30-60 s schedule. Before, each short session restarted the schedule (a retry every second
  for ever), and `Bye(engine_switch)` was answered with no wait at all (a busy loop). A session counts as stable
  after twice `Timing::dead_after` (30 s).
- The web server makes its data and cache folders the user's only at start (`0700` on Unix, also for folders an
  older version made; security review S11).

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
