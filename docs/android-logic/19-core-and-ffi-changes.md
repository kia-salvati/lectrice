# 19 · Core and FFI changes

Status: **decided** (C20, C21 conditional on the spike; C23 proposed). Numbering continues the spec's C1–C9
(`docs/2026-10-23-android-spec.md` §3.5, done on `feature/core-v1.1`, not tagged). **Release: every change in this file
goes into core `v0.2.0`** together with C1–C9 (user 2026-10-24); nothing waits for a later release.

## 1. Goal

List, in one place, every change to the **shared Rust core** (main repo: `lectrice-domain`, `lectrice-app`,
`lectrice-infra`) and to the **`lectrice-ffi` crate** (Android repo) that the Android features need, so that core work is
done once, tested in the main repo, released by tag, and never forked into the Android repo.

## 2. User flow

None directly. The flow for the developer:

1. Make the core change on a `feature/<name>` branch of the main repo, with domain/app/infra tests; the desktop still builds
   and its tests pass (`cargo test`, `clippy -D warnings`, `fmt --check`).
2. Add a CHANGELOG "Core" entry (Added / Changed / Migration).
3. Tag **`v0.2.0`** once every non-conditional row of §3.0 is merged (user 2026-10-24: all of them go into `v0.2.0`; the
   breaking ones are listed under the CHANGELOG's Migration heading). A conditional change that is needed later (C20, C21,
   C6b) ships as `v0.2.x`/`v0.3.0` by the CHANGELOG's semver rule.
4. In `lectrice-android`: bump the tag, `cargo update -p lectrice-app -p lectrice-domain -p lectrice-infra`, update
   `lectrice-ffi`, regenerate the Kotlin bindings, build, run the JVM tests (spec §3.4).

## 3. Rules & logic

### 3.0 Summary table

| # | Change | Crate(s) | Breaking? | Needed by |
|---|---|---|---|---|
| **C10** | **Baked-in speed for exports (time-stretch, Sonic)** | domain, app, infra | yes (export record, start args) | 10 |
| C11 | Engine id per model bundle | infra | no (new field with default) | 01, 08, 11, 16 |
| C12 | Exports tied to a bundle; jobs wait while another bundle is loaded | domain, app | yes | 10 |
| C13 | Read-ahead control: `StopReadAhead`, `StopAllReadAhead`, `SpeakArgs.plan_ahead` | app | no (new field with default) | 08, 09, 21 |
| C14 | Change read-ahead minutes at runtime (incl. the governor's 2 min) | app | no | 08, 11 |
| C15 | Storage usage (library, cache, exports, **per book**) + `ClearAudioCache` | domain (port), app, infra | yes (`AudioRetention` port) | 11, 20 |
| C16 | `SpeakVoiceSample` use case, **disk-cached on demand** | app, infra | no | 01, 16 |
| C17 | Speech stats (sustained RTF) | app | no | 10, 12 |
| C18 | `EstimateAudioExport` use case | app | no | 10 |
| C19 | `DeleteAudioExport`, list exports of every document, file size, **location + `MarkExportSaved`** | domain, app, infra | yes (`AudioExport` fields) | 10, 11, 20 |
| C20 | Import progress (**conditional**: spike M-h > 15 s) | domain (port), app, infra | yes | 03 |
| C21 | Prepare jobs (**conditional**: spike RTF amber/red) | app | no | 14 |
| C22 | Map out-of-space I/O errors to `StorageFull` | infra | no | 03, 10, 12 |
| C23 | Export chapter marks from the `toc` (**proposed**) | app | no (output only) | 10 |
| C24 | Export pause reasons; `StorageFull` pauses **at every stage** (parts kept); `stage` and `error` on the export | domain, app | yes | 10 |
| C25 | "Ready ahead" in the sentence result (for the buffered part of the slider) | app | no (new field) | 05 |
| C26 | New registry name `FileUnreadable` 4006 | domain | no | 03 |
| C27 | `cleaner_version` on the document summary + the current cleaner version | domain, app | yes (field) | 03, 13 |
| **C28** | **Remove a document** (soft remove, restore, purge) + clear one book's audio | domain (ports), app, infra | yes (`DocumentWriter`, `AudioRetention` methods; `meta.json` field) | 20, 21 |
| **C29** | **Audio cache under its own root** (`cache_dir`, Android's `cacheDir`); tolerate clips the OS deleted | infra, ffi config | no (new config key with default = `data_dir`) | 08, 20 |
| **C30** | **Partial export** (sentence range) + per-chapter estimates | domain, app | yes (`StartAudioExportArgs`, record field) | 10 |
| C6b | (from spec C6) model dir without `espeak-ng-data` on sherpa-onnx 2.x (**conditional**: spike M-k, M-l) | infra | no | 01 |

### 3.1 C10 · Baked-in speed for exports (time-stretch)

**Why.** The user can pick a speed (0.75–2×) for an exported audiobook, and the file then plays at that speed everywhere,
at the same pitch (v2 brief §1.4, user-approved design). Today audio is always made at 1× and only the player changes
speed (ExoPlayer's Sonic on Android, the browser on the desktop). Kokoro's own `speed` input would sound slightly more
natural, but its clips can't be reused from the 1× cache, so it is not used.

**Where the stretch happens.** In the export runner (`lectrice-app/src/exports/run.rs`), **between the clip and the part
writer**: each sentence's 1× PCM goes through a stretcher, and the stretched PCM is appended to the part. The exporter
adapters (`FfmpegExporter`, the Android `AacEncoderSink`) don't change: they still receive PCM.

**New domain port** (`lectrice-domain/src/ports.rs`):

```rust
/// Changes the speed of mono PCM without changing its pitch. One stretcher per export part (it keeps state between pushes).
pub trait TimeStretcher: Send {
    /// Feeds samples; appends whatever output is ready to `out`.
    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>);
    /// Ends the stream; appends the remaining output.
    fn flush(&mut self, out: &mut Vec<f32>);
}

pub trait TimeStretch: Send + Sync {
    fn stretcher(&self, sample_rate: u32, speed: ExportSpeed) -> Box<dyn TimeStretcher>;
}
```

**New domain type** `ExportSpeed(u16)` (percent): valid values exactly **75, 100, 125, 150, 175, 200**. Anything else is
refused with `RequestInvalid`. `ExportSpeed::NORMAL = 100`. `as_factor() -> f32` = percent ÷ 100.

**Record and args:**

- `AudioExport.speed: ExportSpeed` (serde field `speedPercent`, default 100 so 0.1/0.2 records load).
- `StartAudioExportArgs.speed: ExportSpeed`.
- `ExportTarget.speed: ExportSpeed`.
- The **same-target rule** in `ExportJobs::enqueue` (today: same document + voice + format) adds **speed** and **engine id**
  (C12). A paused job continues only if all five match.
- **Target file name:** unchanged for speed 100 on the default engine (`exports/<docId>/<voiceId>.<ext>`, so desktop files keep
  their name). Otherwise `exports/<docId>/<voiceId>.<engineId>.<speed>.<ext>`, e.g. `af_heart.kokoro-v1.0-int8.150.m4b`.

**Runner rules:**

1. Speed 100 → **no stretcher** (bypass; output bytes identical to today).
2. Otherwise, at the start of each part: `stretcher = time_stretch.stretcher(sample_rate, speed)`. Each sentence (and each
   silence the runner inserts) is `push`ed; the output goes to `writer.append`. At the end of the part: `flush`, append, then
   `writer.finish()`.
3. **Parts are planned exactly as now** (by estimated 1× audio, `part_minutes = 10`), so the plan stays deterministic and
   resuming is safe. At 2× a part holds about 5 minutes of output.
4. **Chapter marks:** `start_ms = round(input_samples_before_mark × 1000 ÷ sample_rate ÷ speed_factor)`, where
   `input_samples_before_mark` is the 1× sample count from the book's start. Computing from input (not from output) avoids
   the stretcher's internal delay. The error against the real output is under one Sonic frame (≈ 10 ms) per part.
5. `ExportPart.samples` counts **output** samples (what the file holds); progress and time-left still count spoken
   characters, as today.
6. Resume reads the speed from the record; the plan and the stretcher are rebuilt with it, so a resumed file is consistent.
7. Cached 1× clips are reused as today (the cache never holds stretched audio).

**Adapter:** `SonicTimeStretch` in `lectrice-infra/src/audio/sonic.rs`, behind a new feature **`time-stretch`** (in
`default`, and turned on for Android: `features = ["pdfium", "kokoro", "time-stretch"]`).

- The algorithm is **Sonic** (Bill Cox, Apache-2.0), the same one ExoPlayer uses (`androidx.media3.common.audio.Sonic`,
  Apache-2.0), so an exported 1.5× file sounds like in-app listening at 1.5×. Only the speed path is needed (pitch 1.0,
  rate 1.0, no volume), mono `f32`.
- Implementation: port the speed path of ExoPlayer's `Sonic.java` to Rust (about 400 lines), with a header naming the
  source and its Apache-2.0 license, and a NOTICE entry. Whether a maintained Sonic crate exists on crates.io is UNVERIFIED;
  if one does, it may be used only if it is Apache-2.0/MIT, has no `unsafe` C build, and passes the tests below.
- `lectrice-app` gets it through `Adapters.time_stretch: Arc<dyn TimeStretch>`.

**Tests (main repo):**

| Test | Pass |
|---|---|
| 1 s of a 220 Hz sine at 24 kHz, speed 150 | output length 0.667 s ± 1 %; dominant frequency 220 Hz ± 2 % (zero-crossing count) |
| Speeds 75, 125, 175, 200 | length = input ÷ factor ± 1 % |
| Speed 100 | the runner never builds a stretcher; output equals input |
| Chunking | pushing 1 s in 1 chunk vs 50 chunks: lengths within one frame |
| Chapter marks | a fake 3-part export at 200: marks are the 1× marks halved (± 10 ms) |
| Resume | pause after part 1 at speed 150, resume: part 2 uses speed 150 (from the record) |
| Same target | starting doc+voice+format+engine at 125 while a 150 job is paused creates a new job |
| `ExportSpeed` | 76 and 0 are refused with `RequestInvalid` |
| Desktop | `FfmpegExporter` test with speed 150: ffprobe duration = 1× duration ÷ 1.5 ± 2 % |

The desktop web API may gain `speed` on `POST /api/documents/{id}/exports` later; not required now (default 100).

### 3.2 C11 · Engine id per model bundle

- Today `KokoroSpeechEngine::id()` is the constant `"kokoro-v1.0"`. The audio cache key includes the engine id, so two
  bundles would share clips. **Rule: a clip made by one bundle is never played or exported as the other.**
- `KokoroSettings.variant: KokoroVariant { Fp32, Int8 }` (default `Fp32`). Ids: `Fp32 → "kokoro-v1.0"` (unchanged, so desktop
  caches stay valid), `Int8 → "kokoro-v1.0-int8"`. The mapping lives inside the adapter (nothing else knows Kokoro).
- The ffi `EngineConfig::Kokoro` gains `variant`.

### 3.3 C12 · Exports tied to a bundle

- `AudioExport.engine_id: String` (serde default `"kokoro-v1.0"`). `StartAudioExportArgs.engine_id: Option<String>`: `None` =
  the loaded engine.
- Voices are checked against the loaded engine (both bundles expose the same 28 English voice ids; UNVERIFIED for the int8
  bundle's `voices.bin`: the spike checks it; if they differ, `StartAudioExport` refuses an unknown voice with `VoiceUnknown`).
- `ExportJobs` runs the **oldest queued job whose `engine_id` equals the loaded engine's id**. Other queued jobs wait.
  `AudioExport.waiting_for_engine: bool` (computed when the export is read, not stored) tells the UI.
- The core still holds **one engine**. Swapping bundles = the ffi rebuilding the speech + export services with the new
  engine (spec §5.2 `reload_engine`). A running job is paused by that (reason `EngineSwitch`, C24) and comes back with
  `resume_all`.

### 3.4 C13 · Read-ahead control

- `stop_read_ahead(document_id)`: cancels every **queued** read-ahead job (priority `ReadAhead`) for that document, any
  voice, any engine. The one being synthesized finishes and is cached (it can't be interrupted).
- `stop_all_read_ahead()`: the same for every document (bundle switch, thermal CRITICAL).
- `SpeakArgs.plan_ahead: bool` (serde/ffi default `true`). `false` = synthesize or return this sentence only: no read-ahead
  plan, no cancelling of queued jobs, no `retain_around`. ExoPlayer keeps loading items while paused; with `false` those
  loads can't restart a stopped read-ahead.
- Nothing in the core restarts read-ahead by itself: only a `speak_sentence` with `plan_ahead = true` plans.
- The shell's rules (when to stop, reduce and resume: paused 60 s, sleep timer, book change, removal, battery saver / low
  battery, thermal SEVERE / CRITICAL, low memory) are the `ReadAheadGovernor`'s, file 08 §3.3. They stay in Kotlin (they
  read Android signals); the core only gets the three calls above and C14.
- Tests: after `stop_read_ahead`, the queue holds no `ReadAhead` job of that document; a `plan_ahead = false` speak plans
  nothing; a later `true` speak plans again.

### 3.5 C14 · Read-ahead minutes at runtime

- `ReadAhead::set_read_ahead_minutes(minutes)`; `minutes` in `1..=60`; refused with `RequestInvalid` unless
  `keep_minutes_per_document ≥ minutes × 1.5` (the existing `MIN_KEEP_TO_READ_AHEAD_RATIO`). With the phone's `keep = 60`,
  the settings 5 / 10 / 20 and the governor's **Reduced = 2** all pass.
- Takes effect at the next `speak_sentence` (which re-plans). Lowering it also drops queued jobs beyond the new horizon.

### 3.6 C15 · Storage usage and clearing the cache

- `AudioRetention` gains `usage() -> CacheUsage { bytes, clips }` and `clear() -> AudioEviction` (deletes every clip not
  being read or written right now).
- New use case `GetStorageUsage -> StorageUsage { library_documents, library_bytes, cache_bytes, cache_cap_bytes,
  export_files, export_bytes, export_parts_bytes, documents: Vec<DocumentUsage { document_id, library_bytes, cache_bytes }> }`
  (library from `DocumentReader`, bytes = `original.pdf` + `document.json` + `meta.json` sizes; cache per document from the
  `AudioRetention` index after a refresh (C29); exports from the export records' `file_bytes`, C19, counting only files
  still in app storage; parts from the parts folders). Removed documents (C28) are skipped.
- New use case `ClearAudioCache`. Books, positions and exports are untouched.
- The global cap stays a setting (`cache_max_mb`); the phone passes **1024**.

### 3.7 C16 · `SpeakVoiceSample`

- `SpeechServices.speak_voice_sample(voice_id) -> Vec<u8>` (WAV). **One fixed sample sentence for every voice and every
  model**, a core constant (our own invented line, first brief §10):
  `VOICE_SAMPLE_TEXT = "This is how I sound when I read your books aloud."` with `VOICE_SAMPLE_VERSION = 1` (bump it if the
  text ever changes, so old samples aren't reused).
- **Made on demand, then kept on disk** (decided 2026-10-24): the first tap on a voice's sample button synthesizes it
  (priority `Now`, ~2–3 s of audio, so about 1–2 s on a green phone, plus the engine load the very first time); it is then
  stored as `<cache_dir>/samples/<engineId>/<voiceId>.v<VOICE_SAMPLE_VERSION>.wav` (C29's cache root) and every later tap
  plays it at once, also after a restart. Not part of the sentence cache's window or cap (28 voices × 2 bundles ≈ 8 MB at
  most).
- **Why not all voices right after the model download:** 28 syntheses (about 40–60 s of full CPU on a green phone, minutes
  on a red one) at the moment the user wants to start, for voices most people never try; and the samples would still be
  lost whenever Android clears the cache. On demand costs one short wait (the button's spinner is designed for it) and
  nothing for voices never heard. The default voice's sample is made anyway on S4 (file 01), so the first sample a new
  user hears later is instant.
- Errors: `VoiceUnknown`, `SpeechEngineUnavailable`, `SpeechSynthesisFailed`. A failed disk write is logged and the sample
  still plays (from memory).

### 3.8 C17 · Speech stats

- `SpeechWorker` keeps the last **50** synthesized jobs (1× audio ms, synthesis ms; cache hits are not jobs).
- `speech_stats() -> SpeechStats { engine_id, rtf_median: Option<f32>, samples: u32, engine_load_ms: Option<u32> }`.
  `rtf_median` is `None` below 10 samples.

### 3.9 C18 · `EstimateAudioExport`

- Args: `document_id, voice_id, engine_id, read_skipped, speed`. Output: `ExportEstimate { sentences, parts, audio_ms_1x,
  output_ms, cached_ms_1x }`.
- `audio_ms_1x` from the export plan's spoken characters ÷ 18 chars/s; `output_ms = audio_ms_1x ÷ speed`; `cached_ms_1x` =
  the same estimate over the plan's sentences whose clip is in the cache for that voice + engine (`AudioRetention::contains`).
- Pure plan + `contains` checks; no synthesis. Expected to take < 300 ms for an 8 000-sentence book (measure; if slower,
  skip `cached_ms_1x` and return 0).

### 3.10 C19 · Deleting exports, listing all, file size

- `AudioExport.file_bytes: Option<u64>` (set when done).
- `AudioExport.location: Option<ExportLocation>` with `ExportLocation { AppStorage { path }, Shared { uri: String } }`
  (serde default: `AppStorage` with the existing `file` path when done). `uri` is an opaque string the shell gave; the core
  never opens it.
- `MarkExportSaved(export_id, uri)`: for a `done` export whose file the shell has **moved** to shared storage (file 20
  §3.4): sets `location = Shared { uri }`, clears the app-storage path. The shell deletes the private file first.
- `AudioExport.title: String`: the book title, stored when the export starts (today the title lives only in the
  `ExportTarget` built at run time from the document). Serde default `""` for old records; the shell then shows the
  document's title if the book still exists, else **new** "Removed book".
- `ListAllAudioExports` (every document, newest first, **including documents that were removed**, C28) for Settings →
  Storage.
- `DeleteAudioExport(export_id)`: only for `done`, `failed` or `cancelled`; deletes the file if it's in app storage and the
  record (a `Shared` file is deleted by the shell through MediaStore before this call). An unfinished export is refused
  with `RequestInvalid` (cancel it first).

### 3.11 C20 · Import progress (conditional)

Built **only if** the spike's largest import on the S10+ takes > 15 s (spike plan §6, M-h; spec §9). Then:
`PdfTextSource::read_runs` takes a `&dyn ImportProgress` (`fn pages_read(&self, done: u32, total: u32)`), called at most
every 250 ms; the ffi forwards it to a Kotlin `ImportListener`. The cleaning step after reading is reported as one step
("Cleaning text").

### 3.12 C21 · Prepare jobs (conditional)

Built **only if** the spike puts a phone the user cares about in the amber or red RTF band. Behaviour in file 14 §3:

- `StartPrepare { document_id, voice_id, engine_id, read_skipped, from_sentence, to_sentence }`, `CancelPrepare`,
  `PrepareStatus` (sentences done / total, `ready_until_sentence_id`); jobs at the export priority (3), clips written to the
  audio cache like read-ahead clips; a small `prepare/<docId>.json` record so a restart keeps the range.
- **Pinning:** `AudioRetention::retain_around` gains `pinned: Option<Range<SentenceId>>`; clips in the pinned range of the
  position's document + voice + engine are never evicted by the window (the global cap still applies, stopping the job at
  cap − 100 MB).

### 3.13 C22 · `StorageFull` from the file system

Every `lectrice-infra` storage adapter (document repository, audio cache, export records, part writers) maps an I/O error
of kind `StorageFull` (or raw OS error `ENOSPC` = 28) to `AppError::new(StorageFull).and(StorageFailed)`, so the shells see
`StorageFull` as the primary name. (`io::ErrorKind::StorageFull` is stable in current Rust; the raw-code check covers older
toolchains.)

### 3.14 C23 · Chapter marks from the TOC (proposed)

Today `export_plan` marks headings of level ≤ 2. Proposed: mark every `toc` entry of level ≤ 2 at its `sentence_id`
(title as in the TOC, cut at 120 chars); use headings only when the TOC is empty. Then the M4B chapters match the chapters
sheet and skip front matter the TOC already drops. Desktop exports change the same way (CHANGELOG "Changed").

### 3.15 C24 · Pause reasons; out of space pauses

- `AudioExport.paused_by: Option<PauseReason>` with `PauseReason { User, System, StorageFull, EngineSwitch }` (serde,
  default `None`; a 0.2 `paused` record loads as `System`).
- `AudioExport.error: Option<ErrorName>`: the error that stopped it (`StorageFull` while paused; any name when `failed`).
- `PauseAudioExport(id, reason)`. `ResumeAudioExports` (the worker's "continue what you can") resumes only `System` and
  `EngineSwitch` pauses. New `ResumeAudioExport(id)` resumes any paused job (the user's "Continue").
- `AudioExport.stage: ExportStage { Parts, Assemble }` (serde default `Parts`).
- **A `StorageFull` error at any step pauses the job** with reason `StorageFull` and **keeps every finished part**: writing
  or finishing a part (the open `.partial` is dropped), saving the record, `assemble` (the partly written target is deleted;
  `stage = Assemble`), writing the chapter box. In `ExportJobs::complete`, a `RunOutcome::Failed` whose primary name is
  `StorageFull` (C22) becomes `Paused` and **does not call `discard_parts`**. This fixes the contradiction where E5 said
  "nothing is lost" but the 0.2 runner failed the job and deleted its parts on any error.
- Other errors still fail it (failed is final; parts deleted).
- Resuming an `Assemble`-stage job skips straight to assemble (all parts are recorded).
- Tests: a fake writer that returns `StorageFull` on part 3 → paused, parts 1–2 kept on disk and in the record; resume →
  starts at part 3. A fake `assemble` returning `StorageFull` → paused at `Assemble`, every part kept, no target file;
  resume → assembles only.

### 3.16 C25 · "Ready ahead"

`SpokenSentence` (and the ffi `SpokenAudio`) gains `ready_ahead_ms: u32`: the estimated 1× audio of the **contiguous**
cached sentences right after this one in playback order, as the read-ahead planner already sees it (no extra I/O).
The player sheet's slider draws it as the buffered part. If skipped, the slider simply has no buffered part.

### 3.17 C26 · `FileUnreadable`

Registry: `FileUnreadable => (4006, 422, true, "This file couldn't be opened.")`. Raised by the Kotlin import when the
content resolver can't open or read the picked file (permission revoked, a cloud file that failed to download). Lives in
the Rust registry so the mirror test keeps both equal.

### 3.18 C27 · Cleaner version on the summary

`DocumentSummary.cleaner_version: u32` and a `current_cleaner_version()` service function, so the UI knows **before** it
calls `get_document` that the book will be cleaned again (and shows "Updating this book's text…", file 03).

### 3.19 C28 · Remove a document

**Why.** "Remove from library" is in v1 (user 2026-10-24; file 21). Phones are small: a book is its PDF copy, the cleaned
text and up to hundreds of MB of audio.

**Ports** (`lectrice-domain/src/ports.rs`, `audio_retention.rs`):

- `DocumentWriter::set_removed(id, removed_at: Option<Timestamp>)` (writes `meta.json`'s new `removedAt`, serde default
  `null`) and `DocumentWriter::delete(id)` (deletes `library/<docId>/`, `meta.json` first so a crash halfway never leaves a
  listed half-book).
- `DocumentReader`: `list_recent` skips removed documents; `find` / `find_summary` return `None` for them (so
  `get_document` gives `DocumentNotFound`); new `list_removed() -> Vec<DocumentId>`. `CachedLibrary` drops its cached entry
  on `set_removed` and `delete`.
- `AudioRetention::remove_document(id) -> AudioEviction`: deletes every clip of that document (all cleaner versions,
  engines, voices), skipping clips in use (they're deleted when released). A `put` for a removed document is dropped.

**Use cases** (`lectrice-app`, `LibraryServices`):

- `RemoveDocument(id)`: `set_removed(now)`; `stop_read_ahead(id)` (C13). Unfinished exports aren't touched here (the shell
  pauses them, file 21 §3.2).
- `RestoreDocument(id)`: `set_removed(None)`. Also called by `ImportPdf` when the imported PDF's id is removed but not
  purged (no re-cleaning).
- `PurgeRemovedDocuments { delete_exports_for: Vec<DocumentId> }`: for every removed document: cancel and drop its
  unfinished exports (parts deleted); delete its finished exports' files and records **only** if listed in
  `delete_exports_for` (else kept, still listed by C19); `AudioRetention::remove_document`; `DocumentWriter::delete`. Called
  by the shell after the undo window, and by the core itself at `open` (startup) with an empty list (crash safety: exports
  kept).
- `ClearDocumentAudio(id) -> AudioEviction`: `AudioRetention::remove_document` without removing the book (Storage screen
  "Clear this book's audio").

**Tests:** remove → not in `list_recent`, `get_document` = `DocumentNotFound`; restore → back with its position; purge →
folder and clips gone, kept exports still listed with their title; import of a removed-not-purged PDF restores it without
cleaning; startup purges removed documents. The desktop web API may expose it later (`DELETE /api/documents/{id}`); not
required now.

### 3.20 C29 · The audio cache under its own root

- `FsAudioCache::open(cache_dir, max_bytes)` takes its own root, from a new config key `storage.cache_dir` (default:
  `data_dir`, so the desktop's layout is unchanged). The ffi `CoreConfig` gains `cache_dir`; Android passes
  `cacheDir/lectrice` (file 20 §3.1), so Android shows the clips as the app's "Cache".
- **Android may delete any file there at any time.** The adapter already treats a missing file on `get` as a miss and drops
  it from the index; added:
  - an **empty or unreadable** clip on `get` is a miss too (deleted and dropped from the index), never returned as audio;
  - `AudioRetention::refresh()`: re-scans the root and rebuilds the index; called by `GetStorageUsage`, `EstimateAudioExport`
    and `ClearAudioCache`, so their numbers don't count clips the OS removed;
  - `put` re-creates missing directories (already the case) and never fails because a parent vanished between calls.
- Voice samples (C16) live under `<cache_dir>/samples/`; the sentence cache's scan ignores that folder.
- Tests: delete clip files behind the cache's back → `get` misses and re-synthesis works; `refresh` → usage drops; a
  0-byte clip → miss, not audio.

### 3.21 C30 · Partial export (a range)

- `StartAudioExportArgs.range: Option<ExportRange { from_sentence: SentenceId, to_sentence: SentenceId }>` (inclusive, in
  playback order; `None` = whole book). Refused with `RequestInvalid` if `from > to` or outside the document.
- `AudioExport.range: Option<ExportRange>` (serde default `None`); `ExportTarget.range` too. The **same-target rule** adds
  the range.
- The plan (`export_plan`) keeps only sentences in the range; **chapter marks** only for entries whose `sentence_id` is in
  the range, timed from the range start (the first mark at 0 if the range starts mid-chapter, titled with that chapter);
  title metadata `"{title} ({label})"` where the shell passes the label in `StartAudioExportArgs.range_label:
  Option<String>` (e.g. "Ch. 7–12"; stored as `AudioExport.range_label`), since only the shell knows the book's chapter
  numbering.
- Target file name adds `.s<from>-<to>` before the extension when a range is set.
- `EstimateAudioExport` takes the same `range` and also returns two **cumulative** tables over the whole book's export plan
  (same voice, read-code choice): `toc_offsets: Vec<TocOffset { toc_index: u32, first_sentence: SentenceId,
  audio_ms_1x_before: u64 }>` (one per TOC entry) and `part_offsets: Vec<PartOffset { first_sentence, page_label,
  audio_ms_1x_before }>` (one per planned part, for books without a TOC), plus `total_audio_ms_1x`. Any range's 1× length is
  then a difference of two offsets, so the shell can size every chapter without another call. Which TOC entries count as
  chapters stays the shell's rule (file 07 `isChapterNode`); the core doesn't duplicate it. File 10 §3.11 uses these.
- Tests: a range export plans only its sentences; marks shift to the range start; the same book's full and partial exports
  are different targets.

### 3.22 FFI changes (Android repo, `lectrice-ffi`) vs spec §5.2

| Item | Change |
|---|---|
| `CoreConfig` | as in the spec; `cache_max_mb = 1024`; `read_ahead_minutes` from Settings; **+ `cache_dir`** = `cacheDir/lectrice` (C29) |
| `EngineConfig::Kokoro` | + `variant: KokoroVariant` (C11) |
| `SpokenAudio` | + `ready_ahead_ms` (C25) |
| `SpeakArgs` | + `plan_ahead: bool` (default true) (C13) |
| `ExportFormat` | `{ M4b }` only (the core keeps `Mp3` for the desktop) |
| `start_export` | `(document_id, voice_id, variant, speed_percent: u16, read_skipped, range: ExportRangeRecord?, range_label: String?)` (format dropped) (C10, C12, C30) |
| `AudioExportRecord` | + `speed_percent`, `engine_id`, `range`, `paused_by`, `stage`, `waiting_for_engine`, `file_bytes`, `location`, `error`, `title` (C10, C12, C19, C24, C30) |
| `pause_export(id)` | reason `User`; new `pause_exports_for_system()` (the worker's `onStopped`) |
| `resume_exports()` | resumes `System` and `EngineSwitch` only (C24); new `resume_export(id)` |
| new | `delete_export(id)`, `list_all_exports()`, `mark_export_saved(id, uri)` (C19) |
| new | `estimate_export(document_id, voice_id, variant, read_skipped, speed_percent, range?) -> ExportEstimateRecord` (with `toc_offsets`, `part_offsets`) (C18, C30) |
| new | `speak_voice_sample(voice_id) -> SpokenAudio` (C16; disk-cached) |
| new | `speech_stats() -> SpeechStatsRecord` (C17) |
| `stop_read_ahead(doc)` | now backed by C13; new `stop_all_read_ahead()` |
| new | `set_read_ahead_minutes(m)` (C14) |
| `cache_usage_mb()` | replaced by `storage_usage() -> StorageUsageRecord` (with per-book `documents`) (C15, C28) |
| `clear_audio_cache()` | backed by C15 |
| `DocumentSummaryRecord` | + `cleaner_version`; new `current_cleaner_version()` (C27) |
| new | `remove_document(id)`, `restore_document(id)`, `purge_removed_documents(delete_exports_for: List<String>)`, `clear_document_audio(id)` (C28) |
| `AudioEncoderSink.finish_part` | returns the part's sample count (core progress note) |
| `import_pdf` | + optional `ImportListener` (C20, conditional) |
| new (conditional) | `start_prepare`, `cancel_prepare` (C21, file 14) |

## 4. States

Not applicable (no UI). Each change's error behaviour is in §3.

## 5. Edge cases

- **Old records:** every new field has a serde default (speed 100, engine `kokoro-v1.0`, no pause reason → `System` for a
  stored `paused`, stage `Parts`, no range, location `AppStorage` with the existing path), so 0.1/0.2 export records load;
  `meta.json` without `removedAt` loads as not removed.
- **A clip deleted by Android while it's being read:** `get` already holds the bytes or misses; never a half clip (atomic
  writes, C29 empty-clip rule).
- **Purge while an export of that book is running:** the shell paused it first (file 21); purge cancels it (parts deleted)
  before deleting the library folder, so the runner never reads a vanished document.
- **Startup purge vs a slow shell:** the core purges removed documents at `open`; a book the user undid just before a crash
  was already restored (`restore` writes `meta.json` at once).
- **A paused export whose bundle was deleted:** the model page refuses to delete a bundle that an unfinished export uses
  (file 11), so this can't happen from the UI. If it happens anyway (files removed by hand), the job stays queued and
  `waiting_for_engine` is true forever; the export page offers "Cancel export".
- **Re-cleaned document under a paused export:** already handled: the plan no longer matches → the job fails
  (`Internal`, "the document changed").
- **Sonic at the end of a part:** `flush` drains the stretcher, so no audio is lost between parts; each part starts a fresh
  stretcher (a tiny discontinuity at part joins, < 10 ms, inaudible between sentences).

## 6. Where it lives

All of §3.1–§3.21 in the main repo (`backend/crates/`). §3.22 in `lectrice-android/rust/lectrice-ffi`. No change to
`lectrice-api` except wiring the new adapter (`SonicTimeStretch`) and the new config keys' defaults (`storage.cache_dir`
defaults to `data_dir`).

## 7. Data and persistence

- `exports/<exportId>.json` (export records): new fields `speedPercent`, `engineId`, `range`, `rangeLabel`, `title`,
  `pausedBy`, `stage`, `error`, `fileBytes`, `location`.
- Export file names as in §3.1 and §3.21 (`.s<from>-<to>` for a range).
- `meta.json`: new `removedAt` (C28). `document.json` is unchanged.
- The audio cache root can differ from `data_dir` (C29); on Android it is `cacheDir/lectrice/audio/`, samples in
  `cacheDir/lectrice/samples/`.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Speed is baked in with a time-stretch after synthesis (Sonic), not Kokoro's speed input | Opus, in the approved v2 brief | 2026-10-23 |
| One engine in memory; an export for the other bundle waits | v2 brief (approved) | 2026-10-23 |
| int8 gets its own engine id; fp32 keeps `kokoro-v1.0` | Opus | 2026-10-24 |
| `StorageFull` pauses an export **at every stage** (keeps parts) | Opus, from the approved E5 copy + user | 2026-10-24 |
| C23 chapter marks from the TOC | Opus, **proposed** | 2026-10-24 |
| **All changes in this file ship in core `v0.2.0`** (closes the earlier open question) | user | 2026-10-24 |
| Remove a document: soft remove + restore + purge (C28) | user (feature) + Opus (shape) | 2026-10-24 |
| Audio cache root separate (C29), Android's `cacheDir` | user (visible in Settings) + Opus | 2026-10-24 |
| Voice sample: one fixed sentence, made on demand, disk-cached (C16) | user (fixed sentence) + Opus (on demand) | 2026-10-24 |
| Read-ahead control in the core is only stop / stop-all / `plan_ahead` / minutes; the when-rules stay in Kotlin (C13) | Opus | 2026-10-24 |
| Partial export by sentence range with cumulative estimates (C30) | user (feature) + Opus (shape) | 2026-10-24 |

No open questions in this file.

## 9. Done when

- [ ] Every row of §3.0 that isn't conditional is merged in the main repo with its tests; the desktop builds and its full
      test suite, clippy and fmt pass with default features and with `--no-default-features --features
      pdfium,kokoro,time-stretch`.
- [ ] The C10 tests in §3.1 pass, including the ffprobe duration check on the desktop.
- [ ] CHANGELOG has the entries with a Migration note for the breaking ones.
- [ ] The C24, C28, C29 and C30 tests listed in their sections pass.
- [ ] `lectrice-ffi` exposes §3.22; the Kotlin `ErrorRegistryTest` passes with `FileUnreadable`.
- [x] Tagged `v0.2.0`; the Android repo pins it.
- [ ] `grep` finds no Kokoro/sherpa names outside `lectrice-infra/src/speech/`.

## 10. Design screens

E1 (speed, quality), E2–E5 (states), S4 and E1v (voice samples), R4 (buffered slider). New screens that rely on these
changes and need a design pass: E5b, E6, E7 (file 10), T3–T3d (file 20), L7, L7c, L7u (file 21).
