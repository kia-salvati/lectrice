# Android NPU toggle options (A / B / C): progress note (Fable, 2026-10-25)

Built from `docs/2026-10-25-android-npu-toggle-brief.md` §3–§8. Only `design/2026-10-23-android-quiet-v2.html`,
48 new files `design/screens/android-npu-*.png` and this note were touched; no git commands, no Artifact, nothing
downloaded (network blocked in every headless run), no code outside `design/`. The feature itself stays blocked on
licensing (file 22); these are options for the user to pick.

## Open it

```
google-chrome "file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html#theme=light&w=412"
```

Then scroll to **"NPU acceleration · options (25 October)"** (after the Settings gallery, before the questions). Solo
mode works for every new ID, e.g. `?screen=NB2&theme=black&w=360`.

## What was built

- **Page chrome (§3.1–§3.2):** the `h2.group` "NPU acceleration · options (25 October)", the `#npu-options` impact box
  (three option bullets, "Recommended: B.", "Blocked on licensing"), four `h3.npu-opt` galleries `npuA` / `npuB` /
  `npuC` / `npuK`, and "Questions (25 October, NPU)" appended to `#questions` with §9's four questions verbatim.
- **36 new frames**, all tagged "New": NA1–NA9 + NA1x; NB1–NB11 + NB1x, NB2x, NB10x; NC1–NC8; NK1–NK4 (the four `x`
  frames are the 200 % font spot checks, caption "200 % font").
- **CSS (§4.2)** appended as one block at the end of the phone CSS (before the reduced-motion rule), the brief's rules
  plus five small additions listed under "Ambiguities" 2–4. No new tokens; greyed = `fg2` + the disabled switch.
- **Icons:** `memory` and `description` (both render correctly).
- **Copy and data (§4.5):** `NPU` and `COPY.npu` exactly as the brief, defined after `fmtMB` (after the export maths).
- **Helpers (§4.6):** `npuOn`, `npuDis`, `npuEdition`, `rowCls`, `npuSwitch`, `npuSubA`, `npuRowA`, `npuSectionB`,
  `npuActs`, `npuStatusCard`, `T4n`, `npuCardC`, `firstUseDialog`, `firstUsePage`, `firstUseSheet`, `aboutQnn`,
  `editionSheet`, `npuSnack`, and the frame shorthands `npuT1(opt, state)` / `npuT2(opt, state)`.
- **Hooks in existing code (§4.3), all off by default:** `T1(v, { npu })` (option A row after the model row; option B
  "Performance" header + section between Voice model and Storage; option C model-row text from `modelVal`; the
  `.scroll` gets `data-center-cur` and the NPU row / model row the class `cur`; "Qualcomm AI Runtime" About row in the
  NPU edition), `T2(v, { npu })` (option C card after the small-model card; option A "Remove NPU files (1.25 GB)" on the
  Full quality card while on), `T3` model meta `"370 MB + NPU files 1.25 GB · in use"` when a model has `npuMB`.
- **The live phone is untouched:** `Live` never passes `npu`; the smoke test opened the live Settings and Voice model
  pages and found no NPU row, section, card or About entry.

## Pixel comparison of the existing frames (§3)

`T1`, `T1s`, `T2`, `T3`, `R1` (light, 412) were captured from the untouched file into the scratchpad **before** the
edit and again **after**: all five are byte-identical before/after. Against `design/screens/`: `T1`, `T3`, `R1`
identical; `T2` has no stored light screenshot (never taken); `T1s` differs from its stored file by a 216 × 25 px strip
(at 2×) around the Storage row's value, and it differs in exactly the same way **before** my edit, so it is
pre-existing capture drift, not this change. Nothing in `design/screens/` was overwritten.

## Headless smoke test (Chrome, CDP, scratchpad only; network blocked)

- Gallery page: 144 frames (108 + 36), 75 "New" tags, the four NPU galleries in the brief's order, every caption has
  its note. **Zero exceptions, zero console errors**; the only log entry is the blocked Google Fonts request (as in
  every earlier run).
- Toolbar: Dark and Black switch every screen; 360 rebuilds all 144 frames.
- No element with a visible scrollbar (gallery page at light/412 and black/360, and every new frame solo at 412 light
  and 360 dark); no horizontal page overflow; no flex neighbours overlapping in rows, section notes, status cards or
  the NPU card; every icon-only button has an `aria-label`; the greyed switches (NA5–NA7, NB7–NB9, NC6, NC7, NK3) all
  carry `aria-disabled="true"` inside a `.dis` row/card; every centred T1 frame has its NPU row (C: model row) at 50 %.
- First-use: NA8's dialog fits inside the screen at 412 and 360 (no scrolling needed at 1×); NB10's text block ends
  above the buttons at 1× and scrolls at 200 % (NB10x), buttons inside the screen.
- Live phone: play → reader → player sheet → back ×2 → library → Settings → Voice model → back ×2 all work.
- Only two "overflow" hits in new frames, both harmless: the indeterminate bar's animated span (NB5, NC4; clipped by
  `overflow:hidden`) and, at **200 % font and 360 wide together** (not a screenshot case), the nowrap text button
  "Read Qualcomm's license" is 410 px wide. At 412 it fits.

## Checklist (§8)

- [x] Only the mockup file edited (+ this note + screenshots); existing frames pixel-identical (above).
- [x] Every frame in light, dark and black and at 360: no overlap, no visible scrollbar (test above + screenshots).
- [x] 200 % font: NA1x, NB1x, NB2x, NB10x added with `{ x2: true }`, caption "200 % font"; screenshotted.
- [x] Greyed rows: `aria-disabled="true"` on the switch; labels on every icon-only control.
- [x] Copy exactly as §4.5 / file 22 §3.11; the only strings not in `COPY.npu` are the two section headers of the NPU
      page ("Files" from §4.6, "About" reused from `COPY.t1.about`).
- [x] No text from the user's books (only the mockup's invented books).
- [x] Self-review: every new CSS class and helper is used; no leftover scaffolding.

## Ambiguities resolved (most literal reading; say if you want another)

1. **"1.25 GB", not `fmtMB`.** `fmtMB(1250)` prints "1.3 GB", while every label in the brief says 1.25 GB, and §4.5
   allows the literal. `NPU.filesLabel = '1.25 GB'` is used for "Remove NPU files (1.25 GB)", the NPU files row and the
   Storage meta (NK4); other `npuMB` values would still go through `fmtMB`.
2. **Icon sizes (CSS additions):** the lead icon inside option B's `button.pick` and the icon in option C's card are
   not reached by `.row > svg.lead`, so `.row.split .pick > svg` and `.mcard.npuc .flex > svg` set 24 × 24; the
   first-use page's 48 dp circle is `.npuic`. A greyed split row also greys its icon (`.row.dis .pick > svg`).
3. **First-use page at 200 %:** with the brief's `.col` + absolute `.bottom`, the three paragraphs would be clipped
   under the buttons at 200 % font. The page uses `.col.fucol`: the `.fu` block is `flex:1; overflow-y:auto` (no
   scrollbar) and `.bottom` sits in flow at the bottom (`padding-bottom:40px`), so at 1× it looks the same and at 2×
   the text scrolls between the title and the buttons.
4. **First-use dialog scroll area** is the inline style the brief gives (`max-height:calc(100% - 96px); overflow-y:auto`
   on `.dlg`); the actions are one right-aligned column.
5. **"Remove NPU files" on T2 in option A** sits **inside** the Full quality card as its last line (like T2b's "Delete
   (126 MB)" inside the small card), not as a loose button between the two cards.
6. **Option C's model row** has no subtitle in the mockup (its one line is the title), so `modelVal[state]` replaces
   that line. Its `cur` class is added with `rowCls()` (a class insert) because `settingsRow` was not to be changed;
   the same helper adds `dis` to the NPU page's switch row for `needsFull`.
7. **Option C's card body is `page.card[s][1]`** as §4.6 says, so NC4 (checking) reads "This takes about half a
   minute." under "Use the NPU", and NC3 "Downloading the NPU files · 312 of 830 MB"; greyed states show the note.
8. **`npuSwitch` disabled:** `switchCtl(...)` with `aria-disabled="true"` inserted (the brief's wording); `data-act`
   stays, as nothing toggles in static frames.
9. **Option B note button at 360 (NB8):** "About the NPU edition" sits beside the note as the brief's CSS implies
   (`margin:-10px -12px -10px 0`), so the note wraps to four lines at 360. Screenshot `android-npu-NB8-light-360.png`.
10. **Split row padding:** the brief's `.row.split{padding-right:8px}` puts the switch 8 px further right than the
    other switches on T1; kept as specified.
11. **Edition** is derived from the state (`npuEdition`: `normal`/`normalAllowed` → normal, else NPU), so the T1 call
    stays `{ opt, state }`.
12. **NK4's summary card still says "1.2 GB used by Lectrice"** while the Full quality row says "370 MB + NPU files
    1.25 GB": `storeTotals` was not in the allowed hooks, so the NPU files are not added to the totals. Say if they should be.
13. **Spacing on the NPU page** (status card in `px16`, paragraphs `px16 mt8`, 48 px end spacer) follows T1's rhythm.
14. **Spinner in a subtitle** (NB-style checking) needs a flex line, so that `.m` is `display:flex` with the spinner
    before the text; option A keeps the spinner in `.tr` before the switch.

## Screenshots (48, device pixel ratio 2, solo mode, network blocked) in `design/screens/`

- Light 412 (36): `android-npu-NA1…NA9-light.png`, `-NB1…NB11-light`, `-NC1…NC8-light`, `-NK1…NK4-light`,
  `-NA1x-light`, `-NB1x-light`, `-NB2x-light`, `-NB10x-light`
- Dark (6): `-NA1-dark`, `-NB1-dark`, `-NB2-dark`, `-NB7-dark`, `-NC2-dark`, `-NK1-dark`
- Black (2): `-NB2-black`, `-NB10-black`
- 360 (4): `-NA1-light-360`, `-NB1-light-360`, `-NB8-light-360`, `-NC2-light-360`

Command (one frame; 412 → window 432 × 935, 360 → 380 × 820):

```
google-chrome --headless=new --disable-gpu --host-resolver-rules="MAP * ~NOTFOUND" \
  --force-device-scale-factor=2 --window-size=432,935 --virtual-time-budget=3000 \
  --screenshot=design/screens/android-npu-NB2-light.png \
  'file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html?opt=3&theme=light&w=412&screen=NB2'
```
