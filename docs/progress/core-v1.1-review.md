# Core v1.1.0 part 2: independent review

Reviewed on 2026-10-24: the uncommitted changes on `feature/core-v1.1` (backend, `NOTICE`, `CHANGELOG.md`, design spec,
frontend types), against `docs/android-logic/19-core-and-ffi-changes.md` and the feature files it points to. I read the
code, not the author's report. `design/` and `docs/android-logic/` were left out. Nothing was committed, pushed or stashed.

## Findings

Severity: **H** = wrong output or data loss users would hit; **M** = a race or edge case with a real effect; **L** = small
or defensive.

| # | Sev | Where | What | State |
|---|---|---|---|---|
| 1 | **H** | `lectrice-infra/src/audio/sonic.rs:46, :137` | **The Sonic port drifted.** `remaining_to_copy` (and the ≥ 2x count) were cut to whole samples on every pitch period, and the lost fraction was never carried over, so the output came out shorter than input ÷ speed. Measured at 1.75x: 1,635 samples (68 ms) short over 2 minutes, so about 0.34 s per 10-minute part. Chapter marks are timed as 1x ÷ speed, so they landed later and later through the book (several seconds by the end of a long book). | **Fixed.** The rounding error is now carried from one period to the next (what Media3's Sonic does), counts are rounded in `f64`, the cross-fade count is capped at the period, and the carry is reset at `flush`. New test `a_long_stream_keeps_its_exact_length`: 60 s at 75/125/150/175/200 must land within one pitch period of input ÷ speed. It failed before the fix and passes after. |
| 2 | **H** | `lectrice-app/src/exports/run.rs:222, :391` | **Chapter-mark error added up across parts.** Marks were `cumulative 1x samples ÷ speed` from the start of the book. Each part's real stretched length differs a little from 1x ÷ speed (the stretcher's tail at `flush`, any per-part error), and the parts are joined end to end, so the error grew with every part. | **Fixed.** `PartStart::mark_ms` = the earlier parts' **real** output samples + this part's 1x audio so far ÷ speed. The error is now bounded within one part, as the spec intends. A resume uses the stored parts' `samples`, so it times marks the same way. New test `chapter_marks_follow_the_real_length_of_the_earlier_parts`: a fake stretcher adds 100 ms at each part's end, and every mark must sit exactly on a part start. The old formula fails it (third mark about 44 ms instead of ≥ 400 ms). |
| 3 | **M** | `lectrice-app/src/exports/jobs.rs:634` | **A finished run could be lost.** In `complete`, a run that returned `Done` was turned into a pause when the user paused it during assembly or a shutdown came at that moment (both match arms came before `Done`). The file existed, but the job stayed paused and was assembled again later. | **Fixed.** `Done` now comes before the pause and shutdown arms (and clears `paused_by`). Only a cancel beats it. |
| 4 | **M** | `jobs.rs:613, :702` | **A cancelled run could leave an orphan file.** Cancelling during assembly (or a purge of the book) dropped the parts but left the finished `.m4b` on disk with no record. | **Fixed.** The file is deleted through `AudioExporter::delete_file`, unless another record owns the same path (with desktop 1.0 short names, an older done export of the same target shares it). |
| 5 | **M** | `jobs.rs:718` | **`drop_replaced` dropped saved exports.** When an export of the same target finished, every older done export of that target was deleted from the list, including one the user had moved to shared storage (`MarkExportSaved`, "Save to Audiobooks"). That file is a different file, and it vanished from the Storage list. | **Fixed.** Exports with a `Shared` location are kept. New test `an_export_moved_to_shared_storage_is_not_replaced`. |
| 6 | **M** | `jobs.rs:409` (`purge_document`) | **A running export cancelled by a purge wasn't saved.** On disk it stayed `running` until the run stopped. If the app was killed in that window, the next start loaded it as a resumable `System` pause of a book that was then deleted. | **Fixed.** The cancelled record is saved at once. The purge test now checks the stored status right after the purge. |
| 7 | M | `lectrice-infra/src/storage/audio_cache.rs:408` (`scan_clips`) | **A clip deleted during a rescan failed the whole call.** If the OS deleted a clip between `read_dir` and `metadata`, the error made `refresh()`, and so `GetStorageUsage`, `EstimateAudioExport` and `ClearAudioCache`, fail with `StorageFailed`. That's exactly the Android `cacheDir` case C29 is about. | **Fixed:** `NotFound` is skipped. |
| 8 | L | `lectrice-infra/src/export/ffmpeg.rs:165` (`parts_bytes`) | **Parts deleted while being counted gave an error.** An export finishing or being cancelled during `parts_bytes` made the storage screen fail. | **Fixed:** `NotFound` is skipped. |
| 9 | L | `ffmpeg.rs:187` (`delete_file`) | **`..` paths passed the safety check.** `starts_with(exports_dir)` compares components, so `<exports>/../x` was accepted. | **Fixed:** paths with a `..` component are refused. A test case was added to the existing `delete_file` test. |
| 10 | L | `lectrice-app/src/services.rs:69` | **Wrong doc comment.** It said "`None` *with* the speech engine". | **Fixed** ("without"). |
| 11 | L | `audio_cache.rs:323` (`refresh`) | **A clip can drop out of the index.** The rescan runs without the lock and then replaces the index, so a `put` that lands during the scan is left out until the next refresh or `put`. Effect: that clip isn't counted and may be made again. | Not fixed: harmless, and holding the lock over a full scan would block playback. |
| 12 | L | `lectrice-app/src/speech/read_ahead.rs:329` (`ClipStore::store`) | **A clip can be stored after a purge.** The "document still listed" check and the `put` aren't atomic, so a purge in between leaves one clip behind. | Not fixed: one clip at most. The cap and the window evict it later, and so does `ClearAudioCache`. The spec's "a put for a removed document is dropped" holds apart from this window. |
| 13 | L | `lectrice-infra/src/storage/document_repository.rs:236` (`delete`) | **A crash mid-delete leaves an orphan folder.** `meta.json` is deleted first (as the spec asks), so a crash between that and `remove_dir_all` leaves a folder no purge will see again (PDF copy and `document.json`). | Not fixed: a tiny window, and the order is the spec's. A later startup sweep of folders without `meta.json` could clean it up, but it must not race a first-time import. |
| 14 | L | `jobs.rs:397` | **Old failed and cancelled records of a purged book stay listed.** `purge_document` keeps every non-unfinished job unless `delete_exports_for` lists the book, including failed and cancelled ones with no file. They show in `list_all` until deleted. | Not changed: it follows spec §3.19 literally. The Android Storage screen may want to hide them, or the core could drop them; that's a product decision. |
| 15 | L | `ExportRecord.source_samples` | **The field is now mostly unused.** After fix 2, marks only need the 1x count *within* a part, so the stored total only feeds the progress log's `audio_seconds`. | Kept, so the public API stays the same for the Android agent. It can be removed in a later version. |

Checked and found correct (no change):

- Sonic against Media3's speed path: AMDF, down-sampling, refine window, `previousPeriodBetter`, overlap-add, insert/skip,
  the flush padding and the cut to the expected length. 1x bypasses it in both the runner (no stretcher) and the
  adapter. Very short input (shorter than one window) is padded and cut to `round(n ÷ speed)`.
- No clicks at part joins: each part's stretcher is flushed, and the tail is cross-faded into zero padding, then cut.
- StorageFull pauses the job at every stage: part append/finish (open part dropped), record save (`out_of_space` → stop →
  pause), assemble (stage `Assemble`; resume only assembles). Parts are never discarded on that path.
- Resume reads speed, engine and range from the record. Same target = document + voice + format + engine + speed + range.
- Engine id: fp32 `kokoro-v1.0` / int8 `kokoro-v1.0-int8`. It's part of the clip key and the sample key, and the runner
  only claims jobs of the loaded engine. `waiting_for_engine` is computed on read.
- Estimate: 18 chars/s, `output_ms = audio_ms_1x ÷ speed`, cached part through `contains` after a `refresh`, cumulative
  TOC and part offsets.
- Read-ahead `stop` only cancels queued jobs (`cancel_if_queued`); `plan_ahead = false` neither plans nor trims;
  runtime minutes go into the plan's anchor.
- Architecture: domain → app → infra/api dependencies point inward. No Kokoro, sherpa or ffmpeg names outside
  `lectrice-infra` (`speech/` and `export/`). One error registry (`FileUnreadable` 4006, names stored by name). DI through
  `Adapters`/`assemble`. Constants are named.
- Logging: only ids, counts, sizes, durations and paths. A smoke-test log had no text, title or label fields (the
  `file_name` import log is from v1, unchanged).
- Public-repo rule: no text from `~/Desktop/*.pdf` in the diff, the new files or the tests (test books use invented lines).

## Verification (2026-10-24, in `backend/` unless noted)

| Command | Result |
|---|---|
| `cargo build --release` | ok |
| `cargo test` (workspace) | ok: **428 passed**, 0 failed, 12 ignored (the 3 new tests included) |
| `cargo clippy --all-targets -- -D warnings` | ok |
| `cargo fmt --check` | ok (after one rustfmt line wrap in `jobs.rs`) |
| `cargo test -p lectrice-infra --test export_ffmpeg -- --include-ignored` (real ffmpeg/ffprobe) | ok: 9 passed (incl. 1.5x duration) |
| `cargo clippy -p lectrice-infra --no-default-features --features <f> --all-targets -- -D warnings` for `""`, `pdfium,kokoro,time-stretch`, `pdfium,kokoro`, `time-stretch`, `ffmpeg-export` | ok for all five |
| `cargo test -p lectrice-infra --no-default-features` | ok: 60 passed |
| `cargo test -p lectrice-infra --no-default-features --features pdfium,kokoro,time-stretch` | ok: 86 passed |
| `frontend/`: `npx tsc -b`, `npm run lint` (oxlint), `npm test` | ok, ok, 21 passed (`package.json` has no `typecheck` script; `tsc -b` is what `build` runs) |

Not run: the ignored book tests (`~/Desktop` PDFs) and the three real-Kokoro tests.

**Smoke test:** the release binary on **:7891** (not :7878), with a temporary config, a temporary `data_dir` and a
separate `cache_dir` in the scratchpad, `max_sentences = 30`.
- Health `ready`. Imported the smallest `~/Desktop` book: 201, 8,031 sentences, `cleanerVersion` 3.
- Sentence audio 200 `audio/wav`. Clips went under `cache_dir/audio`.
- `GET /api/voices/bf_emma/sample`: 200 in 1.0 s, then 200 in 0.9 ms (read from `cache_dir/samples`).
- M4B export: 202 with every new field. `done` 30/30, `fileBytes` set, location `appStorage`, file
  `exports/<doc>/af_heart.m4b` (desktop names kept). Download 200 `audio/mp4`; ffprobe: aac, 168.0 s.
- `DELETE /api/documents/{id}`: 204. Then GET gives 404 `DocumentNotFound`, the list is empty, the library and audio
  folders are empty, and the export file is kept.
- Log: one WARN, the expected 404, and no errors. Server stopped with SIGTERM; temporary data deleted.
- Speed exports aren't reachable over HTTP (no `speed` in the web API), so the real Sonic was checked through its unit
  tests and the ignored ffmpeg 1.5x test, not by ear.

## Public API changes

**None.** No public type, field, function or trait signature changed. Behaviour changes the Android side may notice:
- Exports at a speed other than 1x: output length and chapter marks are now accurate (Sonic carries its rounding;
  marks are anchored to the real output of earlier parts).
- A newer export of the same target no longer removes a done export saved to shared storage.
- A run that finishes while being paused or shut down is now `done` instead of `paused`.
- A cancelled or purged run that had already assembled its file now deletes that file.
- Purging a book with a running export saves the cancelled record at once.

## Files changed by this review

`backend/crates/lectrice-infra/src/audio/sonic.rs`, `lectrice-infra/src/storage/audio_cache.rs`,
`lectrice-infra/src/export/ffmpeg.rs`, `lectrice-infra/tests/export_ffmpeg.rs`, `lectrice-app/src/exports/run.rs`,
`lectrice-app/src/exports/jobs.rs`, `lectrice-app/src/exports/fakes.rs`, `lectrice-app/src/exports/tests.rs`,
`lectrice-app/src/services.rs`, and this note.
