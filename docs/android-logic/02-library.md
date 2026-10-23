# 02 · Library

Status: **decided**.

## 1. Goal

Answer "what was I doing?" at a glance and start listening in one tap: a **Continue card** for the current book, a
**Recent** list with progress, and a round **"Open PDF"** button in the thumb corner. It is the app's home screen.

## 2. User flow

1. The Library opens after the first run, on every cold start (unless the system restores another screen, file 12), and
   when the user goes back from the reader.
2. **Continue card** (when there is a current book, §3.1): tap the round play button → the reader opens **and plays**
   from the saved place. Tap anywhere else on the card → the reader opens **paused** at the saved place.
3. **Recent rows:** tap → the reader opens paused at that book's saved place (or at the start).
4. **Open PDF** (the floating button) → the system picker → the import row appears at the top of Recent → when done the
   reader opens (file 03).
5. **Settings** icon (top right) → Settings (file 11).
6. Banners between the header and the card: model not installed (L5, "Download" → S2), download running (file 01 §3.6),
   and any library-level error.
7. System back on the Library leaves the app (no confirmation).

## 3. Rules & logic

### 3.1 Which book is on the Continue card

- The **most recently opened** document (first of `list_recent`) **that has a saved position** (`lastSentenceId != null`).
- If that document is currently **playing**, the card's button shows **pause** and toggles playback without opening the
  reader; the card body still opens the reader.
- If its position is the last playable sentence (finished, file 13), the caption reads **new** "Finished · listen again"
  and play starts from the beginning.
- No document with a position → no card; Recent starts right under the header.
- The card's book is **not repeated** in Recent.

### 3.2 Card content

| Line | Content | Rule |
|---|---|---|
| Caption | "Continue listening" | `caption` 500 `fg2` |
| Title | the document title | `title-m`, 2 lines max, ellipsis |
| Location | "Ch. 7 · Long Documents · p. 265" | short chapter label + name + printed page label of the saved sentence (file 07 §3.3); no TOC → "p. 265"; ellipsis only on the name |
| Progress | 4 dp bar + "49 % · 4 h 12 min left" | percent read and time left at the book's saved speed (README §4.7) |
| Button | 56 dp round `accent` play | TalkBack: "Play The Long Book from chapter 7" (no chapter: "Play The Long Book from page 265") |

The location and time left need the document's TOC and text. The summary list doesn't carry them, so the ViewModel loads
the **card's document only** with `get_document` in the background (it's cached by the core's library cache) and shows the
card with title + percent first, then fills location and time left. (Cost: one `get_document`, measured by the spike; if
it's slower than 0.5 s the card simply fills in late.)

### 3.3 Recent rows

- Source: `list_recent(limit = 20, offset)`, newest `lastOpenedAt` first, **paged 20 at a time** as the user scrolls (the
  next page loads when 5 rows from the end).
- Row (min height 72, divider inset 72): **cover tile** 40 × 56 (§3.4) · title 16/20 500 (1 line) · meta 14/18 `fg2`
  "{pageCount} pages · opened {relative time}" · right column: "Continue · 34 %" (`accent`) above a 56 × 4 bar, or
  **"Start"** when never started.
- Relative time: port of `formatRelativeTime` ("just now", "2 hours ago", "yesterday", "last week"); `now` is passed in.
- TalkBack row label: "{title}, {percent} percent read, opened {relative}" or "{title}, not started, opened {relative}".
- Long-press on a row (or on the Continue card): the **book sheet** L7 with "Clear this book's audio" and "Remove from
  library" (file 21). TalkBack: custom action **new** "Remove from library" on every row and on the card.

### 3.4 Cover tiles (generated, no core change)

- A rectangle in one of **6 hues** picked by `hash(title) mod 6` (the first brief §9.1 hue pairs, light and dark), with the
  title's initials (first letters of the first two words that start with a letter; one word → its first two letters) in
  Noto Serif 600 at 40 % of the tile height. Radius 8 (row), 12 (bigger uses).
- The same tile, rendered once as a 512 × 512 bitmap into `cacheDir/covers/<docId>.png`, is the lock-screen artwork (file
  09).

### 3.5 The Open PDF button

- 56 dp round `accent` FAB with `note_add`, right 16, bottom 16 + gesture inset, 1 dp `bg` ring, TalkBack "Open PDF". The
  list's bottom padding is 120 dp so the last row clears it.
- Always enabled; imports run one at a time (file 03).

### 3.6 Banners (top of the list, margin 0 16 16)

| Condition | Banner | Action |
|---|---|---|
| No bundle installed and no download running | L5 warn: "Voice not installed. You can read now and listen after the download." | "Download" → S2 |
| Download running | **new** "Downloading the voice · 38 %" + 4 dp bar | "Details" → S3 |
| Engine failed to load | err: "The voice couldn't start. Try again, or download it again in Settings." | "Try again" → `reload_engine` |

At most one banner shows, in this order of importance: engine failed > download running > not installed.

## 4. States

| State | What shows |
|---|---|
| **Empty** (no documents) | L2: header, centred "Open a PDF to start listening" / "Lectrice cleans up the text and reads it aloud with a voice that runs on this phone. Nothing leaves your device." / "PDFs with a text layer, up to 200 MB", and the FAB |
| **Loading** | first page of `list_recent`: nothing for 150 ms, then 3 placeholder rows (`bg2` blocks, no shimmer under reduced motion) |
| **Importing** | L3 / L3b row first under Recent (file 03) |
| **Error** loading the list | inline err banner "Something went wrong." + "Try again" (`StorageFailed` / `Internal`) |
| **Offline** | no difference (the library is local) |
| **200 % font** (R12b) | the card stacks its play button under the text; rows grow; the FAB stays 56 dp |

## 5. Edge cases

- **A book's position is beyond its sentence count** (re-cleaned with fewer sentences): the core already drops it
  (`merged_with_saved`); the book shows "Start" and isn't on the card.
- **Two books opened within the same second:** order by `lastOpenedAt`, then by id (stable).
- **Very long titles:** card 2 lines, rows 1 line, ellipsis; TalkBack reads the whole title.
- **A book whose `get_document` fails** (file missing): the card hides its location line; tapping it shows the reader's
  error (file 12, `DocumentNotFound`).
- **Returning from the reader:** the list reloads page 1 (positions and order changed); scroll position resets to the top.
- **Process death:** the Library rebuilds from `list_recent`; nothing to restore.

## 6. Where it lives

| Piece | Module |
|---|---|
| `LibraryScreen`, `ContinueCard`, `RecentRow`, `CoverTile`, `StatusBanner` | `feature:library` (screen) and `core:designsystem` (components) |
| `LibraryViewModel` (state: card, rows, paging, banner, importing rows) | `feature:library` |
| `LibraryRepository` (`recent(limit, offset)`, `import(uri)`), `DocumentRepository.get(id)` | `core:model` interface, `core:data` implementation |
| Engine and install state | `ModelInstaller.state`, `PlaybackController.state` (to show pause on the card) |
| Core calls | `list_recent`, `get_document` (existing); the remove flow's calls are in file 21 (C28) |
| Book sheet, remove confirm, undo snackbar | file 21 |

## 7. Data and persistence

Everything comes from the core (`meta.json`: `addedAt`, `lastOpenedAt`, `lastSentenceId`). The per-book speed used for
"time left" comes from DataStore (file 16). Cover bitmaps are a cache (`cacheDir/covers/`), rebuilt when missing.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Option 1's library in Quiet's style with Quiet's round FAB, Continue card with the 3 dp accent bar | user (approved v2) | 2026-10-23 |
| Generated cover tiles (no page render) | approved design (first brief Q5 drawn as tiles) | 2026-10-23 |
| Card = most recent book with a position; not repeated in Recent | Opus | 2026-10-24 |
| Card play toggles playback when that book is already playing | Opus | 2026-10-24 |
| "Finished · listen again" caption | Opus (new copy) | 2026-10-24 |
| Long-press a row / the card → book sheet with "Remove from library" (file 21) | user (feature in v1) + Opus (placement) | 2026-10-24 |

No open questions.

## 9. Done when

- [ ] With books: the card shows the most recent started book; the play button starts it playing in the reader; the body
      opens it paused.
- [ ] Recent pages in 20 at a time with no jank on a 60-book library (generated test data).
- [ ] Empty library shows L2; the FAB opens the picker.
- [ ] L5 shows without a model; the download banner replaces it while downloading.
- [ ] Light, dark and black match L1 / L1-dark / L1-black; 360 dp matches L1-light-360; 200 % matches R12b.
- [ ] TalkBack reads every row and the card button with the labels above.
- [ ] Long-press a row → L7; Remove → the row goes with the undo snackbar (file 21 §9).
- [ ] No document title or file name in release logs.

## 10. Design screens

L1, L2, L3, L3b, L4, L5, L6, R12b. New, needs a design pass: L7, L7c, L7u (file 21).
