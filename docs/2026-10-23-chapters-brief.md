# Chapters: design + build brief for Fable (2026-10-23)

The user asked: *"we need to specify each chapter beside the page number, and a new feature that allows the user to
see all the chapters and each chapter's sub-chapters, and by clicking on them move to that part."*

This brief has two stages:

1. **Options file (now):** a standalone local HTML file, `design/2026-10-23-chapters-options.html`, with **4
   structurally different options** (§5), interactive, built on design D. The user picks one (or a mix).
2. **Build (later, after the pick):** the chosen option in React, in `frontend/` (§6–§8).

Fable doesn't research: everything you need is here. The backend part is done (`docs/progress/chapters-backend.md`).
**Never publish Artifacts; never commit.** The repo will be public: **no text from the user's books goes into any
file** (book titles of chapters are fine, prose is not). The options file and the mock use the invented books below.

---

## 1. What exists today (design D, as built)

Read these before starting: `design/2026-10-06-reader-options.html` (option D tab), `frontend/src/pages/ReaderPage/ReaderScreen.tsx`,
`components/{MiniPlayer,PlayerPopover,PageMarker,ReadingView}/`, `styles/tokens.css`, `styles/controls.module.css`.

- **No header, no sidebar.** The page is just the article: serif 1.1875rem / 1.7, column `max-width: calc(40rem + 2 * var(--gutter))`,
  `--gutter: 4.5rem` on each side (0 at ≤ 720 px). Bottom padding 9rem keeps text clear of the pill.
- **Page markers** ("p. 37", title "PDF page 61"): sans .75rem, `--fg2`, absolutely placed in the left gutter, right-aligned,
  before the sentence where the page changes. At ≤ 720 px they become inline bordered chips in the text. Not selectable, never spoken.
- **The pill** (fixed, bottom centre; at ≤ 720 px it spans `left/right: 1rem`): prev (36 px) · play (44 px, accent) · next (36 px) ·
  status text (`role="status"`, `--fg2`, `max-width: 14rem`, ellipsis; **hidden at ≤ 420 px**) · locate (only when follow-scroll
  is off) · more (36 px, `aria-expanded`). Status today: `p. 37 of 412`, `p. 37 of 412 · 1.5×`, `Paused · p. 37 of 412`,
  `Preparing audio…`, `Finished`, `Speech engine unavailable`.
- **The popover** (from "more"; above the pill, right-aligned, `width: min(21rem, 100vw - 2rem)`, full width at ≤ 720 px,
  `max-height: calc(100dvh - 6rem)`, scrolls): progress slider + "12 min left", location line `Page 37 · PDF page 61 of 429`,
  speed, voice, "Read code & tables aloud", export panel, "Open another PDF…". A disclosure, not a modal: Escape or a click
  outside closes it, focus is not trapped, Escape returns focus to "more".
- **Shortcuts** (`hooks/useReaderShortcuts.ts`): Space play/pause, ← / → previous/next sentence; ignored in form fields and with
  modifiers. **Follow-scroll** (`hooks/useFollowScroll.ts`) keeps the current sentence in view until the reader scrolls (wheel,
  touchmove, PageUp/Down/Home/End/↑/↓, scrollbar); the locate button resumes it.
- **Jumping:** `playback.jumpTo(sentenceId)` accepts any sentence id (it resolves to the next playable one) and **keeps the
  play/pause state**. `ReaderScreen` wraps every jump in `andFollow(…)` so follow-scroll resumes. A chapter click is exactly
  `selectSentence(entry.sentenceId)`; nothing new is needed in `usePlayback`.
- **Tokens (light / dark):** `--bg #ffffff / #121417`, `--bg2 #f3f3ef / #1c1f24`, `--fg #1c1f24 / #e8e6e1`,
  `--fg2 #5b636d / #a8adb5`, `--line #d3d3cd / #363a41`, `--accent #1d5fae / #8ab8ff`, `--on-accent #ffffff / #0b1a33`,
  `--hl #ffe58f / #4d4214`, `--hl-fg #1c1f24 / #fff6d8`. Measured: `fg2` on `bg2` 5.47:1 light (7.33 dark), `fg2` on `bg` 6.09,
  `accent` on `bg2` 5.72. **Every text ≥ 4.5:1** against its real background; don't mute with `opacity` (it breaks the
  measurement), use `--fg2`. Fonts: `--serif`, `--sans`, `--mono` as in tokens.css.

## 2. The data: `toc`

`GET /api/documents/{id}` now carries `toc` (spec §5). Already in `frontend/src/types/document.ts`:

```ts
export type TocLevel = 1 | 2 | 3;
export interface TocEntry { id: number; title: string; level: TocLevel; sentenceId: number; pageIndex: number; }
// CleanDocument.toc: TocEntry[]   (empty when the PDF has neither bookmarks nor headings)
```

Guarantees from the backend: flat, in reading order; `sentenceId` strictly increases; the first entry has level 1; a level is
at most one deeper than the entry before; never deeper than 3; titles are trimmed, single-line, ≤ 121 chars. `pageIndex` is the
PDF page; show its printed label with the existing `printedLabel(pageLabels, pageIndex)` ("37", "xii"). A part ends where the
next entry of the **same or a higher** level starts (or at the end of the book). Sentences before the first entry (cover,
title page) belong to no entry.

**What real books look like** (the user's 5 books; plan the design for all of these shapes):

| Book | Entries | Shape | Titles look like |
|---|---|---|---|
| Clean Architecture | 222 | **3 levels: parts → chapters → sections**; front matter at level 1 | `PART I Introduction` › `Chapter 1 What Is Design and Architecture?` › `The Goal?` |
| Designing Distributed Systems | 83 | preface + Chapter 1 at level 1, then **parts at 1, chapters at 2**, sections at 3 | `Part I. Single-Node Patterns` › `Chapter 2. The Sidecar Pattern` |
| Design Patterns (GoF) | 390 | chapters 1 › patterns/sections 2 › 3 | `1. Introduction`, `3. Creational Patterns` › `Abstract Factory` |
| LinkedIn Posts 2024 | 198 | **completely flat** (all level 1, no children) | long post titles, e.g. `Comparing Different API Clients: Postman vs. Insomnia vs. ReadyAPI vs. Thunder Client vs. Hoppscotch` |
| The Kubernetes Book | 125 | chapters 1 › sections 2 | `4: Working with Pods` › `Pod theory`; also `Chapter summary` in every chapter |

So: **"chapter" is not always level 1** (parts sit above chapters in two books), titles already carry their own numbering in
different styles (`Chapter 5 …`, `Chapter 2. …`, `4: …`, `1. …`), many sections repeat the same name (`Summary`,
`Conclusion`, `Chapter summary`), and one book is a flat list of ~200 long titles. Don't add your own chapter numbers by
counting rows: use the title's own (§6, `chapterLabel`).

**Mock data** (already in `frontend/src/api/mock/`, use it in the options file too, copied as plain JS):
- **"The Long Book (15 chapters, about 7,000 sentences)"** (`mock-big`): Preface + 15 chapters of 3–8 sections (`7.3 Tables of
  contents that tell the truth`), each section with 3 level-3 sub-sections (`Why it matters`, `In practice`, …). 304 entries,
  chapter 4 has a very long title (*"Designing for Interruptions: How People Really Read on Phones, on Trains and Late at Night
  in Bed"*), section 6.4 too. **"Continue" lands in chapter 7, section 3**, so the current chapter is mid-book. The chapter and
  section names are `LONG_BOOK_CHAPTERS` / `LONG_BOOK_SUBSECTIONS` in `sampleText.ts`.
- **"Designing Calm Software"** (`mock-calm`): small, 6 entries (Preface, 1. Quiet Interfaces › 1.1–1.3 › What to try next).
- **"Meeting Notes (no chapters)"** (`mock-flat`): `toc: []`, for the empty state.

For the options file also make up (invented titles, any wording) a **parts › chapters › sections** book of ~8 parts / 30
chapters, and a **flat** book of ~200 long titles, to show those shapes.

## 3. Definitions every option uses

- **Current entry:** the last entry whose `sentenceId ≤` the current sentence id (any level). `null` before the first entry.
- **Path:** the current entry and its ancestors (walk back: each earlier entry with a smaller level than the last one taken).
- **Current chapter:** the first entry on the path whose title looks like a chapter (`Chapter 4…`, `Ch. 4`, `4: …`, `4. …`),
  else the path's first entry. **Current section:** the path entry right after the chapter, if any.
  Examples: CA in `Proof` → chapter `Chapter 4 Structured Programming`, section `Proof`; K8s in `Pods` → chapter
  `2: Kubernetes principles of operation`, section `Pods`; DDS in `Motivations` → chapter `Part I. Single-Node Patterns`
  (no chapter-like title on the path); LinkedIn → the post's title, no section; the Long Book → `7. Long Documents`,
  `7.3 Tables of contents…`.
- **Short chapter label:** a chapter-like title splits into a number and a name: `Chapter 4 Structured Programming` →
  `Ch. 4` + `Structured Programming`; `4: Working with Pods` → `Ch. 4` + `Working with Pods`; `7. Long Documents` → `Ch. 7` +
  `Long Documents`. Anything else (`Preface`, `PART II …`, `Appendix A …`, a LinkedIn title) has no number: show it whole.
- **Entry progress:** `read` (the whole part is before the current sentence), `current` (on the path), `unread`.
  The current chapter also gets **% through** (by sentence ids: `(current − start) / (end − start)`) and, optionally,
  **time left in the chapter** (same estimate as the popover's "12 min left": characters × `SECONDS_PER_CHAR` ÷ speed).

## 4. Part 1: the chapter beside the page number

Where it shows, how it reads, and how it truncates. **Include these three variants in the options file** (each option in §5
uses one; the file must let the user switch the variant independently of the panel option, e.g. a "Chapter label: A / B / C"
control in the top bar).

**Variant A: in the pill status.** `Ch. 7 · Long Documents · p. 412` (playing), `Paused · Ch. 7 · Long Documents · p. 412`,
`… · p. 412 · 1.5×`. Drop the "of 589" (the popover still has it). Truncation: three spans; `Ch. 7` and `p. 412` never
shrink, only the name gets the ellipsis (`flex: 0 1 auto; min-width: 0`). Let the status grow to ~22rem on wide screens (pill
stays centred, `max-width: calc(100% - 2rem)`). No chapter: today's text. Phone (≤ 420 px, status hidden today): the pill
can't fit it (at 375 px the buttons take ~260 of 343 px), so on phones the chapter shows in the popover (and in variant B's
head if chosen). `role="status"` stays on the status; it changes only when the page or chapter changes, which is fine.

**Variant B: a slim running head.** A one-line sticky bar at the top of the reading view, like a book's running head:
`7. Long Documents › 7.3 Tables of contents that tell the truth` left, `p. 412` right. Sans .8125rem, `--fg2` on `--bg`,
bottom border `--line`, height ~2rem, full width at every size (so it **works at 375 px**). Ellipsis on the section first, then
the chapter (or chapter name only below 420 px). Clicking it opens the chapters panel. It must sit below the existing banners
(`.banners` is sticky top, z 11) and must not jump when they appear. The pill keeps its page text.

**Variant C: in the margin.** The page markers stay "p. 37", and **the first marker after a chapter starts** gets a second
line with the short label (`Ch. 7`, or the name cut to the gutter width, ~3.75rem, with ellipsis and the full title in
`title`). On phones the inline chip reads `p. 37 · Ch. 7` on that first marker only. Quiet, no new chrome, but only visible
near chapter starts, so pair it with variant A for "where am I now".

**In every variant, the popover** gets a chapter line above the location line:
`7. Long Documents › 7.3 Tables of contents that tell the truth` (wraps to 2 lines, then ellipsis), then
`38% of this chapter · 9 min left in it` (`--fg2`), then the existing `Page 412 · PDF page 425 of 589`, and a
**"Chapters…"** button (opens the panel) next to it. This is the phone's main way to see the chapter.

## 5. Part 2: the chapters panel: 4 options

Each option is a **different structure**, not a restyle. All open from D's pill/popover (D has no sidebar), and all from the
`C` key. In the options file each option is a tab; each tab is a working reader (text, pill, popover, panel) at desktop width,
plus a "Phone 375" toggle that renders the same in a 375 × 740 frame.

1. **Drawer / sheet.** A side drawer (right, `min(24rem, 100vw)`, full height, `--bg`, left border `--line`) opened by a new
   pill button ("Chapters", list icon, 36 px, before "more") and the popover's "Chapters…" button. Modal (`<dialog>` with
   `showModal()`: focus moves in, background inert, Escape closes, focus returns to the opener). Header: book title + close
   button. Body: the tree with collapsible chapters. On phones it becomes a **full-screen sheet** sliding up, header sticky.
2. **The popover grows a tab.** No new surface: the "more" popover gets two tabs at its top, **Controls | Chapters**. Chapters
   shows the tree inside the popover (same width, `max-height: calc(100dvh - 6rem)`, scrolls). The pill status (variant A) or
   a click on the chapter line opens the popover straight on the Chapters tab. Keeps D's "everything lives in the pill".
   Note the cost: 21rem wide, so long titles wrap more; the popover stays a non-modal disclosure.
3. **Bottom sheet from the pill.** The pill's chapter button lifts a sheet out of the pill: anchored at the bottom, centred,
   `min(40rem, 100vw - 2rem)` wide, up to 70vh tall, rounded top corners, the pill stays visible below it so play/pause still
   works while browsing. Shows **one level at a time**: the chapter list; tapping a chapter's "›" drills into its sections
   (header gets "‹ All chapters"); tapping a title jumps. Non-modal on desktop, modal on phones (full width, 85vh).
4. **Slim rail on wide screens.** At ≥ 1200 px a quiet always-visible rail sits left of the article, outside the text column
   (`position: fixed`, ~15rem, `--fg2` text, no background): every chapter as one line, **only the current chapter expanded**
   to its sections, the current section highlighted, and the rail scrolls itself to keep the current row in view. Below
   1200 px the rail is gone and option 1's drawer is used (same button, same tree). This one departs from D's "nothing but
   text"; say so in the option's note.

**Every option must show:**
- **Rows:** title (wraps to 2 lines max, then ellipsis; full title in `title`), the printed page label right-aligned
  (`p. 412`, `xii`), and progress: `read` rows get a small check and `--fg2` text, the `current` chapter and section get the
  accent (3 px accent bar on the left + `font-weight: 600`) and the chapter shows `38%` or a thin progress bar, `unread` rows
  are plain `--fg`. Collapsed chapters show their size (`6 sections`). Indent 1rem per level.
- **Opening:** the current chapter's path is expanded and **the current row is scrolled into view** (centred, no smooth
  scroll), and focus goes to the current row.
- **Expand/collapse:** a separate disclosure button per row with children (chevron, `aria-expanded`, label "Show sections of
  7. Long Documents"); the title button jumps. A flat list (LinkedIn) has no disclosure buttons. Optional "Expand all".
- **Jump:** clicking a title jumps there (`jumpTo(entry.sentenceId)`), **keeps play/pause as it was**, resumes follow-scroll
  (the text scrolls to that heading), and closes the panel (drawer, sheet, popover tab) or keeps it open (rail).
- **Keyboard:** `C` toggles the panel (ignored in form fields and with modifiers, like the other shortcuts); in the panel
  ↑ / ↓ move between visible rows, → expands (or moves to the first child), ← collapses (or moves to the parent), Home/End,
  Enter or Space jumps, Escape closes and returns focus to the opener. Inside the panel ← → Space must **not** reach the
  reader shortcuts, and ↑ ↓ / wheel / touch scrolling inside it must **not** stop follow-scroll (see §7).
- **Screen readers:** `<nav aria-label="Chapters">` with nested `<ul>`/`<li>`; each title a `<button>` whose name includes
  the page and state ("7.3 Tables of contents that tell the truth, page 412, current"); `aria-current="location"` on the
  current row; disclosure buttons with `aria-expanded` + `aria-controls`. (A full `role="tree"` is acceptable instead, but
  then follow the WAI-ARIA tree pattern exactly: roving `tabindex`, `aria-level`, `aria-expanded` on the `treeitem`.)
- **Empty state** (`toc: []`): the button stays (the `C` key too) and the panel says *"No chapters found. This PDF has no
  bookmarks or headings Lectrice could detect. Use the progress slider in ⋯ to move around."* The pill/popover show no
  chapter text, only the page, exactly as today.
- **Volume:** the Long Book (15 chapters, 3–8 sections each, long titles, current = 7.3), the parts book, the flat book of
  ~200, and the empty one, switchable from the file's top bar. Rendering up to ~400 rows at once is fine (no virtualization).

**The options file itself:** one self-contained `.html` (no network, no libraries), D's tokens copied with the
`:root:not([data-theme="light"])` / `[data-theme="dark"]` pattern from the 2026-10-06 file, a top bar with: option tabs 1–4,
chapter-label variant A/B/C, book (Long / Parts / Flat / Empty), theme (auto/light/dark), "Phone 375" toggle, and a
play/pause that advances the current sentence on a timer (so the label, `%` and rows update live). Under the tabs a short
note per option: what's good, what it costs. Real-looking body text is invented filler (not from any real book).
Verify at 1280 px and 375 px, light and dark, no horizontal scroll, every text ≥ 4.5:1.

## 6. The build (after the user picks): pure helpers

New `frontend/src/utils/toc.ts` (pure, no React; tests in `tests/utils.test.ts`):

```ts
export interface TocNode { entry: TocEntry; children: TocNode[]; }
export type EntryProgress = 'read' | 'current' | 'unread';

export const buildTocTree = (toc: readonly TocEntry[]): TocNode[];                       // from the flat list, using level
export const currentEntryIndex = (toc: readonly TocEntry[], sentenceId: number | null): number | null;  // binary search
export const tocPath = (toc: readonly TocEntry[], index: number): TocEntry[];           // ancestors first, then the entry
export const entryEndSentenceId = (toc: readonly TocEntry[], index: number): number | null;  // next entry with level ≤ this, or null = end
export const currentChapter = (toc: readonly TocEntry[], sentenceId: number | null): { chapter: TocEntry; section: TocEntry | null } | null;
export const chapterLabel = (title: string): { number: string | null; name: string };   // "Chapter 4 X" → { "4", "X" }
export const entryProgress = (toc: readonly TocEntry[], index: number, sentenceId: number | null): EntryProgress;
export const percentThrough = (toc: readonly TocEntry[], index: number, sentenceId: number, sentenceCount: number): number;  // 0–100
```

- `chapterLabel` recognises, case-insensitively, `Chapter N`, `Ch. N` (N digits or roman), and a leading `N:` / `N.` followed
  by a space. `20 Popular Open Source Projects…` and `10 years ago, Amazon…` (no `:`/`.`) are **not** chapters. Keep the regex
  as a named constant.
- Time left in the chapter goes next to `secondsLeft` in `utils/playbackOrder.ts`:
  `secondsBetween(order, fromSentenceId, toSentenceId | null, speed, secondsPerChar)` from `order.charsFrom`, resolving
  non-playable ids with `resolvePlayable`.
- Formatting strings (`Ch. 7`, `p. 412`, `38% of this chapter`) go in `utils/` beside `formatPageMarker`, not in JSX.
- Tests: tree from a 3-level list and from a flat list; current entry before the first entry / on an entry's first sentence /
  mid-part / after the last; path; end ids; `currentChapter` for the CA-, K8s-, DDS- and LinkedIn-shaped examples in §3;
  `chapterLabel` for every title style in §2 plus the two non-chapters; progress states; `secondsBetween`.

## 7. The build: which files change (frontend rules' layers)

- `types/document.ts`: done (`TocEntry`, `TocLevel`, `CleanDocument.toc`). The mock (`api/mock/`): done.
- `utils/toc.ts` (new), `utils/playbackOrder.ts` (`secondsBetween`), `utils/pages.ts` or `utils/playerStatus.ts`
  (status text with the chapter; keep `playerStatusText` a pure function and update its tests).
- **New component(s)** `components/ChaptersPanel/` (presentational: props `tree`, `currentEntryId`, `progressOf(id)` or a
  precomputed map, `pageLabelOf(pageIndex)`, `onSelect(sentenceId)`, `onClose`; the **expanded set is view state inside it**,
  initialised to the current path). Put a repeated row in a module-scope `TocRow` component in the same file.
  Variant B adds `components/RunningHead/`; variant C changes `PageMarker` (an optional `chapterLabel` prop) and
  `utils/pages.ts` (which markers carry it).
- `MiniPlayer`: take the status as a slot (`status: React.ReactNode`) instead of a string if variant A needs separate spans,
  and a `chaptersButton` slot (rules §1 "pass the difference in"). No mode flags.
- `PlayerPopover`: a `chapterSection: React.ReactNode` slot above the location line (option 2: the Controls | Chapters tabs
  are view state inside the popover).
- `pages/ReaderPage/ReaderScreen.tsx`: `useMemo` the tree and the current chapter/section from `playback.currentSentenceId`;
  `isChaptersOpen` lives here (three triggers: pill button, popover button, `C` key); the panel's `onSelect` is the existing
  `selectSentence` (= `andFollow(playback.jumpTo)`), plus closing the panel.
- `hooks/useReaderShortcuts.ts`: add `onToggleChapters` (`c`/`C`, no modifiers, not in form fields) and **ignore every key
  whose target is inside the panel** (e.g. `target.closest('[data-own-keys]')`).
- `hooks/useFollowScroll.ts`: **ignore wheel, touchmove and scroll keys whose target is inside the panel** (same marker,
  e.g. `[data-own-scroll]`); today any wheel anywhere stops following. The `MiniPlayer` outside-click handler will close the
  popover when the panel is clicked; that's fine.
- No change to `usePlayback`, `api/`, or the backend.

## 8. Acceptance checklist (for the build) and commands

- [ ] Long Book opened with "Continue": the label reads `Ch. 7 · Long Documents · p. …` (or the chosen variant), the popover
      shows `7. Long Documents › 7.3 Tables of contents that tell the truth` and `% of this chapter`.
- [ ] Playing across a chapter boundary updates the label at the first sentence of the new chapter; before the first entry
      (cover) only the page shows.
- [ ] Panel opens from every trigger and from `C`; the current path is expanded, the current row is visible and focused.
- [ ] Clicking a section while **playing** keeps playing from that heading; while **paused** stays paused there; the text
      scrolls there and follow-scroll is on (locate button hidden).
- [ ] Read / current / unread rows are right after jumping backwards and forwards.
- [ ] Expand/collapse by mouse and keyboard; ↑ ↓ → ← Home End Enter Escape as in §5; ← → Space inside the panel do not step
      sentences or toggle play; scrolling the panel does not stop follow-scroll.
- [ ] Escape / close returns focus to the opener; modal variants trap focus and make the page inert.
- [ ] Screen reader (Orca or VoiceOver): the nav is announced, rows say title + page + state, `aria-current` and
      `aria-expanded` are read.
- [ ] Calm Software (6 entries), Long Book (304), Meeting Notes (empty state text, no chapter in pill/popover).
- [ ] Long title (chapter 4) truncates per §4 in the pill/head and wraps to 2 lines in the panel; no horizontal scroll at
      375 px; panel/sheet usable at 375 px.
- [ ] Light and dark; every text ≥ 4.5:1 (measure, list the lowest pair in the report).
- [ ] The Long Book still opens fast (≈0.7 s in mock mode before; no regression with the panel closed).
- [ ] Against the real server (`VITE_API_MODE=http`, after the backend re-cleans a book to version 3): K8s shows
      `4: Working with Pods`-style chapters, Clean Architecture shows parts › chapters › sections, LinkedIn a flat list.

```bash
cd frontend && npm run build && npm run lint && npm test     # 0 lint warnings, all tests pass
npm run dev                                                    # mock mode, http://localhost:5173/
cd ../backend && . "$HOME/.cargo/env" && cargo run --release -p lectrice-api   # real mode (port 7878)
```

Report in `docs/progress/chapters-frontend.md`; screenshots (invented mock text only) in `design/screens/`.
