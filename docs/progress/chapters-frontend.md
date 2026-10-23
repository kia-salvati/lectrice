# Chapters: frontend (done 2026-10-23)

Branch `feature/chapters`, nothing committed. Option **1A**: the outline rail (take 1 of
`design/2026-10-23-chapter-options-v2.html`) plus the bottom sheet, label A in the pill, `C` as a simple toggle, and
"current chapter = nearest chapter-like title on the path, with the title's own number".

## What changed (all under `frontend/`)

Pure helpers (`utils/`, tests in `tests/utils.test.ts`, 21 tests, 8 new):
- `utils/toc.ts` (new): `buildTocTree` / `flattenTocTree` (nodes carry `index` and `parentIndex`), `currentEntryIndex`
  (binary search), `tocPath` (indexes, ancestors first), `entryEndSentenceId`, `tocLocation` (path + chapter + section
  indexes; the chapter is the first chapter-like title on the path, else the top entry), `chapterLabel` (`Chapter N`,
  `Ch. N`, `N:`, `N.`; `20 Popular…` and `10 years ago…` are not chapters), `entryProgress`, `percentThrough`,
  `entryLength`, `isPartNode` / `isChapterNode` (a part has a chapter-like child; a chapter is chapter-like or a
  top-level non-part), `nodeView` (everything a row shows), `childrenText` (`6 sections`), `filterTocTree`,
  `formatChapterPercent`.
- `utils/playbackOrder.ts`: `secondsBetween(order, from, to | null, speed, secondsPerChar)` (resolves non-playable ids).
- `utils/playerStatus.ts`: `playerStatusText` became `playerStatusParts` → `StatusPart[]` (`{ text, shrinks }`), so the
  pill can let only the chapter name truncate: `Ch. 7 · Long Documents · p. 213`, `Paused · …`, `… · 1.5×`; without
  a chapter it is today's `p. 37 of 412` as one span.
- `utils/sentences.ts`: `sentenceIdEnd(document)` (one past the last id, cues included: where the last part ends).

Hooks:
- `hooks/useChaptersPanel.ts` (new): the use case. Rail open/closed (remembered in `localStorage`
  `lectrice.chaptersRailOpen`), sheet open, wide (≥ 1200 px) / phone (≤ 720 px) media queries, the opener for focus
  return, `openRail` / `closeRail` / `openChapters` (popover: rail if shown, else sheet) / `toggleSheet` (pill) /
  `toggleWithKey` (`C`) / `returnFocus`. `hooks/useMediaQuery.ts` (new, `useSyncExternalStore`).
- `useReaderShortcuts`: `C` → `onToggleChapters`; every key whose target is inside `[data-own-keys]` is ignored.
- `useFollowScroll`: wheel / touchmove / scroll keys inside `[data-own-scroll]` no longer stop following. Also: a jump
  further than 2 window heights scrolls instantly instead of smoothly (see deviations).
- `usePlayback`: exposes `order` (read-only) so the screen can compute "min left in this chapter"; no logic change.

Components:
- `components/ChaptersPanel/`: `TocRow` (shared row: title button with `data-toc-index`, 2-line clamp, size text,
  bar, meta with `%` / check / `p. 412`, visually hidden ", page 412, current"), `ChaptersRail` (+ `ChaptersRailHandle`:
  header with book title, jump-to-current, ×; filter box; Collapse all ⇄ Expand all; the tree with parts as group
  headings, length-proportional bars with read-fill on chapter rows, disclosure buttons with `aria-expanded` /
  `aria-controls`; keeps the current row in view while playing unless focus is inside), `ChaptersSheet` (a `<dialog>`:
  `show()` on desktop, `showModal()` on phones; one level at a time with `›` drill and `‹` back; opens on the current
  chapter's sections), `ChaptersEmpty` (the empty-state text), `scrollInto.ts` (DOM helpers: centre / keep in view /
  find rows). Styles: `ChaptersPanel.module.css` (rows), `ChaptersRail.module.css`, `ChaptersSheet.module.css`.
- `components/ChapterLine/`: the popover's chapter line (`7. Long Documents › 7.3 …`, `38% of this chapter · 9 min
  left in it`; clicking it opens the chapters).
- `MiniPlayer`: `statusParts` instead of `statusText`; the popover's open state is now controlled by the page
  (`isPopoverOpen` / `onPopoverOpenChange`, so "Chapters…" can close it); a chapters button (list icon, before "more",
  `aria-expanded` / `aria-controls`, hidden while the rail shows). `PlayerPopover`: `chapterSection` slot above the
  location line and a "Chapters…" button beside it.
- `ReaderScreen`: memoises the tree, `sentenceIdEnd` and `tocLocation`; wires the three triggers; the text column gets
  `padding-left: var(--rail-width)` (new token, 17rem) while the rail shows. `icons.tsx`: list, chevron, check, close.
- `config/constants.ts`: `WIDE_SCREEN_QUERY`, `NARROW_SCREEN_QUERY`, `RAIL_OPEN_STORAGE_KEY`.

## Checks

`npm run build`, `npm run lint` (0 warnings), `npm test` (21/21): pass.

Mock mode in headless Chrome 154 (CDP scripts in the session scratchpad, nothing added to the project), 1280 × 800
and 375 × 740, light and dark — every item below passed:
- Long Book via "Continue": pill `Paused · Ch. 7 · Long Documents · p. 265`; popover `7. Long Documents › 7.3 Tables of
  contents that tell the truth`, `29% of this chapter · 31 min left in it`; the rail opens with 7.3 current, visible,
  `aria-current="location"`.
- Label flips at the first sentence of the next chapter (`Ch. 9` → `Ch. 10 · Settings…`); before the first entry only
  the page shows (`p. 2 of 429`, `p. Cover of 150` on the real books).
- Rail: × closes (handle + pill button appear, `localStorage` = `0`, still closed after reload), handle reopens with
  focus on the current row; filter `contrast` narrows to chapter 8 and auto-expands, `zzzz` → "No titles match.",
  Escape clears then leaves; Collapse all → 16 rows, Expand all → 304; jump-to-current re-expands the path, centres and
  focuses 7.3 after scrolling away; ↑ ↓ Home End, → expands / moves in, ← collapses / moves to the parent.
- Jumps: a title click while paused stays paused, the heading lands centred, follow-scroll is on (no locate button),
  the rail stays open; while playing it keeps playing; rows re-colour correctly after jumping back (ch. 7 unread,
  ch. 4 read) and forward.
- Traps: → ← inside the rail / sheet never step sentences; Space activates the focused row (a jump), never play/pause;
  wheel over the rail keeps following, wheel over the text stops it; a click on a panel's own background keeps focus
  inside it (`tabIndex={-1}` on both roots) so keys stay trapped.
- Sheet (rail closed, or < 1200 px): opens on the current chapter's sections with the current row focused, `‹ All
  chapters`, `›` / → drill, ← back, Enter jumps and closes, Escape / click outside / the pill button close it, focus
  returns to the pill button; at 375 px it is modal (`:modal`, backdrop, full width, bottom 0, tapping the dimmed text
  does nothing), drill size text hidden ≤ 420 px, status hidden ≤ 420 px, no horizontal scroll, popover fits.
- `C`: 1280 px opens the rail with focus inside / closes it (from inside or from the text); 1000 px and 375 px open and
  close the sheet; in the empty book it opens the sheet.
- Empty book (Meeting Notes): no rail, no handle, page-only label, sheet and popover show the empty state / no chapter
  line, "Chapters…" still there. Calm Software: `Ch. 1 · Quiet Interfaces`.
- Long Book open time ≈ 0.7 s in mock mode (rail open), unchanged.
- Contrast (measured, lowest text pair): `--fg2` on `--bg2` 5.47:1 light (7.33 dark) — sheet meta, pill status, handle;
  `--fg2` on `--bg` 6.09 (8.18); `--accent` on `--bg2` 5.72 (8.17) for `‹ All chapters`. Non-text: the bar track
  `--line` on `--bg` 1.50.
- Not run: a screen reader session (Orca). The markup follows the brief (`nav` › `ul` › `li` › `button` with page and
  state in the name, `aria-current`, `aria-expanded` + `aria-controls`, labelled dialog).

Real backend (release build, scratch data dir, port 7879, the two PDFs imported through the UI; nothing stored in the
user's library; servers stopped afterwards):
- Designing Distributed Systems: 83 entries; `Preface` and `Chapter 1. Introduction` as chapter rows, `Part I–III` as
  group headings with chapters 2–… under them; a click on `Chapter 3. Ambassadors` → `Paused · Ch. 3 · Ambassadors ·
  p. 21`, heading centred, following; in `Who Should Read This Book` → `Paused · Preface · p. vii`, popover `Preface ›
  Who Should Read This Book`.
- Clean Architecture: 222 entries, `PART I–VI` as group headings, front matter (Dedication … About the Author) as
  chapter rows; `Acknowledgments` → `Paused · Acknowledgments · p. 22`; in `The Goal?` → `Paused · Ch. 1 · What Is
  Design and Architecture? · p. 28`, popover `Chapter 1 What Is Design and Architecture? › The Goal?` and `20% of this
  chapter · 8 min left in it`.

Screenshots (mock content only) in `design/screens/`: new `reader-rail-light.png`, `reader-rail-dark.png`,
`reader-sheet-375.png`; refreshed `reader-light.png` / `reader-dark.png` (rail closed: label A + handle),
`reader-phone-375.png`, `library-light.png`.

## Deviations and notes

- `usePlayback` gained one read-only field (`order`); the brief said "no change", but "min left in this chapter" needs
  the order and rebuilding it in the screen would duplicate state.
- `useFollowScroll` now scrolls instantly for jumps longer than two window heights. A chapter click from mid-book used
  to smooth-scroll ~35 000 px, and with `content-visibility: auto` the target drifted: the heading settled 1 166 px below
  the viewport. Short moves (the sentence creep) stay smooth.
- The popover's open state moved from `MiniPlayer` to `ReaderScreen` (controlled), because "Chapters…" must close it.
- `tocLocation` replaces the brief's `currentChapter` (same rule, returns indexes so the panels and the label share one
  computation); `tocPath` returns indexes.
- The rail hides the pill's chapters button while it is shown (as in the design); the button, the popover's
  "Chapters…" and `C` always exist, including for a book without chapters.
- Focus return: the pill button and the handle get focus back; after "Chapters…" from the popover the opener is the
  popover's own button, which is gone when the panel closes, so focus falls back to the "more" button (if it was the
  last focused element) or the body. The design behaves the same.
- Space on a focused row activates it (a jump), as any button; it never reaches play/pause.
- `Paused` keeps today's rule of not showing the speed.
