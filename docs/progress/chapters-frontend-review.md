# Chapters: frontend review (Fable, 2026-10-23)

End-of-task review of the `feature/chapters` frontend diff against `rules/CLAUDE_FRONTEND_RULES.md`, the brief
(`docs/2026-10-23-chapters-brief.md`), the build report (`docs/progress/chapters-frontend.md`) and the chosen design
(take 1 + label A in `design/2026-10-23-chapter-options-v2.html`). Only files under `frontend/` were edited; nothing
committed; API contract unchanged (`TocEntry` matches the backend's camelCase `id/title/level/sentenceId/pageIndex`).

## Issue → change, file by file

| File | Issue | Change |
|---|---|---|
| `src/hooks/useReaderShortcuts.ts` | **Bug (key trap leak).** "Inside the panel" was decided with `event.target.closest('[data-own-keys]')` on the `window` listener. When a key changes the sheet's level (→ drill, ← back), React's own handler runs first and unmounts the focused row, so by the time the event reaches `window` the target is detached and `closest` finds nothing: the sheet drilled *and* the reader stepped a sentence. Reproduced in headless Chrome (status `Ch. 2 · p. 33` → `Ch. 1 · p. 32` after ← ← in the sheet). | Membership is now read from `event.composedPath()` (fixed at dispatch time) via a new pure helper `startedInside(event, selector)` in `src/utils/events.ts`. |
| `src/hooks/useFollowScroll.ts` | Same `target.closest` pattern for `[data-own-scroll]` (wheel / touch / scroll keys). Not observed failing (those targets are not unmounted), but it is the same mechanism and §5 asks for one consistent one. | `isOwnScroll(event)` uses `startedInside`. |
| `src/utils/events.ts` (new) | — | The helper above, 1 line + doc comment explaining why not `closest`. |
| `src/utils/toc.ts` | §11: `NodeView.isChapter` was returned but no component read it (`fillPercent` already encodes "is a chapter row"); only tests asserted it. | Removed from the interface and the return; `isChapterNode` stays (the rail uses it for the widest-chapter bar). |
| `tests/utils.test.ts` | Followed the above. | Dropped `isChapter` from the five `nodeView` expectations. 21/21 still pass. |
| `src/pages/ReaderPage/ReaderScreen.tsx` | §2 "no logic in JSX": the popover's chapter line computed `percentThrough`, `secondsBetween`, `entryEndSentenceId` and two template strings inside a JSX prop; the guard also repeated `chapter === null` which `location === null` already implies. | A small `chapterLine(chapterIndex, sentenceId)` builder above the `return` names each piece (`pathText`, `percentText`, `secondsLeftInChapter`); the JSX gets finished strings. Guard reduced to the two values TypeScript actually needs narrowed. |
| `src/components/ChaptersPanel/ChaptersSheet.tsx` | The dialog was `aria-label="Chapters"` while the nav inside is also "Chapters"; the design labels the dialog by its heading (book title / drilled chapter), which is also what a screen reader should announce when the level changes. | `aria-labelledby={`${id}-title`}` on the `<dialog>`, the `id` on the `<h2>`. |

Everything else in the diff was checked and kept as is:
- **Layers (§1):** `utils/toc.ts`, `sentences.ts`, `playbackOrder.secondsBetween`, `playerStatus.ts` are pure (tests
  cover them); `useChaptersPanel` holds the use case (which surface is open, media queries, focus return,
  storage); the expanded set / filter / sheet level are view state inside the components, as the brief asked; the
  page only wires. `MiniPlayer` and `PlayerPopover` take slots and "may do" props (`showChaptersButton`), no mode
  flags. Status is passed as `StatusPart[]` so only the chapter name can shrink.
- **Record maps, no `any`, one-line imports (§2):** `LAYOUT_CLASS`, `PROGRESS_CLASS`, `PROGRESS_TEXT`,
  `STATUS_PARTS`, `ONE_LEVEL_DEEPER`; no `any`, no casts; all imports single-line.
- **Performance (§7):** `nodeView`/`entryLength` call `entryEndSentenceId` (O(n)) per row per render, so a render is
  O(n²) in toc entries. Measured in node with 390 entries over 8,000 sentences: **0.10 ms per full-rail render**, so
  no memoisation layer was added (the rule says "where measured to matter"). Tree, `sentenceIdEnd` and
  `tocLocation` are memoised in the screen; the rail re-renders ~300 rows per sentence step, which is cheap.
- **CSS (§11):** every `styles.*` in the four new modules resolves and every class is referenced (`ChaptersPanel`,
  `ChaptersRail`, `ChaptersSheet`, `ChapterLine`); no transitions/animations beyond the design (the design's chevron
  transition and sheet slide-up were deliberately left out); the only hover states are the row hover and the head
  button hover that the design has.
- **Contrast (§10):** measured in headless Chrome on the real composited backgrounds. Lowest text pair: `--fg2` on
  `--bg2` **5.47:1** light (pill status, sheet meta, read titles in the sheet, handle) / 7.33 dark; `--fg2` on
  `--bg` 6.09 / 8.18 (rail meta, read titles, part headings, sizes); `‹ All chapters` accent on `--bg2` 5.72 / 8.17.

## The build report's deviations: justified?

- `usePlayback` exposes `order` (read-only): **yes** — "min left in this chapter" needs `charsFrom`; rebuilding the
  order in the screen would duplicate state the hook already owns.
- `useFollowScroll` scrolls instantly for jumps > 2 screens: **yes** — a smooth scroll across ~35,000 px of
  `content-visibility: auto` blocks lands off target as estimated heights are replaced; the constant is named and
  documented. Short moves stay smooth.
- Popover open state lifted to `ReaderScreen` (controlled): **yes** — "Chapters…" must close it; a controlled
  `isPopoverOpen` / `onPopoverOpenChange` pair is the rules' `value` + `onChange` shape.
- `tocLocation` instead of the brief's `currentChapter` (indexes, not entries): **yes** — one computation feeds the
  pill, the popover and both panels; the tests cover the CA / K8s / DDS / LinkedIn / Long Book shapes from brief §3.
- `C` as a simple toggle (closes the rail from the text too): the design scrolled the rail to the current chapter
  instead; the user chose the toggle (CLAUDE.md "My defaults: `C` = simple toggle"). Kept.
- Pill chapters button hidden while the rail shows; button / "Chapters…" / `C` present for a book without chapters:
  as in the design and brief §5 "Empty state". Kept.
- Focus return after "Chapters…" from the popover falls back to "more" or the body: same as the design. Kept.

## Checks after the changes

- `npm run build` ✓, `npm run lint` (oxlint) 0 warnings ✓, `npm test` 21/21 ✓.
- Headless Chrome 1280 × 800 and 375 × 740, light and dark, mock mode (own vite on port 5199, stopped afterwards;
  CDP script in the session scratchpad, nothing added to the project): **49/49** checks — Continue lands on
  `Paused · Ch. 7 · Long Documents · p. 265` with 7.3 current, visible and `aria-current`; focus not stolen on load;
  filter (`contrast` → chapter 8 expanded, typing `c` in the filter does not close the rail, `zzzz` → "No titles
  match.", Escape clears then leaves); `C` closes / reopens the rail with focus on the current row, storage `0`/`1`;
  ← → inside rail and sheet never step sentences (now including level changes), Space on a row jumps and stays
  paused; wheel over the rail keeps following, over the text stops it, locate resumes; a rail title click jumps,
  keeps the rail, re-colours rows (ch. 1 read, ch. 7 unread); Collapse all 16 / Expand all 304; jump-to-current;
  popover chapter line + `% of this chapter · … in it`, "Chapters…" closes the popover and focuses the rail; sheet
  non-modal on desktop, labelled by its heading, opens on the current chapter's sections, → drills, ← backs out,
  Enter jumps and closes with focus back on the pill button, Escape / click outside / pill button close it; handle
  reopens the rail; empty book (no rail, no handle, page-only status, `C` → empty text, popover without chapter
  line but with "Chapters…"); Calm `Ch. 1 · Quiet Interfaces`; 375: status hidden, no horizontal scroll, modal
  sheet full width with drill sizes hidden, Escape returns focus to the pill button, backdrop tap does nothing,
  popover fits, `C` opens/closes the sheet.
- Screenshots viewed (not added to the repo): rail light/dark at 1280 and sheet light/dark at 375 match take 1 /
  label A.

## Left as is, and why

- The design's sheet has a **"Show as rail"** button (≥ 1200 px, rail closed); the build has only the top-left
  handle. Behaviour the build report did not promise; adding a feature is outside a cleanup review. Worth a
  one-line decision from the user.
- `ChaptersSheet` is remounted via `key={String(isSheetModal)}` when the window crosses 720 px while open; the
  `[isModal]` effect alone would reopen the dialog but not refocus it, so the key is the simpler correct option.
- Escape in the modal sheet is handled by the dialog's `onKeyDown` and also guarded by `onCancel`; a double
  `closeSheet` would blur the just-focused opener, but Chrome does not fire `cancel` for a prevented keydown
  (verified: focus ends on the pill button at 375 px). `onCancel` stays for non-keyboard close requests.
- Not run: a screen-reader session (Orca) and the real-server pass; the build report covered the latter and the
  markup (`nav › ul › li › button` with page + state in the name, `aria-current`, `aria-expanded` + `aria-controls`,
  labelled dialog) is unchanged apart from the dialog label above.
