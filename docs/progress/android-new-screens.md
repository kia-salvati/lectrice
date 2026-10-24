# Android new screens (storage, remove from library, export space): progress note (Fable, 2026-10-24)

Built from `docs/2026-10-24-android-new-screens-brief.md` §3–§9 plus the orchestrator's three decisions (M4B only
everywhere; "Free up about …" = the amount still missing; keep the "There's room now" card). Only
`design/2026-10-23-android-quiet-v2.html`, `design/screens/android-q2-*.png` and this note were touched; no git
commands, no Artifact, nothing downloaded (network blocked in every headless run).

## Open it

```
xdg-open /home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html
```

- Under the live phone: `◁ Back`, **Phone space: Plenty | Nearly full**, **Android Settings ›** (page chrome).
- "How to try" items 8–13 walk through the new flows. The "New on 24 October" box sits under the live phone;
  the "Questions (24 October)" list at the bottom shows the three questions with their answers.
- Solo mode works for every new ID, e.g. `?opt=3&theme=black&w=360&screen=T3`.

## What was built

**39 new frames**, all tagged "New" in their captions, in the order of the brief:

- Library (after L6): L7, L7l, L7c, L7ce, L7cx, L7u, L7z.
- Export (after E5): E6, E6n, E7, E7f, E7t, E7x, E7p, E2p, E4p, E3h, E3p, E5n, E5b, E4s.
- Settings (after T2c): T1s, T3, T3e, T3l, T3z, T3g, T3b, T3c, T3ca, T3cu, T3cd, T3d, T3k, T3f, T3r, T3s, X1, X1c.

Components: `bookSheet`, `removeDialog`, `undoSnack`, `fallbackCard`, `e6Footer`, `partPage`, `rangeList`,
`exportFileSheet`, `deleteDialog`, `deleteAllDialog`, `T3`, `X1`, `storeTotals`; the hooks of brief §4.3
(`L1` books/card options, `T1` storageRow, `playerSheet`/`exportPage` sub/range/list, `exportBody` new keys,
`positionFrame` `[data-center-cur]`, caption "New" tag, new icons, `STORE` and the space helpers of §4.5–§4.6).

**M4B only (orchestrator decision 1):** the Format group, its "Spec change" badge, the MP3 notes and the MP3
Impact bullet are gone; `xFormat` no longer exists. Summaries read "Heart · Full quality · 1×"; E4 / E4p / E4s keep
"… · M4B"; N3's second line "Heart · 1× · M4B" stays (logic file 10 §3.10 still says it). The export row's subtitle
is now "Pick the voice, quality and speed" (file 10 §2). E1x became "1.5× picked" (`xSpeed: 3` only). The old question
1 (MP3) was removed from "8. Questions". `.badge` stays: X1 uses it for "Android’s screen, not Lectrice’s".

**Live phone (§8):** long-press (500 ms, 8 dp slop) and right-click on Library rows and the Continue card → book
sheet; Clear this book's audio; Remove → dialog (playing / export running / exports checkbox) → row collapses
200 ms → Undo snackbar 8 s → purge (library bytes raise "free"; ticked exports go too; a paused export of the book
is dropped); the Continue card falls back to Designing Calm Software, then to none (L2). Settings shows the single
Storage row; T3 with book sheets, export-file sheets (Open / Share / Save to Audiobooks / Delete / Go to the
export), Clear audio cache (600 ms spinner), Delete all (mock snackbar), model rows → T2 and back. Phone space
toggle re-renders the E1 footer (E6 / E6n), T3 and the T1 row, and pauses a running export that no longer fits
(E5n at its %; the player row freezes). E6 → E7 with the suggested range, From/To lists (centred current entry,
"move the other end" rule, cross-faded footer), Pick what fits, Start → E2 "… · Ch. 7–12" → E4p; Save to
Audiobooks → E4s (+ the export in Storage switches to "In Audiobooks"); Make another version → E1. "Free up space"
(E6 / E5n) parks the player sheet, opens T3 with the banner; when enough is freed the banner becomes the room-now
card (and a paused export continues at once); ← / ◁ / "Back to the export" un-parks the sheet. Android Settings ›
→ X1 (Clear cache → X1c; Clear storage → T3 with ✕ → back to X1). ◁ Back order as §8.5.

## §3 re-take check (existing frames)

All 33 existing frame screenshots were re-taken into the scratchpad at the same viewport and pixel-compared:

- **21 identical**, including L1 (light / dark / black / 360), T1, R1, R13 (all), N1–N2L, R4x's neighbours.
- **11 differ only where decision 1 removed MP3/format copy**: E1, E1-black, E1-360, E1q, E1v, E1x (Format group
  gone), E2 and E4 (summary line without "M4B"), R4, R4-black and R13m-black (export row subtitle). These were
  re-taken into `design/screens/`.
- **R1h-light** differs in a 20 px strip of reader text under the pill; three captures (CDP and the original
  `--screenshot` command) each gave a different strip, so it is capture noise in untouched reader code. Left as is.
- `page-light` / `page-black` (1400 × 900) differ only in the "How to try" column (6 new items): re-taken.

## §4.6 number checks (computed by the page's helpers in the headless run)

| Case | Got |
|---|---|
| F whole book | 242.256 MB; needed **585 MB** |
| Suggested range | 7–12: "Chapters 7–12 · about 3 h 5 min · about 89 MB", "Takes about 45 min on this phone"; needs 283 ≤ 320; 7–13 needs 332 |
| 7–14 | "Chapters 7–14 · about 4 h 25 min · about 127 MB", needs **362 MB** → Start disabled, "Pick what fits" |
| Free 120 | chapter 7 alone needs 136 → `suggestRange` null (E6n); "Free up about 465 MB" |
| E5 at 21 of 49 | "Needs about 481 MB free · 120 MB free now" |
| E5b | "Needs about 342 MB free · 120 MB free now"; banner "Free up about **222 MB** and continue" (decision 2) |
| T3 banner from E6 | "Free up about 265 MB to export this book." |
| 2× whole book | needs 342 MB (still > 320) |
| Storage | models 496 · books 48 (+ 301 audio) · cache 312 (31 %) · exports 272 · total 1173 → "1.2 GB" |
| X1 | App size 44 MB · User data 817 MB · Cache 312 MB · Total 1.2 GB; after Clear cache: 0 B · 861 MB |
| E7p | "320 MB free … needs about 486 MB.", "Pages 97–272 · about 3 h 40 min · about 106 MB", "Takes about 55 min" |
| Live export sizes | whole book at 1× → 231 MB (from the summary string, as the brief says); 7–12 → 89 MB |

Measured: E6 footer 283 px at 412 (brief: ≈ 290), 307 px at 360; the body above it keeps 528 / 389 px visible
(limit 200). No horizontal overflow in L7, L7ce, T3, T3g, E6, E7, E7x at 412 or 360; `.sz` never clipped; the
L7ce dialog buttons stay on one line at 360.

## Headless smoke test (Chrome 154, CDP, scratchpad only; network blocked)

All steps pass with no console errors or exceptions: numbers above; long-press and right-click open the sheet
without opening the book; dialog over sheet shows two scrims; Cancel keeps the sheet; remove → 3 rows + undo
snackbar → Undo restores (store and rows); clear audio snackbar; purge after 8 s with the box ticked (exports
x2/x3 gone, free +22 +268); Continue-card removal → fallback card → Undo; T1 row text; T3 summary + legend; book
row → sheet; Save to Audiobooks (meta, total 1 GB, free unchanged, snackbar); Delete (dialog text, row gone, free
+115); Clear audio cache (spinner → snackbar → disabled → header without audio); model row → T2 → back → T3;
Delete all dialog + snackbar; back → Settings → Library; E6 footer; E7 suggested range; To list centred (at the
scroll end for entry 12); 7–14 doesn't fit; Pick what fits; From after To moves the To; Start → E2 with range → E4p
(89 MB listed) → E4s (disabled button, lifted snackbar, "In Audiobooks") → Make another version → E6 again;
Free up space → T3 banner 265 → delete 153 → banner 112 → delete 115 → room-now card → Back to the export (footer
normal); whole export paused by "Nearly full" at its % (E5n text with k, needed/free line, Continue disabled;
player row frozen; reopening shows E5n); Free up space (resume) → room-now "The export continues." → back → E2 →
E4 (231 MB listed); back order list → E7 → E1 → sheet; X1 numbers → Clear cache → X1c → Clear storage → T3 ✕ → X1
→ back; `prefers-reduced-motion`: row, sheet and E7 push are instant; gallery: 108 frames, 39 New tags, frame order
as §3.1, no element with a visible scrollbar, no horizontal page overflow, at light/412 and black/360; the page
chrome toggle and "Android Settings ›" drive the live phone.

## Checklist (§9)

- [x] Only the mockup file, this note and screenshots; no Artifact, no git, nothing downloaded; opens from disk.
- [x] Existing frames unchanged except decision 1's M4B clean-up (re-take comparison above).
- [x] Every new ID exists in the right gallery and order, with caption and "New" tag; solo mode works (the
      screenshot batch used it).
- [x] Copy verbatim (curly quotes/apostrophes in `COPY`); numbers match §4.5 / §4.6.
- [x] New frames are M4B-only; only E4 / E4p / E4s name the format (and now the whole file is).
- [x] No visible scrollbar (checked in T3, sheets, E7 list, gallery); wheel/drag/keyboard scrolling unchanged.
- [x] Light, dark, black from tokens only (no new tokens; the stacked bar uses `--onTile0…4`); no `opacity` text
      muting (opacity only in the 150 ms snackbar/row fades); dialogs over sheets show both scrims (`.dwrap`).
- [x] 360 × 800: L7, L7c/L7ce, T3, E6, E7 measured (above) and screenshotted.
- [x] 200 % font (T3g): legend wraps one item per line, rows wrap, no overlap.
- [x] 48 dp targets (`.cbx` 48, `.xrange` 56, sheet rows 56, `.sysact` 72, snackbar Undo 48, list rows 48);
      `aria-label`s on icon-only controls and the From/To rows; checkbox `role="checkbox"`; list rows
      `role="radio"`; snackbars `role="status"`; the global `:focus-visible` outline applies to every new button.
- [x] Live: all of §8.3 items 1–8 and the §8.5 back order (test above).
- [x] `prefers-reduced-motion`: instant (CSS rule + `after()` guard).
- [x] No text from the user's books; only the mockup's titles and "Garden Club Minutes (no chapters)".
- [x] Self-review: a class scan over the new CSS block finds every class used; no unused helpers or COPY keys;
      `xFormat`, `formatNotes`, `badge` copy and the `x-format` action are gone.

## Ambiguities resolved (most literal reading; say if you want another)

1. **M4B-only flag dropped.** Because the whole file is M4B only (decision 1), the brief's `m4bOnly` view key and
   `exportFrame({ m4bOnly: true })` are unnecessary; `exportChoices`/`exportSummary` are simply format-less.
2. **Book-sheet rows are 56 tall** as the brief says, via a new rule `.screen .sheet .bd.flush .bsh ~ .row
   {min-height:56px}` (the existing opt-3 rule makes sheet rows 48). The export-file sheet shares it.
3. **Dialog + own scrim** are wrapped in `.dwrap` (position absolute, z-index 24) in both the static frames and the
   live phone, so the second scrim really dims the sheet (a bare `scrim() + dialog` would sit under the sheet).
4. **Snackbar stacking.** The brief's `.snack.over{z-index:25; bottom:40px}` was split: `.over` = bottom 40
   (no pill/FAB), `.lift` = z-index 25 (above the full-height sheet, E4s only). Storage snackbars keep the default
   z-index, so a sheet opened while one shows dims it (as on Android) instead of the snackbar covering the sheet's
   bottom row.
5. **Storage / X1 layers get `.layer.above` (z-index 19)** so they sit above the Listen view's stage (z 18) when
   "Free up space" is used from the stage's export page; sheets (20/21) stay above them.
6. **"Phone space: Nearly full" pauses a running export only if its resume no longer fits** (always true for the
   whole book; a short range that still fits keeps running) to avoid an E5n that says Continue is possible.
7. **Loading (T3l):** the legend is omitted along with the empty track (only "…" numbers are specified).
8. **X1 takes the store** (`X1(v, store)`) instead of a `cacheCleared` flag; X1c passes the cleared store, and the
   live X1 follows the live store.
9. **Finished live exports** use the summary's size (231 MB for the whole book at 1×, from `EST`) rather than the
   space formula's 235 MB, as §8.3.7 says; ranges use the formula (89 MB).
10. **E7's From/To `aria-label`** for the Preface reads "From Preface. Change"; page mode "From page 97. Change".
11. **Snackbars that the live phone shows while a bottom button is under them** (e.g. "Saved…" over "Make another
    version") hide after 4 s; the user waits or taps elsewhere, as on Android.
12. **"8. Questions" lost its answered MP3 question**; the new "Questions (24 October)" list quotes the brief's
    three questions verbatim and adds the 24 October answer in italics after each.
13. **T1 storage row "…" while loading** is not wired (no frame or live state loads it).
14. **Android Settings › while Storage is open from an export** discards the parked sheet (the export page is
    reachable again from the player); otherwise X1 opens over whatever is shown.

## Screenshots

New (51), device pixel ratio 2, solo mode, network blocked, in `design/screens/`:

- `android-q2-L7-light.png`, `-L7-black`, `-L7-light-360`, `-L7l-light`, `-L7c-light`, `-L7c-dark`, `-L7ce-light`,
  `-L7ce-light-360`, `-L7cx-light`, `-L7u-light`, `-L7u-black`, `-L7z-light`
- `-T1s-light`, `-T3-light`, `-T3-dark`, `-T3-black`, `-T3-light-360`, `-T3e-light`, `-T3l-light`, `-T3z-light`,
  `-T3g-light`, `-T3b-light`, `-T3c-light`, `-T3ca-dark`, `-T3cu-light`, `-T3cd-light`, `-T3d-black`, `-T3k-light`,
  `-T3f-light`, `-T3r-light`, `-T3s-light`, `-X1-light`, `-X1-dark`, `-X1c-light`
- `-E6-light`, `-E6-black`, `-E6-light-360`, `-E6n-light`, `-E7-light`, `-E7-light-360`, `-E7f-light`, `-E7t-light`,
  `-E7x-light`, `-E7p-light`, `-E2p-light`, `-E4p-light`, `-E3h-light`, `-E3p-dark`, `-E5n-light`, `-E5b-light`,
  `-E4s-light`

Re-taken because decision 1 changed them (13): `E1-light`, `E1-black`, `E1-light-360`, `E1q-light`, `E1v-light`,
`E1x-light`, `E2-light`, `E4-light`, `R4-light`, `R4-black`, `R13m-black`, `page-light`, `page-black`.

Command (one frame; 412 → window 432 × 935, 360 → 380 × 820; the batch used CDP with
`Emulation.setDeviceMetricsOverride` to the same size, which gave pixel-identical results):

```
google-chrome --headless=new --disable-gpu --host-resolver-rules="MAP * ~NOTFOUND" \
  --force-device-scale-factor=2 --window-size=432,935 --virtual-time-budget=3000 \
  --screenshot=design/screens/android-q2-T3-light.png \
  'file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html?opt=3&theme=light&w=412&screen=T3'
```
