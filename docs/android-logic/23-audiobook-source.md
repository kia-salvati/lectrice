# 23 · Play from an audiobook file ("audiobook source")

Status: **decided by Opus 2026-11-13, for the user's review** (the user asked for the feature on 2026-11-13 and said to
build it as recommended without waiting; every open point in §8 has a recommended answer that is followed unless the user
overrules it). UI: option **B** of `design/2026-11-13-audiobook-source-options.html` (picked by Opus, §10; the user may
change it). Build notes: `lectrice-android/docs/progress/F23-audiobook-source.md`.

Change ids: **C35–C39** (file 24 uses C40–C52).

## 1. Goal

A book can be listened to **from an M4B file instead of generated speech**: an audiobook Lectrice made (on this phone or
on the desktop) or one imported from outside (file picker, "Open with", "Share"). Playing from the file costs no CPU for
speech, no heat and no battery beyond decoding AAC (≈ 1–2 % of one core), so a slow phone (S10+ class) plays a whole book
at 2× without "Catching up…", and a book exported on the desktop plays on the phone with the text following along.

**Everything else stays the same:** the reader highlights the sentence being heard, follow-scroll, tap-to-jump,
previous/next sentence, the slider, chapters, speed, the sleep timer, the Listen view, the lock screen and media buttons.

## 2. User flow

**A. An audiobook this phone made (the common case).** Export finishes (E4 "done"). E4 gets one more action:
**"Listen from this file"**. Tapping it links the file to the book and switches the book's audio source to it. The file
stays where it is (app storage, or `Audiobooks/Lectrice/` after "Save to Audiobooks", which keeps the link because the
app owns that MediaStore entry).

**B. An audiobook from the desktop (or any Lectrice-made M4B).** The user opens the `.m4b` with Lectrice ("Open with",
"Share", or **Import… → Audiobook file** in the library's ⋯/FAB menu). Lectrice reads the file's **Lectrice map** (§3.1):
- The book is in the library (same PDF SHA-256) → linked to it at once; snackbar "Linked to *The Long Book*. Listen from
  the audiobook?" [Listen] (switches the source and opens the book).
- The book is not in the library → sheet "This audiobook belongs to *The Long Book*. Open its PDF to read along."
  [Open PDF…] (system picker; after the import the link is made) · [Not now] (the file is remembered as pending for 30 days;
  importing that PDF later links it by itself).

**C. A foreign audiobook (no Lectrice map).** Sheet "Which book is this audiobook for?" listing library books, best
chapter-title match first with "Chapters match" under it (§3.6). Picking one links it in **chapter mode** (the text
follows chapters, not sentences; the sheet and Listen view say so). [Cancel] drops it.

**Switching back and forth.** In the player sheet, under Voice, a row **"Audio"** with two choices:
**Made on this phone** · **Audiobook file** (only shown when the book has a linked audiobook). The pill shows a small
`audiobook` mark while the file plays. Removing the link: book sheet (library long-press) → "Remove audiobook link"; the
file itself is never deleted by this.

## 3. Rules & logic

### 3.1 The Lectrice map inside the M4B (C35, C36, C37; shared core)

Every M4B the core exports carries a **Lectrice map**: a top-level ISO BMFF `uuid` box (`ISO/IEC 14496-12 §4.2`, vendor
extension; players skip unknown boxes) appended after the file is assembled, with the extended type
`6c656374-7269-6365-2d6d-61702d763031` (ASCII "lectrice-map-v01") and a UTF-8 JSON payload:

```json
{"format":"lectrice-audiobook","version":1,
 "documentId":"<SHA-256 of the PDF>","cleanerVersion":4,"engineId":"kokoro-v1.0-lectrice-fast-v1","voiceId":"af_heart",
 "speedPercent":150,"readSkipped":false,"range":{"fromSentence":0,"toSentence":708},"sampleRate":24000,
 "durationMs":2146900,
 "sentences":[[0,"9f3a1c22",0],[1,"0b7e4410",3120],...]}
```

- `sentences`: one entry per spoken sentence **in playback order**: `[sentence id, text hash, start ms on the file's
  timeline]`. A sentence ends where the next starts (the last at `durationMs`). Sentences with nothing to say (silent) are
  left out. Start times use the same arithmetic as the chapter marks (`PartStart::mark_ms`, 1× samples ÷ speed, timed
  per part from what earlier parts really hold), so they never drift.
- **Text hash** = FNV-1a 32-bit of the sentence's `spoken_text()` after Unicode lowercasing and collapsing whitespace,
  as 8 hex digits. It identifies a sentence across cleaner versions without putting book text in the file twice.
- Size: ~30 bytes per sentence (≈ 180 KB for an 8-hour book of 6 000 sentences, < 0.1 % of the file).
- **Both shells get it:** the core's `ExportRunner` collects the marks (C35, kept in the export record per part so a
  resumed export has them all) and passes a `LectriceMap` to `AudioExporter::assemble` (C36, port change). The M4B
  adapters call `lectrice_infra::export::mp4_map::append_map` after their own assembly (C37): the desktop's ffmpeg
  exporter and Android's `AndroidExporter`. MP3 exports (desktop) carry no map.
- Records saved before C35 have no marks for their finished parts: an export resumed from such a record gets **no map**
  (logged; the file still plays as a foreign audiobook in chapter mode).

### 3.2 Reading a file (C38, core)

`read_audiobook(path)` (FFI; Kotlin passes `/proc/self/fd/N` for a content URI) walks the top-level boxes without reading
the audio: the `uuid` map (if any), `moov/mvhd` duration, `moov/udta/chpl` chapter marks, `moov/udta/meta/ilst` title,
and checks there is one AAC (`mp4a`) audio track. Results: `AudiobookInfo { map: Option<LectriceMap>, duration_ms,
chapters, title }`, or `AudiobookUnreadable` (not an MP4, no audio track, damaged boxes, map JSON invalid or of a newer
major version; reading stops at 64 MB of boxes outside `mdat`).

### 3.3 Matching the file to a book (C38, core, pure and tested)

`match_audiobook(document, info) -> AudiobookMatch`:

| Case | Result | Timeline |
|---|---|---|
| map present, `documentId` = book, `cleanerVersion` = book's | **Exact** | the map's entries as they are (each hash is checked; a mismatch drops to Remapped) |
| map present, same `documentId`, other cleaner version | **Remapped** if ≥ 90 % of the map's hashes are found in order in the book (monotone greedy walk, window 200 sentences), else **Chapters** | matched sentences keep their times; unmatched ones are left out |
| map present, other `documentId` | `AudiobookOtherBook` (the UI offers to open the right PDF, §2 B) | none |
| no map | **Chapters** when ≥ 50 % of the file's chapter titles match the book's TOC titles (case-, accent- and number-insensitive), else **Chapters** with only the matches found; 0 matches → **Unaligned** | chapter starts ↔ TOC entries |

`AudiobookTimeline` (FFI record): `mode` (Sentences / Chapters / Unaligned), `file_speed_percent` (from the map, else
100), `entries: [{ sentence_id, start_ms, end_ms }]` (sentence mode) or `[{ toc_index, sentence_id, start_ms }]`
(chapter mode), `matched_percent`.

### 3.4 Playing (Android)

**One shared abstraction (file 24 §3.12):** a book has one **audio source** at a time:

| Source | What feeds the player | Producers |
|---|---|---|
| `Generated` ("Made on this phone") | sentence clips: cache → (helper, file 24) → local engine | engine, helper, Prepare |
| `AudiobookFile` | slices of the linked M4B | none: the file is the audio |

In code: `SentenceAudioSource` (core:model) with `GeneratedAudioSource` (today's `speak_sentence` path) and
`AudiobookAudioSource`. `Media3PlaybackController` asks the book's source for each item; nothing else in the player
changes.

**Sentence mode (Exact / Remapped).** The playlist window is the timeline's sentences in order (it replaces the core's
`playback_order` for this book; code/table cues are in the file or not, as exported). Each item's audio = the file
from `start_ms` to `end_ms`, decoded with `MediaExtractor` + `MediaCodec` (AAC → 16-bit PCM, trimmed to the exact
sample) and handed to ExoPlayer as a WAV clip, like a generated one. A reader keeps the extractor open and reads on from
where the last slice ended, so consecutive sentences cost one decode pass (≈ 100× real time on the S10+). No clip is
written to the audio cache.

- **Speed:** the file may have a speed baked in (`file_speed_percent`). The player rate = wanted speed ÷ file speed
  (e.g. 1.5× file, 1.5× wanted → 1.0; 1× wanted → 0.67). The speed control keeps showing the listening speed.
- **Highlight, follow-scroll, jump, prev/next, slider, sleep timer, chapters, lock screen title = sentence:** unchanged,
  because items are still sentences with the same ids.
- **Read code & tables aloud** has no effect in file mode (the file has what it has); the switch shows "Set by the
  audiobook" and is disabled.
- **Voice** row shows the file's voice (from the map) and is disabled; "Made on this phone" re-enables it.
- **Read-ahead, the governor, Prepare, the helper:** off for a book in file mode (nothing to make). The slow-phone tip
  never shows in file mode; on a slow phone the tip gains "Listen from the audiobook" when the book has one (file 24 §3.6).

**Chapter mode (no map).** ExoPlayer plays the file itself (progressive, one `MediaItem` per chapter with
`ClippingConfiguration`), so memory stays flat. The reader highlights the **chapter heading** of the current chapter and
follow-scroll goes to it; prev/next = previous/next chapter; the slider seeks in time; tap-to-jump on a sentence jumps to
its chapter's start. The Listen view shows the chapter title in the current slot. Lock-screen title = chapter title.
**Unaligned:** one item for the whole file; nothing in the text is highlighted; the sheet says "This audiobook can't
follow the text".

### 3.5 Positions

- Sentence mode saves the reading position as today (sentence id), so switching sources keeps the place exactly.
- Chapter mode saves the chapter's first sentence plus a file offset (`audiobookOffsetMs` per book in preferences), so
  re-opening continues mid-chapter in file mode and at the chapter start in generated mode.
- Switching source keeps playing state: pause, swap the playlist around the current sentence (or the current chapter's
  start), play if it was playing.

### 3.6 Linking and storage

- A link = `{ documentId, uri, kind (OwnExport | Imported), mode, fileSpeedPercent, durationMs, matchedPercent,
  linkedAt, fileSize, lastModified }`, kept in `filesDir/lectrice/audiobooks.json` (Kotlin; tiny). One link per book; a
  new link replaces the old one (the old file is not deleted).
- **No copy of an imported file.** Lectrice keeps a persisted read permission (`takePersistableUriPermission`) on the
  user's file. The app's own exports are read by path (app storage) or by the MediaStore URI it created.
- The timeline is computed when the link is made and again when the book's cleaner version changes or the file's size /
  last-modified changes; it is cached in `filesDir/lectrice/audiobook-maps/<documentId>.json` (sentence ids and times only).
- Storage screen (file 20): a book's row shows "+ audiobook" when linked; the file's size is not counted (it isn't ours)
  unless it's an app-storage export (already counted under Exports).
- Removing the book from the library removes the link (not the file). Deleting an export that is linked asks first:
  "*The Long Book* listens from this file. Delete it anyway?"; afterwards the book goes back to Made on this phone.

### 3.7 Errors

| When | Shown | Then |
|---|---|---|
| Not an M4B / damaged / no audio (`AudiobookUnreadable` 3103) | snackbar "This file isn't an audiobook Lectrice can play." | nothing linked |
| Lectrice map of another book (`AudiobookOtherBook` 3102) | §2 B sheet | open that PDF or not now |
| File gone or permission lost when playing (`FileUnreadable` 4006) | pill status "Audiobook file missing"; sheet row "Find it again…" (picker) · "Use Made on this phone" | playback pauses at the current sentence; switching to generated continues from there |
| File changed since linking (size or date) | re-read silently; if the match gets worse (Exact → Chapters), snackbar "The audiobook changed; the text follows chapters now." | |
| Remapped below 90 % | the link is made in chapter mode; sheet note "Made from an older version of this book's text, so the text follows chapters." | |
| A sentence slice can't be decoded | that sentence is skipped (like a failed synthesis, file 09 §3.5); 3 in a row → as "file missing" | |

New registry names (C39): `AudiobookNotLectrice` 3101 (only logged: foreign files are fine), `AudiobookOtherBook` 3102,
`AudiobookUnreadable` 3103. Kotlin `ErrorName` and `ErrorRegistryTest` follow.

### 3.8 Logging

Ids, counts, mode, matched percent, durations, file size; never titles of foreign files' chapters, never the map's
hashes, never sentence text. The lock-screen title rule (sentence text on the lock screen, file 09) is unchanged.

## 4. States (player sheet "Audio" row and pill)

| State | Pill | Sheet "Audio" row |
|---|---|---|
| no link | as today | row hidden |
| linked, generated active | as today | "Made on this phone" selected; "Audiobook file · 2 h 41 min · Heart" |
| file active, sentence mode | `audiobook` mark before the status | "Audiobook file" selected; detail "Text follows each sentence" |
| file active, chapter mode | mark | detail "Text follows chapters" |
| file active, unaligned | mark | detail "Can't follow the text" |
| file missing | "Audiobook file missing" | warning + "Find it again…" |

## 5. Edge cases

- **Partial export (range):** only that range is in the timeline; past its end the player stops with "End of the
  audiobook. Continue with Made on this phone?" [Continue] (switches source and plays on).
- **Two exports of the same book:** the newest linked one wins; E4's "Listen from this file" on another replaces it.
- **Book re-cleaned (cleaner version bump):** Exact becomes Remapped (or Chapters) at the next open; positions are
  sentence ids of the new version, as everywhere.
- **Exports made with the speed baked in at 2× and listened at 0.75×:** rate 0.375; Sonic handles it (sounds stretched;
  the sheet shows the file's speed in the detail line "made at 2×").
- **Desktop exports:** identical map, made by the same core; the desktop doesn't play from files in this version.

## 6. Where it lives

| Piece | Where |
|---|---|
| C35 sentence marks in the runner + record | `lectrice-app/src/exports/run.rs`, `lectrice-domain/src/export.rs` |
| C36 `LectriceMap` + text hash + JSON codec; `AudioExporter::assemble(…, map)` | `lectrice-domain/src/audiobook.rs`, `ports.rs` |
| C37 `mp4_map` (append the box; read map, mvhd, chpl, ilst title) | `lectrice-infra/src/export/mp4_map.rs` (pure Rust, no new deps) |
| C38 `match_audiobook` | `lectrice-domain/src/audiobook.rs` |
| C39 registry names | `lectrice-domain/src/error.rs` |
| FFI `read_audiobook(path, document_id)` → `AudiobookTimeline` | `lectrice-android/rust/lectrice-ffi` |
| `SentenceAudioSource`, `AudiobookLinks`, `AudiobookSlicer` (MediaExtractor/MediaCodec) | `core:model`, `core:data`, `core:playback` |
| E4 action, "Audio" row, pill mark, link sheets, import menu | `feature:reader`, `feature:library` (Compose: Fable, from the brief) |

## 7. Data and persistence

`audiobooks.json` (links), `audiobook-maps/<doc>.json` (timelines), `audiobookOffsetMs` + `audioSource` per book in the
book listening preferences. All excluded from backup like the rest of `filesDir` (file 17).

## 8. Decisions (recommended, followed unless the user overrules)

1. **Map format:** a top-level `uuid` box with JSON, appended (not an `ilst` freeform item): no rewrite of `moov`, any MP4
   tool skips it. ✔
2. **Imported files are not copied** (persisted URI permission). ✔
3. **Foreign files:** supported in chapter mode (attached to a book the user picks). ✔
4. **Switching is never automatic** (the user picks the source; only the slow-phone tip suggests it). ✔
5. **Desktop playing from files:** not in this version (the desktop makes speech fast enough).

## 9. Done when

- Desktop and phone exports carry the map; `read_audiobook` reads it back (core tests with invented documents).
- On the S10+: export a book, "Listen from this file", the highlight follows each sentence at 1× and 1.5× within one
  sentence for the whole book; prev/next, slider, chapters, sleep timer, lock screen work; CPU well under the generated
  path; a file deleted behind the app's back gives "Audiobook file missing" without a crash.
- A foreign M4B (made by ffmpeg from invented audio with chapters) links in chapter mode.

## 10. Design

Options in `design/2026-11-13-audiobook-source-options.html` (brief `docs/2026-11-13-audiobook-source-brief.md`).
**Picked: B** (see the brief's §5 for why); the user may switch to A or C later.
