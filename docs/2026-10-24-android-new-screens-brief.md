# Lectrice for Android: new screens brief for Fable (2026-10-24)

Status: **design decisions made; ready to build.** Opus wrote this (decisions, exact specs). **Fable builds §3–§9 and makes no
design decisions of its own.** Where this brief is silent, keep what `design/2026-10-23-android-quiet-v2.html` ("the
mockup") already does and say so in the progress note.

Inputs: the approved mockup and its brief `docs/2026-10-23-android-quiet-v2-brief.md` ("the v2 brief"), the logic spec
`docs/android-logic/` (README §7; files 02, 10, 11, 12, 20, 21), the user's decisions of 2026-10-24 in `CLAUDE.md`.

Reading guide for the user: **§1 what's new and why, §2 where the design follows the spec against my instinct, §10
questions.** §3 onwards is for Fable.

---

## 1. What's new and why (short)

Three groups of screens, all in the Quiet v2 style (same tokens, type, rows, sheets, dialogs, snackbars; light / dark /
black; 412 and 360 wide; no visible scrollbars). Nothing new is invented visually: every new screen is built from parts
the mockup already has.

1. **Remove a book (L7, L7c, L7u).** Long-press a Library row or the Continue card → a short **book sheet** (cover tile,
   title, sizes, "Clear this book's audio", "Remove from library" in red) → a **confirm dialog** that says what goes, how
   much it frees, and only the extra lines that apply ("Playback stops.", "Its export in progress is cancelled.", the
   "also delete its exported audiobooks" checkbox, unticked) → the row slides away and a **snackbar with Undo** stays 8 s.
   Why this shape: destructive actions live where the thing is listed (Library), one step deep, never next to "Library"
   in the player; the dialog is honest about the numbers; Undo makes a mistake cheap.
2. **Storage (T1s, T3…, X1).** One "Storage" row in Settings opens one screen that shows every byte: a summary card
   ("1.2 GB used by Lectrice · 23.4 GB free on this phone") with a thin stacked bar, then Voice models, Books (largest
   first, same book sheet), Audio cache (bar + "Clear audio cache"), Exported audiobooks (Open / Share / Save to
   Audiobooks / Delete), and at the bottom the phone's storage page and "Delete all Lectrice data". The same screen opens
   from **Android's** Settings → Apps → Lectrice → Storage → "Clear storage" (with ✕ instead of ←); that Android page is
   drawn as a labelled mock (X1) so the user sees the whole path. Why: the user asked to see and clear the cache from the
   system too; one screen with explanations is safer than Android's "wipe everything".
3. **Export and space (E6, E7, E5n, E5b, E3h, E3p, E4s, E2p, E4p).** The export page already shows a live estimate; now
   the estimate also **checks the space**. If the whole book doesn't fit, the footer turns into a calm warn card with two
   ways on: **Export part of the book** (primary: a From/To chapter range that fits is already picked) or **Free up
   space** (opens Storage with a banner "Free up about 265 MB to export this book."). Running out of space mid-export
   pauses and keeps the parts (E5 gets a "needs / free now" line); running out while assembling is E5b; heat and battery
   pauses get their own lines; "Save to Audiobooks" **moves** the file and the button turns into a disabled "Saved to
   Audiobooks".

Decisions I made where the spec is silent (marked "Opus" in the copy table):

- **Room-now banner:** on the Storage screen opened from an export, once enough is freed the banner changes to a neutral
  card "There's room now to export this book." + "Back to the export" (E5 context: "There's room now. The export
  continues."). Without it the user frees space and gets no signal that it was enough.
- **"Clear this book's audio" is hidden** when the book has no audio (nothing to clear); the meta then reads only
  "PDF and text 5 MB".
- **T3c "Save to Audiobooks" row has the subtitle "Moves the file to Audiobooks/Lectrice"**, so the move isn't a
  surprise when the row then says "In Audiobooks".
- **An unfinished export in T3c** shows the note "To delete it, cancel the export on its page." (the spec says it can't be
  deleted there; the note says why the Delete row is missing).
- **E4s** adds "Make another version" (spec §2 step 6 lists it; the old E4 frame doesn't have it).
- **E5b's progress area** reads "Exporting · 98 %" / "All 49 parts made · putting them together" (the spec gives only the
  banner).
- **Stacked bar colours:** the strong member of each cover-hue pair (`--onTile0…4`), so segments are visible on `bg2` in
  light and dark. Legend dots use the same colours. Decorative only (rows carry the numbers).
- **Snackbar action ("Undo")** is `label` 500 in `inverseFg` (no accent on the inverse surface: accent on `inverseBg`
  fails 4.5:1 in both themes, and the rules forbid new colour pairs).
- **Range labels:** "Chapters 7–12", "Chapter 7" for one, "Preface–Ch. 3" when the range starts at the unnumbered
  Preface; pages: "Pages 97–272".
- **Mockup-only controls** (page chrome under the live phone, not app UI): "Phone space: Plenty | Nearly full" and
  "Android Settings ›". They let the user try E6/E7/E5 and the system path.

## 2. Where I follow the spec although a design reading differs (please check)

1. **E5b copy says "Free up about {needed}"** with needed = F + M (342 MB), while the line under it says "Needs about
   342 MB free · 120 MB free now". The user really has to free 222 MB. Built as the spec says; Q2 asks to change it to
   `needed − free`.
2. **The new frames are M4B-only** (no Format group, summaries "Heart · Full quality · 1×") as the logic spec says. The
   **existing** frames E1, E1v, E1q, E1x, E2–E5, N3 and the live phone still show the Format group and "M4B" in the
   summary, because this pass must not change them. Q1 asks whether to clean them up.
3. **Clearing the audio cache doesn't raise "free".** Free space is `getAllocatableBytes`, which already counts our
   cache (`cacheDir`) as free (file 10 §5). So in the live demo, "Clear audio cache" lowers Lectrice's total but not the
   export's shortfall; deleting exports or removing a book does. The banner number follows that rule.
4. **Removing a book frees "about {library + cache} MB"** even when its exports' checkbox is ticked (spec §3.1 defines n
   that way). The dialog doesn't re-count when the box is ticked.
5. **"Save to Audiobooks" doesn't change the phone's free space** (the file moves on the same phone); it lowers Lectrice's
   own total only. The live demo follows that.

---

## 3. Build instructions for Fable: the output

- **Edit one file:** `design/2026-10-23-android-quiet-v2.html`. **Add** to it; keep every existing frame, its look, its
  caption and the live phone's existing behaviour unchanged. The only allowed changes to existing code are the small,
  backwards-compatible hooks listed in §4.3 (new optional parameters with defaults that reproduce today's output, new
  `data-*` attributes that change nothing visible, new `case`s in switches). After your edits, every existing frame must
  render pixel-identical (check: re-take `android-q2-L1-light`, `E1-light`, `E4-light`, `T1-light`, `R4-light` into the
  scratchpad and compare with the files in `design/screens/`; report the result).
- No new libraries, no network assets (the Google Fonts link stays as it is), no Artifact, no git, nothing downloaded,
  nothing outside `design/` and your progress note `docs/progress/android-new-screens.md`.
- Screenshots: `design/screens/android-q2-<ID>-<theme>[-360].png`, new IDs only (§9). **Don't overwrite any existing
  file.**
- All placeholder content is invented (the titles below come from the mockup's `BOOKS` plus one new invented title).
  **No text from the user's books.**

### 3.1 Where the new frames go

| Gallery (`FRAMES` key) | Append after | New frames, in this order |
|---|---|---|
| `library` | `L6` | L7, L7l, L7c, L7ce, L7cx, L7u, L7z |
| `export` | `E5` | E6, E6n, E7, E7f, E7t, E7x, E7p, E2p, E4p, E3h, E3p, E5n, E5b, E4s |
| `settings` | `T2c` | T1s, T3, T3e, T3l, T3z, T3g, T3b, T3c, T3ca, T3cu, T3cd, T3d, T3k, T3f, T3r, T3s, X1, X1c |

- Each new frame object gets `isNew: true`. In `buildPage`, a frame with `isNew` renders its caption as
  `${id} · ${name} <span class="newtag">New</span><small>…</small>` (page chrome; existing captions unchanged).
- **Page chrome additions:**
  - Right after the `.live` block: `<aside class="impact" id="new-2026-10-24">` with `<h3>New on 24 October: storage,
    removing books, export space</h3>` and a `<ul>` of the three bullets of §1 (one sentence each: the first sentence of
    each numbered item in §1), then a second `<h3>Notes</h3>` with the five items of §2 verbatim as a `<ul>`.
  - "How to try": append items 8–13 (§8.6).
  - `#questions`: append a `<h2>Questions (24 October)</h2>` and an `<ol>` with §10's questions verbatim.
  - Under the live phone, in `.sysbar`, after "◁ Back": a `.seg`-style group `Phone space: [Plenty] [Nearly full]`
    (`data-space="plenty|low"`, `aria-pressed`) and a button `Android Settings ›` (`data-sys="android"`). These are
    labelled page chrome (same styles as the toolbar's `.seg`).

---

## 4. Tokens, CSS, icons, data, helpers

### 4.1 Tokens

**No new tokens.** Everything uses the existing sets. The only new "colours" are references to existing tokens:
`--onTile0…4` for the stacked bar, `--warnBg/--warnFg` for the warn card, `--inverseBg/--inverseFg` for snackbars.

### 4.2 New CSS (append one block at the end of the phone CSS, before the page-chrome overrides)

```css
/* ===== 2026-10-24: remove from library, storage, export space ===== */
.newtag{display:inline-block; margin-left:6px; font-size:11px; font-weight:600; padding:0 6px; border-radius:6px; border:1px solid var(--paccent); color:var(--paccent); vertical-align:1px}
/* book sheet header (L7 / T3b) and export-file sheet header (T3c) */
.bsh{display:flex; gap:16px; align-items:center; padding:0 16px 12px}
.bsh .tx{flex:1; min-width:0}
.bsh > svg{width:24px; height:24px; flex:none; color:var(--fg2); align-self:flex-start; margin-top:2px}
.row.danger, .row.danger > svg.lead{color:var(--errFg)}
/* checkbox row in the remove dialog */
.cbx{display:flex; align-items:center; gap:12px; min-height:48px; width:100%; text-align:left; margin-top:12px; color:var(--fg); font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans)}
.cbx svg{width:24px; height:24px; flex:none; color:var(--fg2)}
.cbx[aria-checked="true"] svg{color:var(--accent)}
.dlg .more{margin-top:8px}
/* snackbar with an action; .over = above a full-height sheet */
.snack .act{flex:none; height:48px; margin:-14px -8px -14px 0; padding:0 12px; font:500 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); color:var(--inverseFg)}
.snack.over{z-index:25; bottom:40px}
.snack.out{opacity:0; transition:opacity 150ms linear}
/* Library row leaving / coming back (200 ms) */
.gone{overflow:hidden; transition:height 200ms cubic-bezier(.2,.9,.25,1.05), opacity 150ms linear}
/* Storage: summary card, stacked bar, legend */
.sumcard{margin:0 16px; border-radius:12px}
.stack{display:flex; gap:2px; height:8px; border-radius:999px; overflow:hidden; margin-top:12px; background:var(--line)}
.stack span{display:block; height:100%; min-width:2px}
.k0{background:var(--onTile0)} .k1{background:var(--onTile1)} .k2{background:var(--onTile2)} .k3{background:var(--onTile3)} .k4{background:var(--onTile4)}
.legend{display:flex; flex-wrap:wrap; gap:4px 16px; margin-top:12px; font:400 calc(12*var(--sp))/calc(16*var(--sp)) var(--sans); color:var(--fg2)}
.legend span{display:inline-flex; align-items:center; gap:6px; white-space:nowrap}
.legend i{width:8px; height:8px; border-radius:50%; flex:none}
.shead{padding:0 16px 8px; font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); color:var(--fg2)}
.lrow .sz{flex:none; font:500 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); color:var(--fg)}
.row .t.errc{color:var(--errFg)}
.btn.txt[disabled]{background:none; color:var(--fg2)}
/* neutral "room now" card */
.okcard{display:flex; gap:12px; align-items:flex-start; flex-wrap:wrap; border-radius:12px; padding:12px 16px; background:var(--bg2); border:1px solid var(--line); font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans)}
.okcard > svg{width:24px; height:24px; flex:none; color:var(--okFg)}
.okcard .tx{flex:1; min-width:0}
.okcard .acts{display:flex; justify-content:flex-end; width:100%}
/* export footer warn card (E6) and the part page's range rows (E7) */
.warncard{display:flex; gap:12px; align-items:flex-start; margin-top:8px; border-radius:12px; padding:12px 16px; background:var(--warnBg); color:var(--warnFg)}
.warncard > svg{width:20px; height:20px; flex:none; margin-top:2px}
.warncard .t{font:500 calc(16*var(--sp))/calc(24*var(--sp)) var(--sans)}
.warncard .s{font:400 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans); margin-top:2px}
.xfoot .warnline{color:var(--warnFg)}
.xrange{display:flex; align-items:center; gap:16px; min-height:56px; border-radius:12px; background:var(--bg); padding:8px 12px; width:100%; text-align:left; color:var(--fg); flex:none}
.xrange + .xrange{margin-top:8px}
.xrange .tx{flex:1; min-width:0}
.xrange .tr svg{width:24px; height:24px; color:var(--fg2)}
.inner{position:absolute; inset:0; z-index:5}           /* a sheet inside a pushed page (E7 lists) */
.inner .scrimL, .inner .sheet{position:absolute}
/* Android's own Settings page (X1): a mock of a system screen */
.sysapp{display:flex; flex-direction:column; align-items:center; padding:8px 16px 0}
.sysacts{display:flex; gap:12px; padding:24px 16px 8px}
.sysact{flex:1; height:72px; border-radius:16px; background:var(--bg2); display:flex; flex-direction:column; align-items:center; justify-content:center; gap:4px; color:var(--fg); font:500 calc(14*var(--sp))/calc(20*var(--sp)) var(--sans)}
.sysact svg{width:24px; height:24px; color:var(--accent)}
.sysact[aria-disabled="true"], .sysact[aria-disabled="true"] svg{color:var(--fg2)}
```

Contrast (all existing pairs): `warnFg/warnBg`, `errFg/bg2`, `inverseFg/inverseBg`, `okFg/bg2`, `fg2/bg2` are the v2
pairs and already pass 4.5:1. The stacked bar and legend dots are non-text.

### 4.3 Allowed hooks in existing code (backwards-compatible)

| Existing | Change | Existing output stays identical because |
|---|---|---|
| `defaultView` | add `freeMB: 23400, m4bOnly: false, xRange: null` | new keys; nothing reads them yet |
| `exportSummary(v)` | if `v.m4bOnly`, drop `· ${v.xFormat}`; if `v.xRange`, append ` · ${rangeSuffix(v.xRange)}` | both false/null by default |
| `exportChoices(v, …)` | if `v.m4bOnly`, skip the Format header, its segmented control and its note | false by default |
| `exportFooter(v)` | if `!fitsWhole(v)` return `e6Footer(v)` (§6.1) | default free 23.4 GB fits everything |
| `exportBody(v, state, …)` | add the new keys E2p, E3h, E3p, E4p, E4s, E5n, E5b to `bodies`; E4's save button gets `data-act="save-audiobooks"` | old keys untouched; an attribute is invisible |
| `exportPage(v, state, opts)` | state `E1` unchanged; `opts.sub === 'part'` appends a second `<div class="page">` with `partPage(v, opts)` **inside `playerSheet`'s page wrapper** (playerSheet passes `opts.sub` through) | `sub` undefined by default |
| `playerSheet(v, …)` | accept and forward `sub`, `list` (E7 list sheet) | undefined by default |
| `L1(v, opts)` | add `books = BOOKS`, `card = 'long'` options (§5.1) | defaults reproduce today's list and card |
| `recentRow(book)` | add `data-longpress="book" data-title="${esc(book.title)}"` | attributes only |
| `continueCard(v, book)` | add `data-longpress="book" data-title="The Long Book"` on the `.card.cont` | attribute only |
| `T1(v, opts)` | add `storageRow = false`; when true, the Storage section shows the single row of §7.1 instead of the cache block and the library row | false by default (T1b unchanged) |
| `positionFrame` | add: for every `[data-center-cur]` element, scroll so its `.cur` child is centred (same formula as the sheet case); for every `[data-scroll-end="true"]` it already scrolls to the end | no existing element has the attribute |
| `buildPage` | the `newtag` in captions of `isNew` frames | no existing frame has `isNew` |
| `Live` | new state, actions and methods (§8); `back()` gets new steps **before** its existing ones (§8.5) | existing paths unchanged |
| `ICONS` | add the icons of §4.4 | additions only |

### 4.4 New icons (Material Icons, 24 × 24 paths; add to `ICONS`)

```js
delete: 'M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z',
delete_sweep: 'M15 16h4v2h-4zm0-8h7v2h-7zm0 4h6v2h-6zM3 18c0 1.1.9 2 2 2h6c1.1 0 2-.9 2-2V8H3v10zM14 5h-3l-1-1H6L5 5H2v2h12z',
sd_card_alert: 'M18 2h-8L4.02 8 4 20c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-5 15h-2v-2h2v2zm0-4h-2V8h2v5z',
audio_file: 'M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 11h-3v3.75c0 1.24-1.01 2.25-2.25 2.25S8.5 17.99 8.5 16.75s1.01-2.25 2.25-2.25c.46 0 .89.14 1.25.38V11h4v2zm-3-4V3.5L18.5 9H13z',
thermostat: 'M15 13V5c0-1.66-1.34-3-3-3S9 3.34 9 5v8c-1.21.91-2 2.37-2 4 0 2.76 2.24 5 5 5s5-2.24 5-5c0-1.63-.79-3.09-2-4zm-4-8c0-.55.45-1 1-1s1 .45 1 1h-1v1h1v2h-1v1h1v2h-2V5z',
battery_saver: 'M16 4h-2V2h-4v2H8c-.55 0-1 .45-1 1v16c0 .55.45 1 1 1h8c.55 0 1-.45 1-1V5c0-.55-.45-1-1-1zm-1 10h-2v2h-2v-2H9v-2h2v-2h2v2h2v2z',
check_box: 'M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z',
check_box_outline_blank: 'M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z',
open_in_new: 'M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z',
library_music: 'M20 2H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-2 5h-3v5.5c0 1.38-1.12 2.5-2.5 2.5S10 13.88 10 12.5s1.12-2.5 2.5-2.5c.57 0 1.08.19 1.5.51V5h4v2zM4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6z',
```

If a path renders wrongly, say so in the progress note and use the nearest existing icon (`error`, `check_circle`,
`download`, `info`); don't download anything.

### 4.5 Mock data (add after `COPY`; all invented)

```js
/* storage numbers in MB (10^6 bytes, as Android shows them) */
const STORE = {
  free: { plenty: 23400, low: 320 },
  appMB: 45,          // "App & other": APK, native libraries, settings
  sysAppMB: 44,       // Android's "App size"
  cacheCapMB: 1000,
  otherCacheMB: 11,   // voice samples and cover tiles (in the cache, not per book)
  models: [{ id: 'full', name: 'Full quality', mb: 370, inUse: true }, { id: 'small', name: 'Smaller', mb: 126, inUse: false }],
  books: [            // lib = PDF + text; audio = this book's audio cache
    { title: 'The Long Book', lib: 16, audio: 212 },
    { title: 'Designing Calm Software', lib: 3, audio: 58 },
    { title: 'Meeting Notes (no chapters)', lib: 2, audio: 0 },
    { title: 'Quarterly Notes', lib: 5, audio: 0 },
    { title: 'A Field Guide to Small Gardens', lib: 22, audio: 31 },
  ],
  exports: [          // newest first; where: 'app' = in Lectrice, 'shared' = in Audiobooks
    { id: 'x4', title: 'Designing Calm Software', voice: 'Heart', quality: 'Full quality', speed: '1×', state: 'running', pct: 42, mb: 4 },
    { id: 'x3', title: 'A Field Guide to Small Gardens', voice: 'Bella', quality: 'Full quality', speed: '0.75×', state: 'done', where: 'app', mb: 153 },
    { id: 'x2', title: 'A Field Guide to Small Gardens', voice: 'Heart', quality: 'Full quality', speed: '1×', state: 'done', where: 'app', mb: 115 },
    { id: 'x1', title: 'Quarterly Notes', voice: 'Heart', quality: 'Smaller', speed: '1.5×', state: 'done', where: 'shared', mb: 14 },
  ],
};
/* the Continue card when The Long Book is gone (next most recent book with a place; it has no chapters list) */
const CARD_FALLBACK = { title: 'Designing Calm Software', loc: 'p. 7', pct: 34, left: '12 min' };
/* The Long Book: 1× minutes per top-level outline entry (BOOK.outline order: Preface, 1 … 15); sums to 490 = 8 h 10 min */
const CH_MIN_1X = [8, 30, 28, 27, 42, 32, 44, 36, 26, 22, 33, 36, 32, 50, 30, 14];
/* the no-chapters book of E7p (invented, static frame only) */
const PAGE_BOOK = { title: 'Garden Club Minutes (no chapters)', pages: 312, parts: 39, partMin: 10, pagesPerPart: 8 };
```

Derived numbers (Fable computes them with the helpers below; the expected values are given so you can check):

| Number | Value |
|---|---|
| Voice models | 496 MB (370 + 126) |
| Books (library) | 48 MB; audio of books 301 MB |
| Audio cache | 312 MB (301 + 11) of 1 GB → bar 31 % |
| Exports in Lectrice | 272 MB (153 + 115 + 4); the "In Audiobooks" file is **not** counted |
| Total used by Lectrice | 1173 MB → "1.2 GB" (496 + 48 + 312 + 272 + 45) |
| Android's screen | App size 44 MB · User data 817 MB · Cache 312 MB · Total 1.2 GB |

### 4.6 Helpers (add near the view helpers)

```js
const SPEED_X = [0.75, 1, 1.25, 1.5, 1.75, 2];
const BYTES_PER_S = 8000;            // AAC-LC mono 64 kbit/s
const OVERHEAD = 0.03;               // container, chapter box, part joins (check only)
const MARGIN_MIN_MB = 100;
const WHOLE_MIN_1X = 490;            // The Long Book at 1×
const RTF = { full: 120 / 490, small: 180 / 490 };   // so the whole book takes "about 2 h" / "about 3 h"
const UNDO_MS = 8000, SNACK_MS = 4000, ROW_MS = 200, CLEAR_MS = 600;

const fmtMB = (mb) => mb < 1000 ? `${Math.round(mb)} MB` : `${(mb / 1000).toFixed(1).replace(/\.0$/, '')} GB`;
const fmtLen = (min) => { const m = Math.round(min); if (m < 60) return `${m} min`; const h = Math.floor(m / 60), r = m % 60; return r ? `${h} h ${r} min` : `${h} h`; };
const fmtMake = (min) => { if (min < 5) return 'a few minutes'; if (min < 60) return `about ${Math.round(min / 5) * 5} min`; const m = Math.round(min / 10) * 10; return `about ${fmtLen(m)}`; };
const outMin = (min1x, s) => min1x / SPEED_X[s];
const shownMB = (min1x, s) => outMin(min1x, s) * 60 * BYTES_PER_S / 1e6;                 // what the UI shows
const finalMB = (min1x, s) => shownMB(min1x, s) * (1 + OVERHEAD);                        // F
const neededMB = (F) => 2 * F + Math.max(MARGIN_MIN_MB, 0.1 * F);                       // new export
const resumePartsMB = (F, doneShare) => (F - doneShare * F) + F + Math.max(MARGIN_MIN_MB, 0.1 * F);
const resumeAssembleMB = (F) => F + Math.max(MARGIN_MIN_MB, 0.1 * F);
const fitsWhole = (v) => neededMB(finalMB(WHOLE_MIN_1X, v.xSpeed)) <= v.freeMB;
/* range = { from, to } indexes into BOOK.outline (inclusive) */
const rangeMin = (r) => CH_MIN_1X.slice(r.from, r.to + 1).reduce((a, b) => a + b, 0);
const rangeFits = (v, r) => neededMB(finalMB(rangeMin(r), v.xSpeed)) <= v.freeMB;
/* start = the chapter of the saved place (outline index CUR.ch + 1 = 7); extend while it fits; null when the start alone doesn't fit */
function suggestRange(v) { const from = CUR.ch + 1; if (!rangeFits(v, { from, to: from })) return null; let to = from; while (to + 1 < CH_MIN_1X.length && rangeFits(v, { from, to: to + 1 })) to++; return { from, to }; }
const entryLabel = (i) => BOOK.outline[i].num ? `${BOOK.outline[i].num}. ${BOOK.outline[i].title}` : BOOK.outline[i].title;
const shortNum = (i) => BOOK.outline[i].num || BOOK.outline[i].title;      // "7" or "Preface"
function rangeLabel(r) {        // footer line 1 and E4
  if (r.pages) return `Pages ${r.p1}–${r.p2}`;
  const a = BOOK.outline[r.from].num, b = BOOK.outline[r.to].num;
  if (r.from === r.to) return a ? `Chapter ${a}` : shortNum(r.from);
  return a ? `Chapters ${a}–${b}` : `${shortNum(r.from)}–Ch. ${b}`;
}
function rangeSuffix(r) {       // the summary line under the export header
  if (r.pages) return `p. ${r.p1}–${r.p2}`;
  const a = BOOK.outline[r.from].num, b = BOOK.outline[r.to].num;
  if (r.from === r.to) return a ? `Ch. ${a}` : shortNum(r.from);
  return a ? `Ch. ${a}–${b}` : `${shortNum(r.from)}–Ch. ${b}`;
}
```

Expected values at 1×, Full quality, free 320 MB (check them in the progress note):

| Case | Shown | Check |
|---|---|---|
| Whole book | "about 8 h 10 min · about 231 MB" (the existing `EST` strings stay for the whole book) | needed **585 MB** → doesn't fit 320 |
| Suggested range | From 7 (Long Documents) to 12 (Accessibility Is the Baseline): "Chapters 7–12 · about 3 h 5 min · about 89 MB", "Takes about 45 min on this phone" | needed 283 ≤ 320; 7–13 needs 332 → stops at 12 |
| Picked 7–14 | "Chapters 7–14 · about 4 h 25 min · about 127 MB" | needed **362 MB** → doesn't fit |
| Free 120 MB | chapter 7 alone needs 136 → nothing fits (E6n) | "Free up about 465 MB" (585 − 120) |
| E5 at 21 of 49 parts | "Needs about 481 MB free · 120 MB free now" | `resumePartsMB(242.256, 21/49)` |
| E5b | "Needs about 342 MB free · 120 MB free now" | `resumeAssembleMB(242.256)` |
| T3 banner from E6 | "Free up about 265 MB to export this book." | 585 − 320 |

(MB values are `Math.round` of the exact value; the whole book's F is 242.256 MB.)

---

## 5. Library: book sheet, remove confirm, undo (file 21, file 02)

### 5.1 Components

**`bookSheet(book, { loading = false })`** — `scrim() + sheet(…, { auto: true, attrs: 'aria-label="Book"' })`, body
`<div class="bd flush">`:

1. Header `.bsh`: `tile(book.title, 40, 56)` · `.tx`: title `tm ell2` (2 lines), meta `bs fg2 mt4`:
   - audio > 0: "PDF and text {lib} MB · audio {audio} MB"
   - audio = 0: "PDF and text {lib} MB"
   - loading: "…"
2. Divider `i0`.
3. Row (only when audio > 0 or loading): `sheetRow('delete_sweep', title, '', '', 'data-act="clear-book-audio"')` with
   title "Clear this book's audio · {audio} MB" (loading: "Clear this book's audio").
4. Row: `<button class="row danger" data-act="remove-book">${ic('delete', 'lead')}<span class="tx"><span class="t"
   style="display:block">Remove from library</span></span></button>`.

Rows are 56 tall (one line), padding 16 (`.bd.flush .row`). The same composable is T3b (§7.4).

**`removeDialog(book, { playing = false, exportRunning = false, exports = [], ticked = false })`** — its own overlay
wrap (§8.2), `scrim()` + `<div class="dlg" role="dialog" aria-label="Remove ${title}?">`:

- `tm`: "Remove “{title}”?" (curly quotes; wraps freely).
- `b mt16`: "This removes the book, its reading place and its audio from this phone, and frees about {lib + audio} MB.
  You can open the PDF again later."
- If `exportRunning`: `b more` "Its export in progress is cancelled."
- If `playing`: `b more` "Playback stops."
- If `exports.length` (only `where: 'app'` and `state: 'done'` exports of this book): checkbox
  `<button class="cbx" role="checkbox" aria-checked="${ticked}" data-act="toggle-del-exports">${ic(ticked ? 'check_box' :
  'check_box_outline_blank')}<span>…</span></button>` with text "Also delete its exported audiobook ({mb} MB)" or "Also
  delete its {k} exported audiobooks ({sum} MB)". Unticked by default. "In Audiobooks" files are never mentioned.
- `.acts`: `btn('Cancel', 'txt', 'data-act="dlg-cancel"')` · `btn('Remove', 'txt errc', 'data-act="remove-confirm"')`.

**`undoSnack(title)`** — `<div class="snack" role="status"><span class="sp1">Removed “{title}”.</span><button class="act"
data-act="undo-remove">Undo</button></div>` (position: existing `.snack`, 112 from the bottom, above the FAB).

**`L1` options** (hook in §4.3): `books` = the Recent books (default `BOOKS.slice(1)`), `card` = `'long'` (today's card),
`'fallback'` (CARD_FALLBACK: caption "Continue listening", title, location line "p. 7", progress 34 + "34 % · 12 min
left", play button `aria-label="Play Designing Calm Software from page 7"`) or `null` (no card; Recent starts under the
header). When `books` is empty and `card` is null, render `L2()` instead.

### 5.2 Frames (gallery `library`)

| ID | Name | Build | Caption note |
|---|---|---|---|
| L7 | Book sheet | `L1(v) + bookSheet(STORE.books[4])` (A Field Guide to Small Gardens) | Long-press a row or the Continue card. TalkBack: "Remove from library" custom action on every row. |
| L7l | Book sheet · sizes loading | `L1(v) + bookSheet(STORE.books[4], { loading: true })` | Sizes show "…" until the core answers (≤ 300 ms). |
| L7c | Remove · playing book | `L1(v) + bookSheet(STORE.books[0]) + removeDialog(STORE.books[0], { playing: true })` (dialog over the sheet) | "Playback stops." shows only when this book is playing; frees about 228 MB. |
| L7ce | Remove · with exports | `L1(v) + bookSheet(STORE.books[4]) + removeDialog(STORE.books[4], { exports: [x3, x2] })` | Exported audiobooks are kept unless ticked; files in Audiobooks are never touched. |
| L7cx | Remove · export running | `L1(v) + bookSheet(STORE.books[1]) + removeDialog(STORE.books[1], { exportRunning: true })` | Its unfinished export is paused now and cancelled when the undo time is over. |
| L7u | Removed · Undo | `L1(v, { books: BOOKS.slice(1, 4) }) + undoSnack('A Field Guide to Small Gardens')` | The row is gone at once; Undo for 8 s brings it back with its place. |
| L7z | Last book removed | `L2() + undoSnack('The Long Book')` | Removing the last book shows the empty library under the snackbar. |

Notes: in L7c, L7ce, L7cx the dialog sits above the sheet's scrim with its own scrim (two scrims stack, as on Android).
Dark/black: no change beyond tokens.

---

## 6. Export: space check, part of the book, new paused states (file 10)

All new export frames use `exportFrame({ m4bOnly: true, … }, opts)`, so they have no Format group and the summary reads
"Heart · Full quality · 1×".

### 6.1 E6 footer (`e6Footer(v)`)

Replaces `.xfoot`'s content when `!fitsWhole(v)`; same container (`<div class="xfoot">`, sticky, `bg2`, top line):

1. `<div data-est aria-live="polite">` with line 1 `lb16`: the whole book "about 8 h 10 min · about 231 MB" (from `EST`,
   as today).
2. `.warncard` (`role="status"`): `ic('sd_card_alert')` + `<div>`:
   - `.t` "Not enough space for the whole book."
   - `.s` "It needs about {fmtMB(needed)} free. The phone has {fmtMB(free)}."
   - **Nothing fits** (`suggestRange(v) === null`): a second `.s` "There isn't room for even one chapter. Free up about
     {fmtMB(needed − free)} and try again."
3. Buttons, `margin-top:8px`:
   - Something fits: `btn('Export part of the book', '', 'data-act="part-page"')` (56, primary), then centred
     `btn('Free up space', 'txt', 'data-act="free-up"')`.
   - Nothing fits: only `btn('Free up space', '', 'data-act="free-up"')` (primary).

Measure at 360 × 800: the footer must not cover the page header; the body above it still scrolls. (Expected footer height
≈ 290 at 412; if at 360 the body's visible part is under 200 px, report it, don't change the layout.)

### 6.2 E7 · Export part of the book (`partPage(v, { range, list = null })`)

A second pushed page in the player sheet, over E1 (same `.page` mechanics, slides in from the right). Structure as
`exportPage`'s E1: header + `.bd xbd` + footer.

- Header `.shh`: `ib('arrow_back', 'Back', 'data-act="part-back"')` + `tm` "Export part of the book".
- Under the header (`px16`): `bs fg2` the summary "Heart · Full quality · 1×" (`exportSummary(v)` without range), then
  `bs fg2 mt4` "{fmtMB(free)} free on this phone. The whole book needs about {fmtMB(neededWhole)}."
- Body (`.bd xbd`, `padding-top:16px`): two `.xrange` buttons:
  - `data-act="range-list" data-end="from"`: `.tx` = `cap fg2` "From" + `b w5 ell` value; trailing `ic('expand_more')`.
  - `data-act="range-list" data-end="to"`: "To" + value.
  - Values: chapter mode `entryLabel(i)` ("7. Long Documents", "12. Accessibility Is the Baseline"); page mode "p. 97".
  - `aria-label`: "From chapter 7, Long Documents. Change" / "To chapter 12, … Change".
- Footer `.xfoot` (`exportFooter` look):
  - Fits: line 1 `lb16` "{rangeLabel} · about {fmtLen(outMin)} · about {fmtMB(shownMB)}"; line 2 `bs fg2` "Takes
    {fmtMake(min1x × RTF[q])} on this phone"; button "Start export ({fmtMake(…)})" `data-act="start-part"`.
  - Doesn't fit: line 2 becomes `bs warnline` "Doesn't fit: needs about {fmtMB(needed)}, {fmtMB(free)} free"; the
    button is `disabled` (same label); under it a centred `btn('Pick what fits', 'txt', 'data-act="pick-fits"')`.
- No "Beginning" row in The Long Book: its first outline entry is the Preface (an entry), so there is no front matter
  before it. (Spec: the row exists only when playable text comes before the first entry.)

**List sheet** (`rangeList(v, end, range)`) inside the page: `<div class="inner">${scrim()}${sheet(…, { auto: true,
attrs: 'aria-label="From"' })}</div>`:

- Header `.shh`: `tm` "From" / "To" + `ib('close', 'Close', 'data-act="list-close"')`.
- `<div class="bd flush" data-center-cur>`: one row per top-level outline entry (16 rows): `<button class="row two
  ${sel ? 'sel cur' : ''}" role="radio" aria-checked="${sel}" data-act="range-pick" data-i="${i}">` with `.tx`: `.t ell2`
  `entryLabel(i)`, `.m` "about {fmtLen(outMin(CH_MIN_1X[i], s))} · about {fmtMB(shownMB(CH_MIN_1X[i], s))}"; trailing
  `ic('check')` on the selected row only. Selected = the current From (or To) entry.
- Picking: a From after the To moves the To to the same entry (and vice versa); the list closes; the footer recomputes.
- Page mode (E7p) is static only; it has no list frame.

### 6.3 New export bodies (add to `exportBody`'s `bodies`)

Reuse `running(ban, buttons)` exactly as E3/E5 do. Banners are `banner(kind, icon, text)`.

| Key | Banner | Progress area | Buttons |
|---|---|---|---|
| E2p | none | `running` with `pct` 42 and the parts line "Part 8 of 19 · about 25 min left" | as E2 |
| E3h | `warn`, `thermostat`, "Paused while the phone is hot. It continues when it cools down." | as E3 (42 %) | `btn('Continue now')` only (no Samsung battery link) |
| E3p | `warn`, `battery_saver`, "Paused to save battery. It continues while charging." | as E3 | `btn('Continue now')` only |
| E5n | `err`, `error`, the E5 text (live: "{k} of 49 parts are kept…" with k from the paused %) | 42 % | under the banner, `bs mt8` (colour `fg`) "Needs about {needed} free · {free} free now"; then buttons `btn('Continue', '', 'disabled')` while free < needed (enabled otherwise) · centred `btn('Free up space', 'txt', 'data-act="free-up"')` · centred `btn('Cancel export', 'txt', 'data-act="page-back"')` |
| E5b | `err`, `error`, "All 49 parts are made, but there isn't room to put them together. Free up about {fmtMB(resumeAssembleMB(F))} and continue." | bar 98 %, label "Exporting · 98 %", parts line "All 49 parts made · putting them together" | as E5n, with "Needs about 342 MB free · 120 MB free now" |
| E4s | — | as E4, but the primary is `<button class="btn" disabled>${ic('check')}Saved to Audiobooks</button>`; the caption "Audiobook players will find it there" stays | Save as… · Share as E4; then centred `btn('Make another version', 'txt', 'data-act="another-version"')`; plus `<div class="snack over" role="status"><span class="sp1">Saved to Audiobooks/Lectrice.</span></div>` |
| E4p | — | as E4 with the summary "Chapters 7–12 · 3 h 5 min · 89 MB · M4B" | as E4 + "Make another version" |

The needed/free line is `bs` `fg` (not muted: it's the actionable number), `margin-top:8px`, directly under the banner.

### 6.4 Frames (gallery `export`)

All `exportFrame({ m4bOnly: true, …over }, opts)`; `free` in MB.

| ID | Name | Over / opts | Caption note |
|---|---|---|---|
| E6 | Not enough space | `{ freeMB: 320 }` | The whole book needs about 585 MB; the phone has 320 MB. Export part of it, or free up space. |
| E6n | Nothing fits | `{ freeMB: 120 }` | Not even one chapter fits: only "Free up space". |
| E7 | Export part of the book | `{ freeMB: 320 }`, `{ sub: 'part', range: { from: 7, to: 12 } }` | The range that fits is already picked, from the chapter you're in. |
| E7f | From list | as E7 + `list: 'from'` | Each chapter with its length and size at the chosen speed. |
| E7t | To list | as E7 + `list: 'to'` | Picking a To before the From moves the From too. |
| E7x | Range doesn't fit | `{ freeMB: 320 }`, `{ sub: 'part', range: { from: 7, to: 14 } }` | Start is disabled; "Pick what fits" restores the suggestion. |
| E7p | No chapters | `{ freeMB: 320 }`, `{ sub: 'part', range: { pages: true, p1: 97, p2: 272, fromLabel: 'p. 97', toLabel: 'p. 265', min1x: 220, neededWhole: 486 } }` | A book without chapters picks pages, in steps of about 10 minutes. (Book: "Garden Club Minutes (no chapters)", 312 pages.) |
| E2p | Running · part | `{ xRange: { from: 7, to: 12 } }`, `{ exportState: 'E2p' }` | The summary line shows the range. |
| E4p | Done · part | `{ xRange: { from: 7, to: 12 } }`, `{ exportState: 'E4p' }` | Chapter marks only for chapters 7–12, timed from the start of the file. |
| E3h | Paused · hot | `{}`, `{ exportState: 'E3h' }` | Pauses when the phone is critically hot; continues after it cools down. |
| E3p | Paused · battery | `{}`, `{ exportState: 'E3p' }` | Pauses on battery saver or at 15 %; continues while charging. |
| E5n | Out of space · parts kept | `{ freeMB: 120 }`, `{ exportState: 'E5n' }` | Continue is enabled once there's room; it also continues by itself. |
| E5b | Out of space · assembling | `{ freeMB: 120 }`, `{ exportState: 'E5b' }` | All parts are kept; only putting them together waits for room. |
| E4s | Saved to Audiobooks | `{}`, `{ exportState: 'E4s' }` | The file moved to Audiobooks/Lectrice; it's no longer counted in Lectrice's storage. |

E7p's page values: summary "Heart · Full quality · 1×"; free line "320 MB free on this phone. The whole book needs about
486 MB."; rows "From" "p. 97" / "To" "p. 265"; footer "Pages 97–272 · about 3 h 40 min · about 106 MB" / "Takes about
55 min on this phone" / "Start export (about 55 min)". (Static; computed from 22 parts × 10 min.)

---

## 7. Storage (files 20, 11) and Android's own page

### 7.1 Settings row (T1s)

`T1(v, { scrolled: true, storageRow: true })`: in the Storage section (after `.sechead` "Storage") one row
`settingsRow('Storage', '1.2 GB used · 23.4 GB free', ic('chevron_right'), 'data-act="t3"')` (free from `v.freeMB`;
loading "…"). Nothing else in that section.

### 7.2 Storage screen (`T3(v, store, { from = 'settings', banner = null, loading = false, empty = false })`)

`<div class="scroll" data-scroll>`; everything below in order. All rows are existing classes.

1. Top bar `.tbar`: `ib('arrow_back', 'Back', 'data-act="back"')` (from `'system'`: `ib('close', 'Close',
   'data-act="t3-close"')`) + `tl` "Storage".
2. Banner (only when `banner`), `px16`, margin-bottom 16:
   - Short: `banner('warn', 'sd_card_alert', 'Free up about {x} to export this book.', btn('Open phone storage', 'txt',
     'data-act="phone-storage"'))`.
   - Room now: `.okcard` with `ic('check_circle')`, `.tx` "There's room now to export this book." (E5 context: "There's
     room now. The export continues."), `.acts` with `btn('Back to the export', 'txt', 'data-act="back"')`.
3. Summary `.card.sumcard` (`bg2`, 1 px line, radius 12, padding 16):
   - `tm` "{fmtMB(total)} used by Lectrice" (loading: "… used by Lectrice")
   - `bs fg2 mt4` "{fmtMB(free)} free on this phone" (loading "…")
   - `.stack` (`aria-hidden="true"`) with five `<span class="kN" style="flex:${mb} 0 0">` in this order: Voice models
     (k0), Books (k1), Audio cache (k2), Exports (k3), App & other (k4); a zero part is left out. Loading: empty track.
   - `.legend` (`aria-hidden="true"`): `<span><i class="kN"></i>Voice models 496 MB</span>` × 5 (zero parts left out).
4. `.sechead` "Voice models"; one `settingsRow` per model: "Full quality" / "370 MB · in use", "Smaller" / "126 MB";
   chevron; `data-act="t2"`. Empty: `settingsRow('No voice installed', '', btn('Download', 'txt'))`.
5. `.sechead` "Books"; `.shead` "{k} books · {lib} MB, plus {audio} MB of audio" (audio 0: "{k} books · {lib} MB");
   then rows **largest total first** (The Long Book 228, Designing Calm Software 61, A Field Guide to Small Gardens 53,
   Quarterly Notes 5, Meeting Notes 2), `divider('i72')` between: `<button class="lrow" data-act="t3-book"
   data-title="…">` with `tile(title, 40, 56)` · `.tx` (`.t ell` title, `.m ell` the sheet's meta line) · `.sz`
   "{lib + audio} MB". Empty: `<div class="b fg2 px16" style="padding-bottom:8px">No books yet.</div>`.
6. `.sechead` "Audio cache"; `px16`: `progress(round(cache / cap × 100), '', 'margin-top:8px')`, `bs fg2 mt4` "{cache} MB
   of 1 GB", `<div class="mt4" style="margin-left:-12px">${btn('Clear audio cache', 'txt', 'data-act="clear-cache"')}</div>`
   (`disabled` at 0 MB; while clearing: the button shows `spinner()` before its label and is disabled), `bs fg2` "Audio is
   made again when it's needed. Android may also clear it when the phone is low on space."
7. `.sechead` "Exported audiobooks" (whole section hidden when there are none); one `sheetRow`-like row per export,
   newest first, with `data-act="t3-export" data-id="…"`, lead `audio_file`, title = book title (1 line, ellipsis), meta:
   - done in Lectrice: "{voice} · {speed} · {mb} MB · In Lectrice"
   - done in Audiobooks: "{voice} · {speed} · {mb} MB · In Audiobooks"
   - unfinished: "Exporting · {pct} % · {mb} MB so far" + `progress(pct, '', 'width:120px;margin-top:4px')` (as R4x)
   Use `sheetRow(icon, title, meta, '', attrs, extra)` (outside a sheet `.row` already has `padding:8px 16px`); a
   `divider()` (inset 16) between rows, as in Settings.
8. `.sechead` "Phone"; `settingsRow('Free up space on the phone', '', ic('chevron_right'), 'data-act="phone-storage"')`;
   then `<button class="row" data-act="delete-all"><span class="tx"><span class="t errc" style="display:block">Delete all
   Lectrice data</span></span></button>`.
9. Spacer 48.

Loading (`loading: true`): every number shows "…" (summary, model rows "…", the books header "…", each book meta "…" and
no `.sz`, cache line "… of 1 GB" with an empty bar, export metas "…"). Empty (`empty: true`): models section "No voice
installed" + Download, books "No books yet.", cache "0 MB of 1 GB" with Clear disabled, exports hidden; summary "45 MB
used by Lectrice" and a one-segment bar (App & other).

### 7.3 Sheets and dialogs

- **T3b book sheet** = `bookSheet(book)` over T3 (same component as L7).
- **T3c export-file sheet** (`exportFileSheet(x)`), `sheet(…, { auto: true, attrs: 'aria-label="Exported audiobook"' })`,
  `.bd flush`:
  - Header `.bsh`: `ic('audio_file')` · `.tx`: `tm ell2` book title; `bs fg2 mt4` "{voice} · {quality} · {speed} · {mb}
    MB" (unfinished: "{voice} · {quality} · {speed}"); `cap fg2 mt4` "In Lectrice" / "In Audiobooks/Lectrice" /
    unfinished "Exporting · 42 % · 4 MB so far" + a 120 × 4 bar.
  - Divider `i0`, then rows:
    - Done, in Lectrice: `open_in_new` "Open" · `ios_share` "Share" · `library_music` "Save to Audiobooks" / sub "Moves
      the file to Audiobooks/Lectrice" · `delete` "Delete" (danger).
    - Done, in Audiobooks: Open · Share · Delete (danger).
    - Unfinished: `download` "Go to the export" with trailing `chevron_right`; then `bs fg2` (`padding:8px 16px 0`) "To
      delete it, cancel the export on its page."
  - `data-act`s: `x-open`, `x-share`, `x-save`, `x-delete`, `x-goto`.
- **T3cd delete dialog**: `dialog('Delete this audiobook?', 'This deletes the file from your phone ({mb} MB).',
  btn('Cancel', 'txt', 'data-act="dlg-cancel"') + btn('Delete', 'txt errc', 'data-act="x-delete-confirm"'))` in its own
  overlay wrap (§8.2).
- **T3d delete-all dialog**: `dialog('Delete all Lectrice data?', 'This deletes your books, reading places, voice models,
  audio and exports kept in Lectrice. Audiobooks you saved to Audiobooks stay. The app closes.', btn('Cancel', 'txt',
  'data-act="dlg-cancel"') + btn('Delete all', 'txt errc', 'data-act="delete-all-confirm"'))`.
- **Snackbars on T3** (existing `.snack`, but `bottom:40px` since there's no pill or FAB here: add the class `over`):
  "Cleared 312 MB." (cache), "Cleared 212 MB of audio." (one book), "Saved to Audiobooks/Lectrice.", the remove undo
  snackbar.

### 7.4 Android's own Settings page (X1, X1c): a mock of a system screen

`X1(v, { cacheCleared = false })`:

1. `.tbar`: `ib('arrow_back', 'Back', 'data-act="x1-back"')` + `tl` "Storage".
2. `.sysapp`: `<div class="mark">L</div>` (64), `tm mt12` "Lectrice", then `mt8` `<span class="badge">Android's screen,
   not Lectrice's</span>` (the existing mockup-annotation badge).
3. `.sysacts`: two buttons `.sysact`: `ic('delete')` + "Clear storage" (`data-act="sys-clear-storage"`), and
   `ic('delete_sweep')` + "Clear cache" (`data-act="sys-clear-cache"`; `aria-disabled="true"` when the cache is 0).
4. `.sechead` "Space used"; four non-interactive two-line rows (`settingsRow(title, value, '')`): "App size" / "44 MB",
   "User data" / "817 MB", "Cache" / "312 MB" (X1c: "0 B"), "Total" / "1.2 GB" (X1c: "861 MB").

Frames: **X1** "Android Settings · App storage" (caption: "Android's own screen (AOSP labels; Samsung's wording
UNVERIFIED). "Cache" is Lectrice's audio cache, covers and voice samples. Lectrice declares manageSpaceActivity, so
"Clear storage" opens Lectrice's Storage screen (T3s) instead of wiping everything."), **X1c** "After Clear cache" (cache
0 B, Clear cache disabled; caption "Nothing else is lost; audio is made again when needed.").

### 7.5 Frames (gallery `settings`)

| ID | Name | Build | Caption note |
|---|---|---|---|
| T1s | Settings · Storage row | §7.1, scrolled to the end (`{ scroll: 'end' }`) | One "Storage" row replaces T1b's three storage rows. |
| T3 | Storage | `T3(v, STORE)` | Everything Lectrice keeps, largest books first. |
| T3e | Storage · end | `T3(v, STORE)`, `{ scroll: 'end' }` (the `.scroll` gets `data-scroll-end="true"`) | Exported audiobooks, the phone's storage page, and the full reset. |
| T3l | Storage · loading | `T3(v, STORE, { loading: true })` | Numbers show "…" until they're measured (≤ 300 ms). |
| T3z | Storage · empty | `T3(v, STORE, { empty: true })` | No books, no exports, no voice. |
| T3g | Storage · 200 % font | `T3(v, STORE)`, `{ x2: true }` | Rows wrap; the bar and legend stay readable. |
| T3b | Book sheet | `T3(v, STORE) + bookSheet(STORE.books[0])` | Same sheet as the Library's long-press. |
| T3c | Export file · in Lectrice | `T3(v, STORE) + exportFileSheet(x3)` | Save to Audiobooks moves the file; Lectrice's total drops by its size. |
| T3ca | Export file · in Audiobooks | `T3(v, STORE) + exportFileSheet(x1)` | Lectrice can still open, share and delete what it saved. |
| T3cu | Export file · unfinished | `T3(v, STORE) + exportFileSheet(x4)` | An unfinished export is cancelled on its own page. |
| T3cd | Delete an audiobook | `T3(v, STORE) + exportFileSheet(x3) + deleteDialog(x3)` | Asks first; frees 153 MB. |
| T3d | Delete all data | `T3(v, STORE) + deleteAllDialog()` (T3 scrolled to the end) | The real reset; saved audiobooks stay. |
| T3k | Cache cleared | `T3(v, { …STORE, cache 0 }) + snack over "Cleared 312 MB."` | No confirm: audio is made again when needed. |
| T3f | Free up (from an export) | `T3(v, { …STORE }, { banner: { short: 265 } })` with `freeMB: 320` | Opened from the export's "Free up space": the banner says how much. |
| T3r | Room now | `T3(v, STORE minus x2 and x3, { banner: { ok: true } })`, `freeMB: 588` | After deleting two exports there's room; back to the export. |
| T3s | Opened from Android | `T3(v, STORE, { from: 'system' })` | Opened by Android's "Clear storage": ✕ returns to Android's Settings. |
| X1 | Android Settings · App storage | §7.4 | (caption in §7.4) |
| X1c | Android · after Clear cache | §7.4 | (caption in §7.4) |

---

## 8. The live phone

### 8.1 New state (in `Live`)

```js
this.store = structuredClone(STORE);           // mutable copy
this.v.freeMB = STORE.free.plenty;
this.s.removed = null;   // { book, index, wasCard, timer } during the undo window
this.s.purged = [];      // titles removed for good this session
this.s.dialog = null;    // 'remove' | 'x-delete' | 'delete-all'
this.s.ticked = false;   // remove dialog checkbox
this.s.parked = null;    // the player-sheet overlay hidden while Storage is open from an export
this.s.request = null;   // { kind: 'start' | 'resume' } when Storage was opened from an export
this.s.range = null;     // E7 range
this.s.list = null;      // 'from' | 'to' while the E7 list is open
```

### 8.2 Overlay plumbing

- Sheets of this pass (book sheet, export-file sheet) use the existing `openOverlay(kind)` with new `overlayHtml` cases
  `'book'` and `'export-file'` (the target book/export kept in `this.s.target`).
- **Dialogs** go in their own wrap so they stack above an open sheet: `<div data-overlay="dialog" style="position:absolute;
  inset:0;z-index:24">${scrim()}${dialog…}</div>` appended to the screen. `topOverlay()` already returns the last wrap.
- **Snackbars** (`showSnack(html, ms)`): remove any current `.snack`, append the new one to the top layer (screen), fade
  it out (`.out`) after `ms` (4 s; 8 s with an action), then remove. One at a time.

### 8.3 Interactions to add (exact)

1. **Long-press** (Library rows and the Continue card, `[data-longpress]`): in `onPointerDown`, before the existing
   checks; a timer of `LONG_PRESS_MS` (500); cancelled if the pointer moves more than `TAP_SLOP` (8) or lifts; when it
   fires: `this.suppressClick = true`, open the `'book'` sheet for that title. Also on `contextmenu` (right-click,
   mockup convenience): `preventDefault()` and open the sheet.
2. **Book sheet** (from Library or T3):
   - "Clear this book's audio" → close the sheet; set that book's `audio` to 0 (and lower the cache); snackbar "Cleared
     {n} MB of audio." (4 s). The playing state doesn't change.
   - "Remove from library" → remove dialog with: `playing` = this is The Long Book and `v.playing`; `exportRunning` = a
     `running` export of that title exists; `exports` = its done in-Lectrice exports; `ticked` false.
   - Checkbox toggles (`aria-checked`, icon).
   - Cancel → only the dialog closes; the sheet stays.
   - Remove → if another book is in its undo window, purge it at once (drop its row data and, if it was ticked, its
     exports). Close dialog and sheet; if The Long Book was playing: `v.playing = false`, `stopTimer()`. Hide the row:
     set its `height` to its current height, then to 0 with `.gone` (200 ms), then remove; if it was the Continue card's
     book, re-render the Library with `card: 'fallback'` (the fallback book drops out of Recent). Remove it from T3's
     data too (Books, totals; a `running` export of it is marked paused). Show the undo snackbar (8 s). If it was the last
     book, render `L2()` under the snackbar.
   - **Undo** → re-render the Library/T3 with the book back at its place and the card restored; numbers back; snackbar
     goes. No playback restart.
   - Timeout (8 s) → purge: the book stays gone; if the box was ticked its done in-Lectrice exports go too (free rises by
     their size); its running export is dropped.
3. **Settings → Storage**: the live `T1` is rendered with `storageRow: true`. The row (`data-act="t3"`) pushes the T3
   layer (same push animation as Settings). Inside T3:
   - Model rows → the existing T2 (back returns to T3).
   - Book rows → book sheet (as 2).
   - "Clear audio cache" → spinner in the button for `CLEAR_MS` (600), then cache 0, every book's audio 0, the books
     header loses "plus … of audio", bar/legend update; snackbar "Cleared 312 MB." The button turns disabled.
   - Export rows → export-file sheet. "Open" / "Share" → close the sheet + snackbar "(Mockup: Android's app chooser opens
     here.)". "Save to Audiobooks" → close + the row's meta switches to "In Audiobooks", Lectrice's total drops by its
     size (free unchanged) + snackbar "Saved to Audiobooks/Lectrice.". "Delete" → dialog → Delete: the row goes (200 ms),
     totals drop; **free rises by its size** (both places); if the section becomes empty it disappears. "Go to the
     export" → close + snackbar "(Mockup: opens Designing Calm Software at its export page.)".
   - "Free up space on the phone" / "Open phone storage" → snackbar "(Mockup: Android's storage page opens here.)".
   - "Delete all Lectrice data" → dialog → "Delete all" → close + snackbar "(Mockup: Lectrice deletes everything and
     closes; the next start shows the welcome screen.)" (nothing is changed in the mock).
   - Every change re-renders T3 in place keeping its scroll position, and re-checks the banner (§8.4).
4. **Phone space toggle** (page chrome): sets `v.freeMB` to 23400 or 320 and re-renders whatever is visible (export page
   footer, T3 summary, the T1 row). If an export is running (E2) when "Nearly full" is chosen: pause it at its current %
   → `exportState = 'E5n'` with k = round(pct / 100 × 49) in the text and needed = `resumePartsMB(F, k / 49)`; the player
   row shows "Exporting · {pct} %" frozen.
5. **Export page with low space** (E1 → E6 → E7):
   - E1's footer shows E6 whenever `!fitsWhole(v)` (any speed: at 2× the whole book still needs 342 MB).
   - "Export part of the book" → push the E7 page with `range = suggestRange(v)`.
   - From / To rows → the list sheet inside the page; picking sets the end (with the "move the other end" rule), closes
     the list, recomputes the footer (cross-fade as `changeExport` does).
   - "Pick what fits" → `range = suggestRange(v)`.
   - "Start export" (E7) → pop E7, set `v.xRange = range`, run the existing `startExport()` (E2 shows the summary with "·
     Ch. 7–12"), ending at **E4p** with "Chapters 7–12 · 3 h 5 min · 89 MB · M4B".
   - "← Back" on E7 → pops to E1, keeping the choices.
   - "Free up space" (E6 or E5n) → hide (park) the player-sheet overlay, open T3 with `request = { kind: 'start' }` (E6) or
     `'resume'` (E5n) and the banner "Free up about {fmtMB(needed − free)} to export this book.".
6. **Banner rule (§8.4)**: after every change on T3 with a request: if `free ≥ needed`, the banner becomes the room-now
   card; for `'resume'` the export also continues at once (state E2, timer resumes from the paused %). Back (←, ◁, or
   "Back to the export") closes T3, un-parks the sheet and re-renders the export page (E1 footer normal again, or E2).
7. **E4**: "Save to Audiobooks" → the page re-renders as E4s (button disabled "Saved to Audiobooks", snackbar over the
   sheet, 4 s). When the live export finishes, add it to `store.exports` (title "The Long Book", the chosen voice and
   speed, `mb` from the summary: 231 for the whole book at 1×, 89 for 7–12; `where: 'app'`); saving switches it to
   `'shared'`. "Make another version" → E1 with the same choices and `xRange = null`.
8. **Android Settings ›** (page chrome): close overlays; push the X1 layer over whatever is shown. "Clear cache" →
   cache 0 (same effect as T3's clear), X1 re-renders as X1c. "Clear storage" → push T3 with `from: 'system'` (✕). ✕ or
   ◁ Back → back to X1. X1's ← or ◁ Back → closes X1 (back to where the user was).

### 8.4 Numbers in the live phone

All T3 numbers are computed from `this.store` (models + books lib + cache + in-Lectrice exports + app) every render; the
T1 row and X1 use the same totals (X1's User data = total − 44 − cache). "Free" = `v.freeMB`, which only changes with the
toggle, deleting exports, and purging a removed book's library bytes (cache bytes don't count: §2.3).

### 8.5 Back order (◁ Back and the in-app ←)

Topmost first: scrubber → **dialog** → **E7 list sheet** → **E7 page** → export page → **book / export-file sheet** → any
sheet → Listen view → **T3 opened from an export (un-park the sheet)** → **T3 (to T1, or to X1 when `from: 'system'`)** →
**X1** → T2 → Settings → Reader → Library. The new steps are inserted at those places in `back()`; the rest is as today.

### 8.6 "How to try" (append)

8. Long-press "A Field Guide to Small Gardens" (or right-click it) → Remove from library → Remove; then tap Undo.
9. Library ⚙ → Storage: tap a book, an exported audiobook, and "Clear audio cache".
10. Choose "Phone space: Nearly full", then play → ⋯ → Export audiobook: see "Not enough space", tap "Export part of the
    book", change "To", and start it.
11. On "Not enough space", tap "Free up space" and delete the two Field Guide audiobooks until the banner says there's
    room; go back.
12. Start a whole-book export with "Plenty", then switch to "Nearly full" while it runs (out of space, parts kept).
13. Tap "Android Settings ›" → "Clear storage" opens Lectrice's Storage screen.

---

## 9. Checklist and screenshots

- [ ] Only `design/2026-10-23-android-quiet-v2.html` edited (plus the progress note and screenshots); no Artifact, no
      git, nothing downloaded; it still opens from disk with no network.
- [ ] Every existing frame and the existing live-phone flows are unchanged (§3 re-take check reported).
- [ ] Every new ID of §5.2, §6.4, §7.5 exists, in the right gallery and order, with its caption and the "New" tag; solo
      mode works for each (`?opt=3&theme=…&w=…&screen=ID`).
- [ ] Copy verbatim from §5–§7 (curly quotes and apostrophes as in the mockup's `COPY`); numbers match the expected
      values of §4.6 and the table in §4.5.
- [ ] New frames are M4B-only (no Format group; summaries without "M4B" except E4/E4p/E4s).
- [ ] No visible scrollbar anywhere (T3, sheets, E7 list); everything still scrolls (wheel, drag, keyboard).
- [ ] Light, dark, black: all colours from tokens; no text muted with `opacity`; dialogs over sheets show both scrims.
- [ ] 360 × 800: L7 sheet, L7c dialogs (long titles wrap, buttons on one line), T3 (legend wraps, book rows ellipsis,
      `.sz` never clipped), E6 footer, E7 rows (values ellipsis) fit with no overlap or clipping.
- [ ] 200 % font (T3g) has no overlap.
- [ ] 48 dp targets; `aria-label`s on icon-only controls; the checkbox is `role="checkbox"`; list rows `role="radio"`;
      snackbars `role="status"`; `:focus-visible` works on every new control.
- [ ] Live: long-press (and right-click) opens the sheet and doesn't also open the book; Undo works within 8 s and the
      purge after; T3 actions update numbers in place; the low-space flows of §8.3 items 4–6 work; X1 path works; ◁ Back
      follows §8.5.
- [ ] `prefers-reduced-motion`: row collapse, page pushes, sheets and snackbar fades are instant.
- [ ] No text from the user's books; only the mockup's titles and "Garden Club Minutes (no chapters)".
- [ ] Self-review: no unused CSS/JS, no scaffolding.

**Screenshots** (device pixel ratio 2, solo mode, network blocked, same command as the v2 progress note; 412 → window
432 × 935, 360 → 380 × 820), new files in `design/screens/`:

- `android-q2-L7-light.png`, `android-q2-L7-black.png`, `android-q2-L7-light-360.png`, `android-q2-L7c-light.png`,
  `android-q2-L7c-dark.png`, `android-q2-L7ce-light.png`, `android-q2-L7ce-light-360.png`, `android-q2-L7cx-light.png`,
  `android-q2-L7u-light.png`, `android-q2-L7u-black.png`, `android-q2-L7z-light.png`
- `android-q2-T1s-light.png`, `android-q2-T3-light.png`, `android-q2-T3-dark.png`, `android-q2-T3-black.png`,
  `android-q2-T3-light-360.png`, `android-q2-T3e-light.png`, `android-q2-T3l-light.png`, `android-q2-T3z-light.png`,
  `android-q2-T3g-light.png`, `android-q2-T3b-light.png`, `android-q2-T3c-light.png`, `android-q2-T3ca-dark.png`,
  `android-q2-T3cu-light.png`, `android-q2-T3cd-light.png`, `android-q2-T3d-black.png`, `android-q2-T3k-light.png`,
  `android-q2-T3f-light.png`, `android-q2-T3r-light.png`, `android-q2-T3s-light.png`, `android-q2-X1-light.png`,
  `android-q2-X1-dark.png`, `android-q2-X1c-light.png`
- `android-q2-E6-light.png`, `android-q2-E6-black.png`, `android-q2-E6-light-360.png`, `android-q2-E6n-light.png`,
  `android-q2-E7-light.png`, `android-q2-E7-light-360.png`, `android-q2-E7t-light.png`, `android-q2-E7x-light.png`,
  `android-q2-E7p-light.png`, `android-q2-E2p-light.png`, `android-q2-E4p-light.png`, `android-q2-E3h-light.png`,
  `android-q2-E3p-dark.png`, `android-q2-E5n-light.png`, `android-q2-E5b-light.png`, `android-q2-E4s-light.png`

Progress note `docs/progress/android-new-screens.md`: what was built, the §3 re-take comparison, the §4.6 number checks,
anything ambiguous you resolved (most literal reading, listed), the screenshot list, and the `xdg-open` line for the
mockup.

---

## 10. Questions for the user

1. **Old export frames still show MP3.** The existing E1/E1x/E2–E5, N3 and the live phone still have the Format (M4B |
   MP3) group from before "M4B only". Clean them up in a small follow-up pass (drop the group, the badge and the MP3
   impact bullet)?
2. **E5b wording.** The spec says "Free up about {needed}" (342 MB) while the phone has 120 MB, so the user really needs
   to free 222 MB. Change the copy to the amount still missing ("Free up about 222 MB and continue")?
3. **"There's room now" card** on the Storage screen after freeing enough for an export: keep it (my addition), or just
   hide the banner?
