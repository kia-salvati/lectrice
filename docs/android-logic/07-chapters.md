# 07 · Chapters

Status: **decided**.

## 1. Goal

Always know where you are in the book (the current chapter next to the page number), and get anywhere in it fast: a
full-height, searchable **outline** of every chapter and section, plus "previous/next section" jumps. Built on the core's
`toc` (title, level 1–3, first sentence id, PDF page), the same data and the same rules as the web app.

## 2. User flow

1. While reading or listening, the pill shows "Ch. 7 · p. 265" (file 05); the player sheet shows
   "7. Long Documents › 7.3 Tables of contents that tell the truth" with "38 % of this chapter · 9 min left in it"; the
   Listen view shows the section title and "38 % of chapter 7".
2. **Open the outline:** from the reader, ⋯ → the **Chapters** button on the chapter line (2 taps), or drag the pill up and
   tap it; from the Listen view, the **Chapters** tool button (1 tap); with TalkBack, the pill's custom action "Show
   chapters" (1 step).
3. The outline opens full height, scrolled so the current entry is in the middle. Every entry of every level is in one
   list; chapter headers stay stuck at the top while you scroll through their sections.
4. Type in "Find a chapter or section" → the list filters, matches in bold, grouped under their chapters.
5. Tap an entry → playback jumps to its first sentence (play/pause kept), the sheet closes; the reader scrolls there with
   follow-scroll on (or the Listen view stays, showing it).
6. "≪ Section" / "Section ≫" (player sheet transport, Listen view) jump to the previous/next TOC entry.
7. A book with no TOC: the outline shows the empty text (R8c); section jumps are disabled.

## 3. Rules & logic

### 3.1 How chapters are reached (user: "do the best practice"; decided by Opus 2026-10-24)

**Rule: chapters live one step inside "the player", never on the pill, and are reached the same way everywhere:**

| From | Path | Taps |
|---|---|---|
| Reader | ⋯ → "Chapters" (on the chapter line, first row under the slider, visible at the half detent) | 2 |
| Reader (drag) | drag the pill up → "Chapters" | 1 drag + 1 tap |
| Listen view | "Chapters" tool button | 1 |
| Listen view → More | the player sheet's "Chapters" | 2 |
| TalkBack, reader | pill custom action "Show chapters" | 1 |
| Library | — (open the book first) | — |

Why this is the best practice here:

1. **It matches what people already know.** Reading apps (Kindle, Google Play Books, Moon+ Reader) keep the table of
   contents one step in: show the chrome, then "Contents". Audiobook and podcast apps (Libby, Pocket Casts, Smart AudioBook
   Player, Audible) put "Chapters" on the full player that the mini player opens. Our reader is the first kind and the
   Listen view the second, so "2 taps from the text, 1 tap from the Listen view" is the familiar pattern of each.
2. **The gesture map stays clean** (README §4.1): tap status = Listen view, tap ⋯ = player sheet, drag = player sheet,
   long-press = speed, swipe = sentence. Making ⋯ a chapters button would leave the player sheet (speed, voice, sleep, export,
   library) behind an invisible drag only; Android's accessibility guidance and WCAG 2.5.1 require a single-tap path for
   every gesture, so ⋯ must stay.
3. **There's no room for a sixth pill control:** at 360 dp the status would shrink to about 80 dp and couldn't fit
   "Ch. 7 · p. 265".
4. **Two taps stay cheap:** the chapter line is a large button with a visible `toc` icon and the word "Chapters", always
   visible at the sheet's half height, and the current place is readable on it before tapping.

### 3.2 Definitions (port of `frontend/src/utils/toc.ts`; chapters brief §3)

- **TOC:** the document's flat `toc` list in reading order; `sentenceId` strictly increases; `level` 1–3.
- **Current entry:** the last entry whose `sentenceId ≤` the current sentence id (any level); none before the first entry
  (binary search).
- **Path:** the current entry and its ancestors (walking back, each earlier entry with a smaller level than the last one
  taken).
- **Current chapter:** the first entry on the path whose title **looks like a chapter** (`Chapter 4 …`, `Ch. 4 …`,
  `Chapter IV …`, `4: …`, `4. …`; regexes `CHAPTER_WORD_RE` and `NUMBERED_TITLE_RE` copied exactly), else the path's first
  entry. **Current section:** the path entry right after the chapter, if any.
- **Entry end:** the first sentence of the next entry with the same or a smaller level; else the end of the book.
- **Chapter percent:** `round((current − start) ÷ (end − start) × 100)`, clamped 0–100, by sentence ids.
- **Time left in the chapter:** chars of the playable sentences after the current one and before the entry end ÷ 18 ÷
  speed (README §4.7).
- **Part node / chapter node:** a part groups chapters (has a chapter-like child); a chapter node is a chapter-like title, or
  a top-level entry that isn't a part. Used for the slider's ticks (file 05).

### 3.3 Labels

| Where | Format | Example | No number | No TOC / before the first entry |
|---|---|---|---|---|
| Pill status | `Ch. {n} · p. {label}` | Ch. 7 · p. 265 | `{title} · p. {label}` (title ellipsized) | `p. 12 of 18` / `p. iii` |
| Continue card | `Ch. {n} · {name} · p. {label}` | Ch. 7 · Long Documents · p. 265 | `{title} · p. {label}` | `p. {label}` |
| Player sheet chapter line | `{chapter title} › {section title}` (2 lines) + `{n} % of this chapter · {t} left in it · Page {label} · PDF page {i} of {count}` | 7. Long Documents › 7.3 Tables of contents… | same | `Page {label} · PDF page {i} of {count}` |
| Listen view top line 2 | section title, else chapter title | 7.3 Tables of contents that tell the truth | same | `p. {label}` |
| Listen view progress | `{n} % of chapter {k}` | 38 % of chapter 7 | `{n} % of {title}` | `{n} % of the book` |
| Media notification artist | `Ch. {n} · {book}` | Ch. 7 · The Long Book | `{title} · {book}` | `p. {label} · {book}` |

`{n}` is the title's own number (digits or roman, as printed), from `chapterLabel`. `{label}` is the printed page label of
the **current sentence's** page (`printedLabel`; falls back to the PDF page number).

### 3.4 The outline sheet (R8, R8s, R8b, R8c)

- Modal sheet, **full height** (top at status inset + 8), `bg2`, top radius 28, scrim. Header 56: "Chapters" `title-m` +
  close 48. Then the **search field** (height 48, radius full, `bg`, 1 dp `line`, `search` icon, placeholder "Find a
  chapter or section" `fg2`; a clear ✕ appears when it has text).
- **One flat list of every entry**, in order:
  - level 1: row min 48, 16/20 500 `fg`, page label right (`body-s` `fg2`);
  - level 2: indented 16, 15/20 400;
  - level 3: indented 32, 14/20 400 `fg2`.
  - Titles wrap to 2 lines, then ellipsis. A chapter-like title is shown as printed (no reformatting).
- **Sticky headers:** level-1 rows stick to the top (`bg2`, bottom 1 dp `line`) while their children scroll under them
  (`LazyColumn` `stickyHeader`).
- **Current marking:** every row on the **current path** gets a 3 dp `accent` bar on its left edge (full row height) and
  weight 600. No read/unread marks (Quiet keeps the list calm; progress is on the chapter line).
- **On open:** search empty; the list scrolled so the deepest path row is vertically centred (instant, no animation).
- **Tap a row** → `jumpTo(entry.sentenceId)` (if that sentence isn't playable, e.g. inside a code block with read code
  off, the core's start rule plays the next playable one); play/pause kept; the sheet closes; follow-scroll on.
- **Search** (R8s):
  - case- and accent-insensitive substring match on titles (`Normalizer.NFD`, marks removed, lower-case); starts at 1
    character; filters on every keystroke (in memory; ~400 entries at most).
  - Result: a level-1 row shows if it matches or any descendant does; under it, the matching level-2 rows plus level-2
    rows with a matching level-3 child, then those level-3 matches. Matched text in **bold** (first match per title).
  - No result: **new** "No chapter or section matches “{query}”." (`body` `fg2`, padding 24 16).
  - Tapping a result jumps as above. The keyboard closes when the list is scrolled.
- **Empty TOC (R8c):** header + "This PDF has no chapters or headings, so there's nothing to list. The page numbers in the
  text show where you are." (no search field).
- TalkBack: rows read "{title}, page {label}" + ", current" on path rows; level-1 rows are headings; the field is "Find a
  chapter or section, search".

### 3.5 Section jumps

Use the flat TOC (all levels). Let `cur` = current sentence id, `e` = current entry index (§3.2), `first(x)` = the first
playable sentence at or after entry `x`'s `sentenceId`.

- **Next section:** the first entry with `sentenceId > cur` → `jumpTo(first(it))`. None → disabled.
- **Previous section:**
  - no current entry (before the first entry) → disabled;
  - if `cur > first(e)` (we are inside `e`) → `jumpTo(first(e))` (back to the start of this section);
  - else if `e > 0` → `jumpTo(first(e − 1))`;
  - else → disabled.
- Play/pause kept; follow-scroll on; haptic `CONFIRM`.
- Empty TOC → both disabled.

### 3.6 Where the TOC appears elsewhere

- Slider ticks (file 05), the media artist line (file 09), the sleep timer's "End of chapter" (file 15), export chapter marks
  (file 10, core C23).

## 4. States

| State | What shows |
|---|---|
| Loading | the TOC comes with the document; the outline can't open before the document loads |
| Empty | R8c |
| No search result | the text in §3.4 |
| Error | n/a (pure in-memory data) |
| Offline / slow phone | no difference |
| 200 % font | rows grow; sticky headers wrap to 2 lines (known: a 2-line sticky header overlaps the row below while scrolled programmatically, noted in the mockup progress note; fix by measuring header height) |

## 5. Edge cases

- **Book with 390 flat entries (a posts collection):** all level 1, no children; every row is a sticky header → use plain
  rows when no entry has children (no sticky headers at all).
- **Parts over chapters** (`Part I` → `Chapter 1` …): the part is level 1 (sticky), chapters level 2; the current chapter is
  the chapter-like one, so the pill says "Ch. 1", not the part.
- **Front matter before the first entry:** no current entry; nothing is marked; the list opens at the top.
- **Two entries on the same sentence:** can't happen (the core skips entries that share a page with the one before and
  `sentenceId` strictly increases).
- **Jump target inside a skipped block:** handled by the core's start rule.
- **Very long titles** (cut at 120 chars by the core): 2 lines + ellipsis; TalkBack reads it all.
- **Re-cleaned book (new cleaner version):** the TOC can change; nothing is cached on the Kotlin side beyond the open
  document.

## 6. Where it lives

| Piece | Module |
|---|---|
| `toc.kt`: `chapterLabel`, `buildTocTree`, `currentEntryIndex`, `tocPath`, `entryEndSentenceId`, `tocLocation`, `percentThrough`, `isPartNode`, `isChapterNode`, `nextSectionTarget`, `previousSectionTarget`, `searchOutline` | `core:model` (pure Kotlin, ported from `frontend/src/utils/toc.ts`; the search and section helpers are new) |
| `ChaptersSheet` (outline + search), `OutlineRow`, `SearchField` | `feature:reader` / `core:designsystem` |
| `ChaptersViewModel` (query, filtered rows, current path) | `feature:reader` |
| Core | `get_document` (its `toc`), `playback_order` (for "first playable") — existing |

## 7. Data and persistence

None of its own. The query is view state (cleared on close). The TOC is part of `document.json` (cleaner version 3+;
older documents load with an empty TOC until re-cleaned).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Phone chapters = Quiet's full-height searchable flat outline with sticky chapter headers (not the spec's drill-in sheet) | user (approved v2 design) | 2026-10-23 |
| Chapters access: 2 taps from the reader, 1 from the Listen view, ⋯ stays the player sheet (§3.1) | **Opus, on the user's "do the best practice"** | 2026-10-24 |
| Current chapter = first chapter-like title on the path, with the book's own number; label "Ch. 7" in the pill | Opus default, built on the web (option 1A); the user hasn't objected | 2026-10-23 |
| No read/unread marks in the outline | the approved mockup (Quiet R8) | 2026-10-23 |
| Section jumps: "previous" goes to the start of the current section first | Opus | 2026-10-24 |
| No sticky headers for a flat outline | Opus | 2026-10-24 |
| "No chapter or section matches" text | Opus (new copy) | 2026-10-24 |

## 9. Done when

- [ ] JVM tests for `toc.kt` reproduce every case of the web's `toc` tests (same invented inputs, same outputs), plus the
      section-jump and search rules above.
- [ ] Each path in §3.1 opens the outline; the current row is centred.
- [ ] Search "summ" on the mock outline gives the R8s grouping with bold matches.
- [ ] Tapping an entry jumps, keeps play/pause and closes the sheet; from the Listen view it stays in the Listen view.
- [ ] A flat 390-entry outline scrolls smoothly (< 1 % janky frames) with no sticky headers.
- [ ] R8c shows for a book with an empty TOC; section buttons are disabled there.

## 10. Design screens

R8, R8s, R8b, R8c, R13k, R4 (chapter line), R1 (pill label).
