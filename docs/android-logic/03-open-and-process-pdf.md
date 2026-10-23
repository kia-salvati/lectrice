# 03 · Open and process a PDF

Status: **spike** (whether long imports get page progress depends on the M1 import time; the rest is decided).

## 1. Goal

Get a PDF into the library **once**: copy it from wherever the user picked it, let the shared Rust cleaner turn it into
the reading view (same result as the desktop for the same file and cleaner version), save it keyed by the SHA-256 of its
bytes, then open it. A file that was imported before opens at once at its saved place.

## 2. User flow

1. Entry points:
   - Library's **Open PDF** button and S4's "Open a PDF" → the system picker (`OpenDocument`, `application/pdf`). The
     player sheet has no "Open another PDF…" row any more (Quiet's sheet has "Library" instead; spec §11.3 listed it).
   - **"Open with Lectrice"** (`ACTION_VIEW`) or **"Share to Lectrice"** (`ACTION_SEND`) from another app with a PDF.
2. The Library shows (navigating there if needed) with an **importing row** first under Recent: placeholder tile, the file
   name, an indeterminate 4 dp bar and "Reading… cleaning text" (L3).
3. Success, new book → the reader opens at the start, paused.
4. Success, **already in the library** → the reader opens at the saved place, with the L6 snackbar "Already in your
   library. Continuing where you left off."
5. Failure → the L4 error sheet (title "Can't read this PDF", the per-error text, "OK"); the importing row disappears.
6. If the user opened another book or left the Library while the import ran, the reader does **not** jump open; instead a
   snackbar **new** "Ready: {title}" + "Open".

## 3. Rules & logic

### 3.1 Copy, then import

1. Read `OpenableColumns.DISPLAY_NAME` and `SIZE` from the content resolver. Name missing → "document.pdf".
2. **Size limit 200 MB** (`import_max_mb`): if `SIZE` is known and larger → `UploadTooLarge` at once, nothing copied.
3. **Space check:** free (allocatable) ≥ `2 × size + 50 MB` (the temp copy + the library's saved copy + its JSON). Else
   `StorageFull`, with "Free up about {2 × size + 50} MB" in the text.
4. Stream the content into `cacheDir/import/<uuid>.pdf`, counting bytes; stop with `UploadTooLarge` past 200 MB (size may
   have been unknown); a write failure for lack of space → `StorageFull`; the resolver failing to open/read →
   `FileUnreadable` (core C26).
5. Call `import_pdf(path, displayName)` (Rust: magic bytes, SHA-256, lookup, else read → clean → split → save; touches
   `lastOpenedAt`).
6. **Always delete the temp file** (`finally`).
7. No persistable URI permission is kept: the core saved its own copy (`library/<docId>/original.pdf`).

### 3.2 One at a time

- Imports run **one at a time** in a FIFO queue (one `import` coroutine on `Dispatchers.IO`; cleaning a 400-page book
  takes seconds and uses one CPU core heavily). Each queued file has its own row; queued ones read **new** "Waiting…".
- An import may run while speech is being made (the speech worker is its own thread). They are not coordinated; playback
  may slow a little during a big import (accepted; the spike measures import time with nothing else running).

### 3.3 Progress

- v1: **indeterminate** bar with "Reading… cleaning text".
- **Conditional (spike M-h):** if the largest book's import on the S10+ takes **> 15 s**, core change C20 adds page progress:
  determinate bar + "Page 120 of 214" (L3b), then "Cleaning text" for the final step. Updates at most every 250 ms.

### 3.4 Opening a book (from the library or after import)

1. `get_document(id)` off the main thread (it also touches `lastOpenedAt`).
2. **Re-clean:** if the summary's `cleaner_version` (C27) is older than `current_cleaner_version()`, the core re-cleans
   from the saved PDF during this call. The reader then shows **new** "Updating this book's text…" (centred, indeterminate
   bar) instead of the text until it returns.
3. Map the records to UI models **once per open**, off the main thread (spec §12), then show the reader at the saved
   sentence (file 13).
4. Target: < 1 s for a 400-page book after its first import (spec §13; the spike measures `get_document`, and if it's >
   1.5 s, paging `get_document_blocks` is added, spec §5.3).

### 3.5 Intents

- Manifest: `ACTION_VIEW` and `ACTION_SEND` with `mimeType="application/pdf"` on `MainActivity` (`launchMode singleTask`,
  so a second intent goes to `onNewIntent`).
- An intent's URI is imported exactly like a picked one. `ACTION_SEND` with several files (`SEND_MULTIPLE`) is not handled
  in v1.
- If the first run hasn't finished (no model, S1 showing), the import still happens and the user lands in the reader; the
  pill shows "Voice not installed".

### 3.6 Error texts (L4 sheet; first brief §10.1)

| Name | Text |
|---|---|
| `PdfNoText` | "“scan-2019.pdf” is a scanned image with no text inside, so there's nothing to read aloud. Lectrice can't read scans yet." |
| `PdfEncrypted` | "“report.pdf” is password-protected. Save an unprotected copy in another app and try again." |
| `UploadTooLarge` | "“atlas.pdf” is 312 MB. Lectrice opens PDFs up to 200 MB." (size unknown: **new** "“atlas.pdf” is larger than 200 MB, the most Lectrice opens.") |
| `PdfInvalid` | "“file.pdf” looks damaged or isn't a PDF." |
| `StorageFull` | "Your phone is out of space. Free up about 400 MB and try again." (the real number) |
| `FileUnreadable` | **new** "“file.pdf” couldn't be opened. If it's in a cloud folder, download it first and try again." |
| anything else | "Something went wrong." (`Internal`) |

The file name shown is the display name, in curly quotes; it's never logged.

## 4. States

| State | What shows |
|---|---|
| Loading | L3 row (indeterminate) or L3b (conditional page progress); "Updating this book's text…" for a re-clean |
| Queued | row with "Waiting…" |
| Error | L4 sheet |
| Offline | no difference |
| Slow phone | the same row; no timeout (cleaning always finishes or fails) |

## 5. Edge cases

- **Same file under another name:** same SHA-256 → already in the library (L6).
- **A book removed a moment ago** (inside the undo window, file 21 §3.5): the core restores it instead of cleaning it again
  (`restore_document`, C28) and the undo snackbar goes; the reader opens at its saved place with L6. After the purge it is
  an ordinary first import.
- **Same book, a different edition/file:** different hash → a second library entry (expected).
- **User leaves the app mid-import:** the coroutine keeps running while the process lives (seconds). If the process dies,
  the import is lost (the temp file is in `cacheDir`, cleaned at next start); the user picks again. (No WorkManager: an
  import is short and user-initiated.)
- **A 0-byte or non-PDF file:** `PdfInvalid` (the core checks `%PDF-`).
- **PDF with a text layer but no sentences after cleaning:** `PdfNoText`.
- **Picking while another import runs:** queued (§3.2).
- **Re-clean changes sentence ids:** the saved position is kept if still in range (core `merged_with_saved`); it may land a
  few sentences off. Accepted (rare; only when the cleaner version rises).
- **A cloud file (Drive) not yet downloaded:** the resolver downloads it while we read (may be slow) or fails →
  `FileUnreadable`.

## 6. Where it lives

| Piece | Module |
|---|---|
| Picker launcher, intent handling | `app` (`MainActivity`) → `LibraryViewModel.import(uri)` |
| `LibraryRepository.import(uri)` (copy, checks, `import_pdf`, temp cleanup) | `core:data` |
| Import queue + rows | `LibraryViewModel` |
| Rust | `import_pdf`, `get_document` (existing); **new** C26 `FileUnreadable`, C27 `cleaner_version`; **conditional** C20 progress |

## 7. Data and persistence

- Core: `library/<docId>/original.pdf`, `document.json`, `meta.json` (web spec §6), under `filesDir/lectrice/`.
- Temp: `cacheDir/import/<uuid>.pdf`, deleted after each import and swept at app start.
- Auto Backup: the library is **excluded** (it can be hundreds of MB; Android's backup quota is 25 MB). Only settings are
  backed up (file 17).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Clean once, keyed by SHA-256; same cleaner as the desktop | user | 2026-10-06 |
| 200 MB limit | web spec / spec §9 | 2026-10-21 |
| Imports one at a time, FIFO | Opus | 2026-10-24 |
| Don't auto-open the reader if the user moved on; snackbar "Ready: …" instead | Opus (new copy) | 2026-10-24 |
| No WorkManager for imports | Opus | 2026-10-24 |
| `FileUnreadable` as a new registry name | Opus (core C26) | 2026-10-24 |

Open (spike): page progress for long imports (C20).

## 9. Done when

- [ ] Picking an invented test PDF imports it and opens the reader; the temp file is gone afterwards.
- [ ] Picking it again (renamed copy) opens at the saved place with L6.
- [ ] "Open with Lectrice" from a file manager imports and opens.
- [ ] Each L4 text appears for: an invented scan (no text), an encrypted invented PDF, a 201 MB file, a text file renamed
      `.pdf`, a full disk (test hook), a revoked URI.
- [ ] Two picks in a row: the second waits with "Waiting…" and then imports.
- [ ] Bumping the cleaner version (debug hook) shows "Updating this book's text…" once, then the text.
- [ ] Release logs show the document id, page/block/sentence counts and duration, never the file name.

## 10. Design screens

L3, L3b, L4, L6.
