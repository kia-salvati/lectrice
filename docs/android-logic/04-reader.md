# 04 · Reader

Status: **decided**.

## 1. Goal

Show the cleaned book as large, calm text filling the screen (design D, Quiet style), mark the sentence being read,
keep it in view while listening, and let the user jump anywhere with one tap. Page markers show where each part sits in
the original PDF but are never spoken.

## 2. User flow

1. The reader opens from the Library (file 02) or after an import (file 03), scrolled so the **saved sentence** sits at
   **35 %** of the screen height, highlighted. Paused unless opened with the Continue card's play button.
2. Play (pill, file 05) → the highlight moves sentence by sentence; the text follows (follow-scroll).
3. The user scrolls by hand → follow-scroll stops; a **"Back to current"** chip appears (↓ if the highlight is below the
   view, ↑ if above). Tap it → smooth scroll back, follow-scroll resumes, the chip hides.
4. **Tap a sentence** → the highlight and the playback position move there. Play/pause stays as it was. Follow-scroll
   resumes.
5. While playing, scrolling down (or 4 s untouched) shrinks the pill to a round button; scrolling up brings it back
   (file 05).
6. Back (gesture or ◁) → predictive preview (R11: the reader shrinks to 0.9 and slides right, the Library visible behind) →
   release → Library. Playback keeps going in the background.

## 3. Rules & logic

### 3.1 Layout

- Edge-to-edge; a **status scrim** (the screen's `bg` at 94 % alpha, status-bar height) over text that scrolls under the
  status bar. No top bar.
- Text column: left/right padding 16 dp (max width 640 dp, centred, on screens ≥ 600 dp); top padding = status inset + 24;
  bottom padding = gesture inset + pill (56) + 12 + 48 (so the last line can scroll above the pill).
- `LazyColumn`, **one item per block**, `key = block.id`, `contentType = block.kind`.

### 3.2 Block rendering (first brief §8.4, Quiet)

| Kind | Look | Spoken |
|---|---|---|
| `heading` level 1 | Noto Serif 26/34 600, margin 48 top / 16 bottom | yes |
| `heading` level 2 | 22/30 600, 32 / 12 | yes |
| `heading` level 3 (and deeper) | 19/28 700, 24 / 8 | yes |
| `paragraph`, `note` | Noto Serif 19/32 400 (text size L), paragraph spacing 16 | yes |
| `listItem` | a bullet paragraph (indent 16, "•") | yes |
| `code` | Roboto Mono 14/20 on `bg2`, radius 8, padding 12, **scrolls sideways inside the block** with the 24 dp right-edge fade while more is hidden (README §4.4); under it the cue line "Read as “Code example.”" (Roboto 13/18 `fg2`) when "Read code & tables aloud" is off | cue only (or its sentences when on) |
| `table` | its text as shown, monospace like code (keeps columns), cue line "Read as “Table.”" | cue only (or its sentences when on) |
| `caption` | Roboto 14/20 italic `fg2` | silent (or its sentences when on) |

- A paragraph is **one `Text` with an `AnnotatedString`**: each sentence is a range; the current sentence gets the `hl`
  background and `hlFg` text (radius 4 via a custom span drawer, padding 0 2), cross-fading 120 ms.
- **Only the block holding the current sentence recomposes** when the highlight moves (its index comes from a
  `derivedStateOf`; other blocks read stable data).
- If the reader benchmark shows jank (spec §12), that one paragraph composable switches to `Canvas` + `TextMeasurer` with
  layouts cached per block. Not before.

### 3.3 Page chips

- Placement: a chip **"p. {label}"** (`label` = the printed page label, `pages` lookup, else the PDF page number) is drawn
  **inline at the sentence where the page changes** (the first sentence whose `pageIndex` differs from the previous
  sentence's), and at the first block of the book. Port of `planPageMarkers` from `frontend/src/utils/pages.ts`.
- Drawn with `InlineTextContent` (Roboto 12/16 `fg2`, 1 dp `line` border, radius 6, height 20, padding 0 6, margin-right 6).
  Not selectable, **never spoken**, not part of any sentence range.
- TalkBack: "Page 265" (and "PDF page 279" when the label differs from the PDF page number).
- Inside code/table blocks no chips are drawn; a page change inside such a block shows its chip on the next spoken block.

### 3.4 Follow-scroll

- State `followScroll: Boolean` (`rememberSaveable`), true on open, after a tap-to-jump, a chapter/section jump, "Back to
  current", closing the Listen view, and play from a stopped state.
- While true: when the current sentence changes, scroll (animated, spring) so the **top of the sentence's line** sits at
  **35 % of the viewport height**, only if it is outside the band 20 %–60 % (so short steps inside the band don't move the
  page; fewer motions while reading along).
- The highlight must never hide behind the pill or the chip (the 60 % band edge is above them on every phone size; at
  200 % font the band is 15 %–50 %).
- A **user drag or fling** of the list sets it false. Programmatic scrolls don't.
- Chip direction: compare the current sentence's block index with the visible range; ↓ if after the last visible item,
  ↑ if before the first. When hand-scrolling brings the current sentence back inside the 20–60 % band, the chip hides and
  **follow-scroll turns back on**, so the user doesn't have to find the chip.
- Chip: right 16, 12 above the pill (or the collapsed button), height 40 (48 touch), `bg2`, 1 dp `line`, icon + "Back to
  current" `label`. TalkBack "Back to current sentence".

### 3.5 Tap to jump

- A tap (README §4.1) on text → `TextLayoutResult.getOffsetForPosition(tap)` → the sentence whose range contains that
  offset. A tap in the gap between sentences picks the nearest one before it.
- Tap on a **heading** → its sentence. Tap on a **code/table** block → the block's cue (read code off) or the first of its
  sentences (on). Tap on a **caption** with read code off → the next playable sentence after it (core's start rule).
- Effect: `playback.jumpTo(sentenceId)`; play/pause kept; follow-scroll on; the position is saved (file 13).
- A tap on a page chip does nothing (it isn't a sentence).
- No double-tap and no long-press actions on text in v1, so taps fire immediately (no 300 ms wait).
- TalkBack: each sentence isn't a separate node (too many). The paragraph is one node; its custom action "Play from here"
  jumps to the paragraph's first sentence; TalkBack's own reading of the text is unaffected.

### 3.6 Without a voice

- The reader works fully: text, chips, chapters, tap-to-jump (it moves the highlight and saves the position).
- The pill shows "Voice not installed" / "Tap to download" (`accent`); tapping the status opens S2 instead of the Listen
  view (file 05).

### 3.7 Volume keys

While the reader (or the Listen view) is on screen, the volume keys change the **media** volume
(`setVolumeControlStream(STREAM_MUSIC)`).

### 3.8 Text size and theme

From Settings (file 11): text size stops S / M / **L** / XL / XXL = 15 / 17 / **19** / 22 / 26 sp for paragraphs (line height
× 1.68, headings scale by the same factor), times the system font scale. Theme light / dark / black. A change re-lays the
text and keeps the current sentence at the same screen position.

## 4. States

| State | What shows |
|---|---|
| Loading (`get_document`) | `bg` with a centred 16 dp spinner after 150 ms; the pill shows disabled |
| Re-cleaning | "Updating this book's text…" + indeterminate bar (file 03 §3.4) |
| Error | `DocumentNotFound` → back to the Library with a snackbar **new** "This book isn't in the library any more."; other errors → centred err text + "Try again" |
| Empty document | can't happen (import refuses `PdfNoText`) |
| Offline | no difference |
| Slow phone | "Preparing audio…" / "Catching up…" in the pill (file 12); the reader itself is unaffected |
| 200 % font | R12: text 38/64 sp, pill status one line with ellipsis, pill 72 dp |

## 5. Edge cases

- **Highlight inside a code block** (read code on): the current code sentence is highlighted inside the block; the block
  scrolls sideways so the highlight's start is visible.
- **Very long paragraph** (several screens): follow-scroll targets the sentence's line, not the block top
  (`LazyListState.scrollToItem(index, offset)` with the offset from the text layout).
- **Rotation:** the current sentence keeps its 35 % position after the re-layout; follow-scroll state survives
  (`rememberSaveable`).
- **Tapping while the list is still flinging** stops the fling (normal Compose behaviour) and is **not** a jump.
- **Highlighted sentence with no visible text** (empty after cleaning): skipped by the core's playback order; never current.
- **Process death in the reader:** the navigator restores `Reader(docId)`; the document reloads; position from `meta.json`.
- **Another book is playing when this one opens:** playback of the other book is paused and its position saved; this
  book's pill starts paused (file 09 §3.8).

## 6. Where it lives

| Piece | Module |
|---|---|
| `ReaderScreen`, `ReaderText` (LazyColumn), `ParagraphText`, `CodeBlock`, `PageChip`, `BackToCurrentChip` | `feature:reader` / `core:designsystem` |
| `ReaderViewModel` (document UI model, current sentence from `PlaybackController.state`, follow-scroll is **view state**) | `feature:reader` |
| Pure helpers: `planPageMarkers`, `printedLabel`, sentence ranges, `blockIndexOf(sentenceId)` | `core:model` (ported from `frontend/src/utils/pages.ts`, `sentences.ts`) |
| Core | `get_document` (existing) |

## 7. Data and persistence

- View state (`rememberSaveable`): `followScroll`, scroll position, Listen view open, sheet open.
- The document UI model is held by the ViewModel for the reader's life (not persisted).
- Position: file 13.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Design D on the phone, Quiet style, no top bar | user | 2026-10-06 / 2026-10-23 |
| "Back to current" chip instead of the spec's "locate" pill button | approved design | 2026-10-23 |
| Highlight at 35 %, move only when outside 20–60 % | Opus (35 % from the design; the band is new) | 2026-10-24 |
| Scrolling the highlight back into view turns follow-scroll on again | Opus | 2026-10-24 |
| No text selection / copy in v1 (conflicts with tap-to-jump) | Opus | 2026-10-24 |
| Text size stops 15/17/19/22/26 sp | Opus (the mockup shows 5 stops, L default) | 2026-10-24 |

## 9. Done when

- [ ] Opening a book puts the saved sentence at 35 %, highlighted.
- [ ] While playing, the highlight never sits behind the pill or chip, at 412 and 360 dp, at 100 % and 200 % font.
- [ ] A hand scroll shows the chip with the right arrow; tapping it returns; scrolling back by hand also restores follow.
- [ ] Tap-to-jump works on paragraphs, headings, code (both switch states) and captions as in §3.5.
- [ ] Page chips appear exactly where `planPageMarkers` says (a JVM test on an invented document) and are never in the
      playback order.
- [ ] Code blocks scroll sideways with the fade only while cut; no scrollbar anywhere.
- [ ] Macrobenchmark on the generated 400-page book: < 1 % janky frames while scrolling (spec §13).
- [ ] TalkBack: page chips read "Page 265", paragraphs offer "Play from here".

## 10. Design screens

R1, R2, R3, R9, R11, R12, L6 (snackbar over the reader), `page-light`/`page-black` (no scrollbars).
