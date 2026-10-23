# Core v1.1.0: changes for the Android split (spec §3.5)

Branch `feature/core-v1.1` (local, from `main`). Nothing committed, pushed or tagged. The uncommitted
`CLAUDE.md` and `docs/HANDOFF.md` edits from `main` came along untouched.

Spec: `docs/2026-10-23-android-spec.md` §3.5 (C1–C9), with §5.2, §8.3 and §15.2 for details.

## What changed, by crate

### `lectrice-domain`
- `ExportStatus::Paused` (+ `is_unfinished()` = queued | running | paused).
- `AudioExport` gains `parts_done`, `parts_total` (serde default, so 1.0 records load).
- `ExportRecord` gains `parts: Vec<ExportPart>` and `chapters: Vec<ChapterMark>` (default empty, skipped when empty).
- New `ExportPart { index, path, samples, sample_rate }` and `ChapterMark { title, start_ms }` (start from the book's start).
- `ExportTarget` gains `export_id`, so an exporter keeps each job's parts apart.
- **Port change (C5):** `AudioExporter` = `check_available`, `begin_part(target, part_index, sample_rate)`,
  `assemble(target, parts, chapters) -> ExportedFile`, `discard_parts(export_id)`. `ExportWriter` = `append`,
  `finish -> ExportPart` (`start_chapter` is gone: chapter marks go to `assemble`).
- **Registry (C4):** `ExporterUnavailable` → "Audio export isn't available."; new `ModelMissing` 4001 (503),
  `ModelDownloadFailed` 4002 (502), `ModelChecksumMismatch` 4003 (502), `StorageFull` 4004 (507),
  `ExportPaused` 4005 (409, operational, "Export paused; it will continue later."). Test count 20 → 25.

### `lectrice-app`
- **New `services` module (C2):** `Adapters`, `Settings`, `Services { documents, speech, exports }` (with
  `speech()` / `exports()` → `SpeechEngineUnavailable`), `DocumentServices`, `SpeechServices`, `ExportServices`
  and `assemble()`, moved from `lectrice-api`. `Settings::upload_max_bytes` is now `import_max_bytes`.
- **Resumable export (C5):**
  - `export_plan(document, read_skipped, max_sentences, part_minutes)` also cuts the plan into `parts`
    (`PartPlan { steps, sentences, spoken_chars }`). A part closes once it holds `part_minutes` of estimated 1x
    audio (18 chars/s, the read-ahead's rate), so a chapter mark at a boundary opens the next part. Same document
    + settings → same parts, which is what makes resuming safe.
  - The runner writes part by part (`begin_part` → `append` × n → `finish`), reports each finished part
    (`RunReport::part_finished`) before starting the next, then calls `assemble` with all parts and chapter marks.
    On a resume it starts at `record.parts.len()`, with the counters and sample offset of the finished parts.
    If the plan no longer matches the record (the document was re-cleaned), the job fails instead of mixing parts.
  - `ExportJobs`: `pause(id)` (queued/running → paused; the open part is dropped, finished parts kept),
    `resume_all()` (paused → queued, oldest first; returns the queued + running jobs), `cancel` now also cancels a
    paused job and deletes the parts. Starting the same export while it is paused continues that job. Interrupted
    jobs (queued/running at load) and a job running at shutdown become **paused** (were `failed`). Parts are deleted
    on done, failed and cancelled.
  - New use cases `PauseAudioExport` (ExportId → AudioExport) and `ResumeAudioExports` (() → active exports), in
    `ExportServices` as `pause` and `resume`.
  - `ExportSettings { max_sentences, part_minutes }`, `Default` = `{ None, 10 }`.
- **C8:** `speech::playback_order(document, read_skipped) -> Vec<SentenceId>` is the public one; the old
  block-index version is crate-private as `playable_in_order`.

### `lectrice-infra`
- **Features (C1)** — see the table below. `pdf`, `speech::kokoro` + `voices`, and `export` are behind their flags;
  WAV, storage, text, clock are always built. Tests that need a flag declare `required-features`.
- **`FfmpegExporter` (C5):** parts are 16-bit mono WAV (hound) at
  `<data_dir>/exports/parts/<exportId>/part-NNNN.wav`, written to a `.partial-` file and renamed on `finish`.
  `assemble` runs one ffmpeg with the concat demuxer (MP3 with tags; M4B with ffmetadata chapters, one pass instead
  of the old encode + remux). `discard_parts` removes the job's parts folder. `remove_partial_files` also cleans
  partial parts and keeps finished ones. `ffmetadata` now takes `ChapterMark` in ms.
- **C6:** `KokoroSettings::provider` (passed as sherpa's `OfflineTtsModelConfig.provider`; an empty name is
  refused).
- **C7:** `PdfiumTextSource::new` documents that a bare `libpdfium.so` lets the system linker find it (checked:
  pdfium-render passes the path straight to `libloading::Library::new`, i.e. `dlopen`).
- `speech::wav` shares `pcm16_mono` / `to_pcm16` with the part writer (removes the old copy in `ffmpeg.rs`).

### `lectrice-api`
- `composition.rs` only picks adapters from the TOML config, then calls `lectrice_app::services::assemble`, then
  `exports.resume` (so on the desktop an interrupted export continues as soon as the server is up).
- `AppState { services: Services, upload_max_bytes }`; routes use `state.services…`.
- Config: `[speech] provider = "cpu"` (validated non-empty).

### Workspace / docs
- `[workspace.package] version = "1.1.0"` (was `0.1.0` even at tag v1.0.0). Workspace `sherpa-onnx` no longer
  sets `features = ["static"]`. `Cargo.lock` changes only the four crate versions. **No new dependencies.**
- `CHANGELOG.md` (Keep a Changelog) with 1.1.0 (Core: Added / Changed / Migration; Desktop server; Web app) and 1.0.0.
- `backend/README.md`: core/shell split, feature table, export parts.

## Feature flags (`lectrice-infra`)

| Flag | Gates | Default |
|---|---|---|
| `pdfium` | `PdfiumTextSource` + the `pdfium-render` dependency | on |
| `kokoro` | `KokoroSpeechEngine`, `KokoroSettings` + the `sherpa-onnx` dependency | on |
| `sherpa-static` | `sherpa-onnx/static` (implies `kokoro`) | on |
| `ffmpeg-export` | `FfmpegExporter` (system ffmpeg) | on |

`default = ["pdfium", "kokoro", "sherpa-static", "ffmpeg-export"]`. Android: `default-features = false,
features = ["pdfium", "kokoro"]` (spec §3.2). Checked with `cargo tree -e features`: with the defaults
`sherpa-onnx feature "static"` comes from `sherpa-static`; with `pdfium,kokoro` it is absent; with no features
neither sherpa-onnx nor pdfium-render is in the tree.

Note: on Linux, sherpa-onnx-sys 1.13.8 links statically when neither `static` nor `shared` is set (its
`build.rs`: only Android/iOS default to shared). So `sherpa-static` changes nothing on the desktop today; it matters
for the spec's intent and if `shared` is ever used on desktop. No extra download was triggered.

## Decisions where the spec was open
1. **Part length** is a core rule (`PART_MINUTES = 10` in `lectrice-app`, the `ExportSettings` default), not a
   config key, so both shells get it without repeating a number. Tests use `part_minutes: 0` (one sentence per part).
2. **Parts are counted in estimated 1x audio** (spoken characters ÷ 18/s), like the read-ahead. Real part length
   varies a little; the cut is deterministic, which matters more for resume.
3. **A third port method, `discard_parts(export_id)`,** besides the spec's `begin_part` and `assemble`: the app
   layer can't touch files, and cancel must delete the parts. Parts live per export id (hence `ExportTarget.export_id`).
4. **`assemble` keeps the parts;** the job registry deletes them after it records the result (done, failed or
   cancelled). A failed job doesn't keep parts (failed is final).
5. **Interrupted jobs become `paused`, and nothing restarts by itself in the core.** The desktop shell calls
   `resume` at start; the Android worker will call it (§8.3 `resume_exports()`). `ResumeAudioExports` returns all
   queued + running jobs, so the worker knows what to wait for.
6. **Pause only affects queued/running jobs;** pausing a finished or paused job returns it unchanged (like cancel).
   `ExportPaused` is registered but not raised anywhere yet (no use case needs to refuse with it).
7. **No HTTP route for pause/resume** (the web contract only gets `partsDone`/`partsTotal` in export JSON). The web
   app doesn't know a `paused` status; the desktop never shows one, because an interrupted job is resumed before the
   server listens. If a pause route is added later, `frontend/src/types/api.ts` and the `Record<ExportStatus,…>`
   maps (`useAudioExport.ts`, `ExportPanel.tsx`) need `paused`.
8. **C4 statuses** for the new names (my choice): 503 / 502 / 502 / 507 / 409.
9. **C6 provider:** any non-empty name is passed to sherpa (it falls back to CPU itself). **The "model dir without
   `espeak-ng-data`" part is not done:** we're on sherpa-onnx 1.13.8, where a missing espeak dir can make sherpa
   exit the process (see the lexicon test). It belongs with the switch to 2.x (§14.2 calls that a core release);
   the change is in `kokoro.rs::engine_config` (make `data_dir` optional when built against 2.x).
10. **Semver:** §3.5 puts trait-breaking changes in 1.1.0. I followed the spec and said so in the CHANGELOG
    (1.0 had no outside users); from 1.1.0 on, breaking core changes raise the major version.
11. **Chapters/toc** (merged after the v1.0.0 tag) are listed in the 1.1.0 CHANGELOG too, since the tag will include them.

## Commands and results (2026-10-23, in `backend/`)

| Command | Result |
|---|---|
| `cargo build --release` | ok |
| `cargo test` (workspace) | ok: 351 passed, 0 failed, 11 ignored, 28 test binaries |
| `cargo clippy --all-targets -- -D warnings` | ok, no warnings |
| `cargo fmt --check` | ok |
| `cargo test -p lectrice-infra --test export_ffmpeg -- --include-ignored` (real ffmpeg + ffprobe) | ok: 6 passed (MP3 tags, M4B chapters at 0 s and 2 s, 3.0 s duration, no partial files) |
| `cargo clippy -p lectrice-infra --no-default-features --features "<f>" --all-targets -- -D warnings` for `""`, `pdfium,kokoro`, `ffmpeg-export`, `pdfium`, `kokoro` | ok for all five |
| `cargo test -p lectrice-infra --no-default-features` | ok: 47 passed |
| `cargo test -p lectrice-infra --no-default-features --features pdfium,kokoro` | ok: 65 passed |

One clippy failure on the way (`cloned_ref_to_slice_refs` in a new test) and one unused-import error with
`--no-default-features` (`safe_component`, now behind `ffmpeg-export`); both fixed. Ignored book tests not run.

**Smoke test.** Port 7878 was already taken by a `lectrice-api` the user started at 20:49 (pid 1557199, running
the old binary). I left it running and used port 7879 with a temporary data dir in my scratchpad:
- `/api/health` → `{"status":"ok","speechEngine":"ready"}`; uploaded the smallest book from `~/Desktop`
  (431 pages, 8031 sentences); `GET /api/documents/{id}` → 3453 blocks, 390 toc entries, cleaner 3; sentence audio
  → 200 `audio/wav`.
- **Resume check:** started an M4B export with `LECTRICE_EXPORT_MAX_SENTENCES=40`, sent SIGTERM while it ran →
  log "export paused by shutdown", record on disk `paused`. Restarted → resumed at start, `done` 40/40, parts 1/1,
  file downloads (200, 1.9 MB, aac, 211.7 s), parts folder empty afterwards.
- Stopped my server and deleted the temp data. Only counts were printed; no book text went anywhere.

`cargo build --release` replaced `target/release/lectrice-api` on disk; the user's running server keeps the old
binary in memory until it is restarted.

## Left for the `lectrice-ffi` step
- The ffi `AudioExporter` adapter (`export_sink.rs`): map `begin_part` / `append` / `finish` / `assemble` /
  `discard_parts` onto the Kotlin `AudioEncoderSink` (§5.2). `finish_part` there should return the sample count
  so the core can fill `ExportPart.samples` (or count in the adapter); chapter `start_ms` is already absolute.
  `abandon_part` = dropping the writer.
- The ffi composition root builds `lectrice_app::services::Adapters` with `FsDocumentRepository`, `FsAudioCache`,
  `FsExportRecordStore` (paths under `filesDir/lectrice`), `PdfiumTextSource::new("libpdfium.so")` and
  `KokoroSettings { provider }`, then calls `assemble`. `resume_exports()` → `exports.resume`,
  `pause_export(id)` → `exports.pause`, `playback_order` → `lectrice_app::speech::playback_order`.
- `CoreErrorInfo` from `ErrorName::ALL` / `spec()`; `core_version()` from `CARGO_PKG_VERSION` of a core crate.
- `ExportFormat` stays `{ Mp3, M4b }` in the core; the ffi maps only `M4b` (§5.2).
- Not done here (see decision 9): accepting a model dir without `espeak-ng-data` when moving to sherpa-onnx 2.x.
- `StorageFull` is registered but no adapter maps an out-of-space I/O error to it yet (`storage_failed` takes any
  error type); worth doing when the phone's export path is built.
- Docs not touched (another agent is working in `docs/`): `docs/2026-10-21-design-spec.md` §8 still has the old
  `ExporterUnavailable` text and doesn't list the 4xxx names or `partsDone`/`partsTotal`.
- Tag `v1.1.0` after the user reviews and commits (not created).
