# 12 · Errors, states and slow phones

Status: **spike** (the slow-phone bands and which phones get Prepare come from M1; the error handling is decided).

## 1. Goal

Every failure has one name (the shared registry), one place where it shows, plain words that say what to do, and a way
out. Every screen has a defined empty / loading / error / offline / slow-phone look. A phone that makes speech slower than
the user listens gets an honest, one-time explanation and a fix, never a broken experience.

## 2. User flow

1. Something fails → the user sees a banner, a sheet, a snackbar or a status line (never a stack trace or an error code).
2. The message says what happened and offers the next step ("Try again", "Download", "Continue", "Set 1.25×").
3. Recoverable problems fix themselves where possible (retry, skip one sentence, fall back to the default voice), and say
   so only when the user would notice.

## 3. Rules & logic

### 3.1 The error table (registry `lectrice-domain/src/error.rs` + core C26)

| Name | Code | Op. | Where it can happen | Shown as | Copy | Recovery |
|---|---|---|---|---|---|---|
| `UploadTooLarge` | 1002 | yes | import | L4 sheet | "“atlas.pdf” is 312 MB. Lectrice opens PDFs up to 200 MB." | OK |
| `PdfInvalid` | 1003 | yes | import | L4 | "“file.pdf” looks damaged or isn't a PDF." | OK |
| `PdfEncrypted` | 1004 | yes | import | L4 | "“report.pdf” is password-protected. Save an unprotected copy in another app and try again." | OK |
| `PdfNoText` | 1005 | yes | import | L4 | "“scan-2019.pdf” is a scanned image with no text inside, so there's nothing to read aloud. Lectrice can't read scans yet." | OK |
| `FileUnreadable` (new) | 4006 | yes | import, model zip import | L4 / import sheet | **new** "“file.pdf” couldn't be opened. If it's in a cloud folder, download it first and try again." | OK |
| `DocumentNotFound` | 1101 | yes | open a book, resume (incl. media resumption of a removed book, file 21) | snackbar on the Library | **new** "This book isn't in the library any more." | back to Library |
| `SentenceNotFound` | 1102 | yes | playback | — | — | rebuild the playlist from `playback_order`; position → the nearest valid sentence; log `warn` |
| `VoiceUnknown` | 2001 | yes | playback, export start | — (playback) / sheet (export) | export: registry text "Unknown voice." | playback: this book falls back to the default voice; log |
| `SpeechEngineUnavailable` | 2002 | yes | play, sample, export | R9 banner under the status bar + pill "Voice unavailable" | "The voice couldn't start. Try again, or download it again in Settings." | "Try again" → `reload_engine` |
| `SpeechSynthesisFailed` | 2003 | no | one sentence | snackbar | **new** "Couldn't read one sentence, so it was skipped." | retry once, then skip; 3 in a row → pause + R9 |
| `ExportNotFound` | 3001 | yes | export page | — | — | page resets to E1; log |
| `ExportNotReady` | 3002 | yes | save/share before done | — | — | buttons are disabled until done; log if it happens |
| `ExporterUnavailable` | 3004 | yes | export start (no AAC encoder: practically never) | sheet | **new** "Audio export isn't available on this phone." | OK |
| `ModelMissing` | 4001 | yes | play, Listen view, export | L5 banner, pill "Voice not installed" | "Voice not installed. You can read now and listen after the download." | "Download" → S2 |
| `ModelDownloadFailed` | 4002 | yes | download | S3c-style banner | **new** "The download failed. Try again." | "Try again" |
| `ModelChecksumMismatch` | 4003 | yes | download, zip import | S3c / import sheet | "Part of the download was damaged. Retrying…" then "Try again"; zip: **new** "This file isn't a Lectrice voice model." | retry once automatically |
| `StorageFull` | 4004 | yes | import, download, export (predicted before start; or mid-export, any stage), save to Audiobooks, cache, position save | L4 / S3d / E6 (before start) / E5, E5b (paused, parts kept) / snackbar (save) / silent (cache) | per place (files 01, 03, 10, 20) | "Free up space" → Settings → Storage (file 20); exports continue by themselves when there's room (file 10 §3.12), or Continue / Try again; a partial export (E7) |
| `ExportPaused` | 4005 | yes | (registered, not raised) | — | — | — |
| `StorageFailed` | 9001 | no | any write | snackbar | "Something went wrong saving data." | log with the cause |
| `Internal` | 9999 | no | anywhere | snackbar | "Something went wrong." | log with the cause |
| `RequestInvalid` 1000, `UploadMissingFile` 1001, `RouteNotFound` 1006, `MethodNotAllowed` 1007, `RequestCancelled` 1008, `ExportFormatUnsupported` 3003 | | | HTTP-only or programming errors | as `Internal` | | log at `error` (a bug) |

Rules:

- `core:data` maps `CoreException.Failed(name, …)` → `LectriceError(ErrorName)`; anything unknown → `Internal`.
- The **primary** name decides the message (`StorageFull` is primary over `StorageFailed`, core C22).
- User-facing text comes from `strings.xml` per name (+ per place); the core's `message` is never shown (it's English
  developer text).
- File names in messages are the display names (shown, never logged).
- Snackbars: one at a time, 4 s (8 s with an action), 12 dp above the pill / collapsed button / FAB; a new one replaces the
  old one; TalkBack announces them (polite).

### 3.2 Screen states (summary; each feature file has the detail)

| Screen | Empty | Loading | Error | Offline |
|---|---|---|---|---|
| First run | — | S3 progress, S4 sample spinner | S3c, S3d, EngineFailed | S3b; zip import works |
| Library | L2 | 3 placeholder rows after 150 ms | inline banner + Try again | no change |
| Import | — | L3 / L3b | L4 | no change |
| Reader | — | spinner after 150 ms; "Updating this book's text…" | snackbar + back, or "Try again" | no change |
| Pill / Listen view | — | Preparing | Voice unavailable / not installed | no change |
| Chapters | R8c | — | — | no change |
| Export | E1 | estimate placeholders ≤ 300 ms | E6 / E5 / E5b / failed / sheets | no change (only the inline model download needs a network) |
| Settings | — | storage "…" | "—" | no change |
| Storage screen (file 20) | "No books yet." | "…" per number | "—" per number | no change |

The app needs a network **only** for the model download (and the GitHub link). Everything else works offline.

### 3.3 Slow phones

**Bands (spike plan §6, sustained RTF on that phone):** green ≤ 0.6 · amber 0.6–0.9 · red > 0.9.

- **Catching up:** the player buffers mid-play (not within 2 s after play, a jump or a seek) → status "Catching up…" with a
  spinner (pill, Listen view). It clears when audio resumes.
- **The tip (R7):** shown when a Catching-up happens **and** `rtf_median × speed > 0.95` (the cause is the speed, not a one-off
  stall). At most **once per book** (DataStore set `slowTipShown`), dismissible (✕), never while audio keeps up. A warn card
  12 dp above the pill.
  - `best` = the highest of the six speeds with `rtf_median × speed ≤ 0.9`.
  - `best ≥ 1×`: "This phone makes speech a little slower than you listen at {speed}, so Lectrice pauses now and then to
    catch up. {smooth} plays smoothly." where `{smooth}` = "1×" or "1× or {best}" (e.g. "1× or 1.25×") · button "Set {best}".
  - `best = 0.75×`: **new** "This phone makes speech slower than real time, so Lectrice pauses now and then to catch up.
    0.75× plays smoothly." · "Set 0.75×".
  - no speed fits (RTF > 1.33): **new** "This phone makes speech slower than real time, so Lectrice pauses now and then to
    catch up." (no speed button).
  - **Conditional (file 14):** a second button "Prepare this chapter while charging" (R7p), only if Prepare is built.
- `rtf_median` comes from `speech_stats()` (core C17), read when the tip is about to show.
- Dismissing or using the tip marks it shown for that book.

### 3.4 Low memory

- `onTrimMemory(TRIM_MEMORY_BACKGROUND)` or higher **while not playing and no export running** → `stop_read_ahead`,
  `unload_engine()` (Kokoro was reported at ~830 MB on iOS; Android is measured by the spike).
- While playing, nothing is unloaded (the foreground service keeps the process important).
- The document UI model of a reader that isn't visible may be dropped and rebuilt (it's re-mapped from `get_document`).

### 3.5 Native crashes in the engine

A C++ abort inside sherpa-onnx or pdfium kills the process (uniffi can't catch it; spec §5.3). To avoid a crash loop:

- Before `reload_engine`, DataStore `engineLoading = true`; after it returns, `false`.
- At app start, if `engineLoading` is still true → `engineCrashCount += 1`, `engineLoading = false`. After a successful load
  and 1 minute of use → `engineCrashCount = 0`.
- If `engineCrashCount ≥ 2`, the engine is **not** loaded automatically; the pill shows "Voice unavailable" and R9 offers
  "Try again" (one manual attempt) and, in Settings, "Download again" for the bundle.

### 3.6 Process death and restore

| Where the user was | What comes back |
|---|---|
| Library | rebuilt from `list_recent` |
| Reader (`Reader(docId)` on the saved back stack) | the document reloads; the position from `meta.json`; paused |
| Listen view / a sheet / the export page | restored from `rememberSaveable` view state, paused |
| Settings, Model page | restored |
| First run, downloading | S3 rebuilt from the worker's state |
| Playing in the background | playback stops with the process; media resumption continues it (file 09 §3.9) |

## 4. States

This file *is* the states reference; see §3.2.

## 5. Edge cases

- **Two errors at once** (e.g. engine failed while a download runs): the Library banner order is engine failed > download
  running > not installed; the pill shows the most specific state.
- **StorageFull while saving a position:** silent; logged; retried at the next save.
- **Repeated `SpeechSynthesisFailed` on one specific sentence** (bad input): skipped every time; the sentence is still shown;
  logged with its id only.
- **The tip at 1× on a red phone:** the "no speed fits" text, plus Prepare if it exists.
- **Thermal throttling:** the RTF rises, the tip may show (it's measured, not guessed). The read-ahead governor (file 08
  §3.3) reduces read-ahead at SEVERE and stops it (and pauses exports) at CRITICAL; nothing is shown for that except the E3
  heat line on a paused export. The spike records heat (spec §15.4).
- **Android cleared our audio cache** (system "Clear cache" or low storage, file 20): not an error; clips are made again;
  at most "Preparing audio…" / "Catching up…".

## 6. Where it lives

| Piece | Module |
|---|---|
| `ErrorName` enum + `LectriceError` | `core:model` |
| Mapping `CoreException` → `LectriceError`; crash-loop flags | `core:data` |
| Strings | each feature's `res/values/strings.xml`, keyed by name |
| `StatusBanner`, `Snackbar host`, `SlowPhoneTip` | `core:designsystem` / `feature:reader` |
| Catching-up detection | `core:playback` (player state + timing) → `PlaybackState.status` |
| Core | registry (existing + C26), `speech_stats` (C17), C22 `StorageFull` mapping |

## 7. Data and persistence

DataStore: `slowTipShown` (set of document ids), `engineLoading`, `engineCrashCount`.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| One registry for both apps, codes 4xxx for device-neutral names | spec C4 (done in core v1.1) | 2026-10-23 |
| `FileUnreadable` 4006 added | Opus (C26) | 2026-10-24 |
| Tip trigger `RTF × speed > 0.95`, "best" speed `≤ 0.9`, once per book | Opus | 2026-10-24 |
| Skip a failing sentence after one retry; stop after 3 in a row | Opus | 2026-10-24 |
| Crash-loop guard (2 crashes → no auto-load) | Opus | 2026-10-24 |

Open (spike): the bands per phone and whether red phones get Prepare-first (file 14).

## 9. Done when

- [ ] `ErrorRegistryTest` passes (names and codes equal in Rust and Kotlin).
- [ ] Every row of §3.1 that can be triggered by a test hook shows the listed copy and recovery.
- [ ] On a throttled test (fake RTF 1.2 via a debug hook) at 1.5×, the tip says "1× or …" correctly and "Set" applies it; it
      never shows again for that book.
- [ ] Killing the process in each place of §3.6 restores as listed.
- [ ] A simulated engine abort twice in a row leaves the app usable with "Voice unavailable".

## 10. Design screens

L4, L5, R7, R7p, R9, S3c, S3d, E3, E5. New, need a design pass: E5b, E6 (file 10), the Storage screen's states (file
20).
