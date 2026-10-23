# Android design options: progress note (Fable, 2026-10-23)

Built from `docs/2026-10-23-android-design-brief.md` §7–§12. Nothing outside `design/` and this
file was touched; no git commands, no Artifact.

## Open it

```
xdg-open /home/kia/Desktop/learning/lectrice/design/2026-10-23-android-options.html
```

- Toolbar: option tabs (1 Pill · 2 Material · 3 Quiet · 4 Stage), Theme (Light/Dark), Width
  (412/360), and on option 2 only Wallpaper (Blue/Green). State is kept in the URL hash.
- Each option: §4 intro, one live phone with a "How to try" list and a `◁ Back` button
  (system back), then the galleries (First run, Library, Reader & player, Chapters, Background,
  Export, Settings) at zoom 0.5 with `ID · name` + note captions. §5 and §6 are at the bottom.
- Solo mode for screenshots: `?opt=1&theme=dark&w=412&screen=R4` renders one frame at zoom 1
  (`screen=live` renders the live phone; add `&play=1` to open the reader playing).

## What was built

One self-contained file (177 KB): inline CSS, vanilla JS, inline Material Icons paths (Apache-2.0).
The only network asset is the Google Fonts link (Noto Serif, Roboto, Roboto Mono); with no network
the page uses the local Noto Serif and the system sans.

- Four token sets as CSS custom properties on `.screen[data-opt][data-theme][data-wall]`: base
  light/dark, option 3 true-black dark, option 2 Material roles (Blue and Green, light and dark;
  the shared components read them through the base names), option 4 stage tokens (the Listen
  stage swaps the base names, so every sheet inside it is in stage colours), six cover hues.
- Shared components per §9.4 (buttons, icon buttons with 48 dp touch, rows, switch, slider with
  buffered part + chapter ticks, chips, progress bars, spinner, bottom sheet with detents, dialog,
  banners, snackbar, cover tiles), reader typography per §8.4 (`--sp` unit, doubled for R12).
- The Long Book is generated from `LONG_BOOK_CHAPTERS` + `LONG_BOOK_SUBSECTIONS` (copied from
  `frontend/src/api/mock/sampleText.ts`) with filler paragraphs from `PREFACE`/`CHAPTER` and the
  brief's 7.3 passage; current sentence "When the promise is kept…", page chip "p. 266" before
  "The most common way". 28 voices from `voices.ts`.
- Static frames per option: 50 (option 1), 54 (2), 54 (3), 54 (4). All §3 IDs exist in every
  option, plus the extras listed below.
- Live phones (one per option) with the §11.1 list 1–10 adapted per option: push transitions,
  2.5 s playback with code-cue skipping and 35 % follow-scroll, tap-to-jump, manual scroll →
  "Back to current" chip (↓/↑), chapters sheet with drill-in, player sheet with half/full detents,
  handle drag and **pill drag that follows the pointer**, speed/sleep/voice/export (42 → 100 % in
  6 s → E4), predictive-back preview (400 ms) then library, Settings push/pop. Option 2: hiding
  app bar, floating toolbar, Now playing with stacked speed dialog / sleep sheet / voice sheet /
  chapters tree, Material menu, wallpaper swap. Option 3: pill collapse (scroll down or 4 s idle)
  and expand, swipe left/right on the pill (± sentence with the 120 ms nudge), long-press play →
  speed scrubber, one-time hint, inline voice/sleep/export, searchable outline. Option 4: shelf,
  Read | Listen toggle, stage with tap = play/pause flash, swipe = ± sentence with the teleprompter
  glide, section jumps, stage-coloured sheets.
- Headless smoke tests (Chrome 154, scratchpad only, not in the repo) drove every live
  interaction above for all four options; all pass with no console errors.

## Checklist (§12)

- [x] One file `design/2026-10-23-android-options.html`; renders from disk without network
      (fallback fonts); no external assets except the Google Fonts link; no libraries; no
      Artifact; git untouched.
- [x] Four option tabs; Light/Dark; 412/360; option 2's Blue/Green; solo mode works for every
      screen ID (the screenshot batch used it for 52 frames).
- [x] Every §3 screen ID exists in every option with a caption; R13 in options 2 and 4; R1h in
      option 3.
- [x] One live phone per option with the listed interactions (see above).
- [x] All colours via custom properties. `grep -c '#[0-9a-fA-F]\{6\}'` = 37 lines: the page-chrome
      token block, the phone token blocks (incl. shade/system-dialog/lock-wallpaper colours kept
      as tokens), the bezel and the punch-hole. None elsewhere.
- [x] No text muted with `opacity` (only whole-layer/chip fades for motion); text pairs are from
      §9.2.
- [x] Interactive elements ≥ 48 × 48 (icon buttons 48, chips/back-chip/option-2 buttons with an
      8 dp pseudo hit area), `aria-label` on icon-only controls, `:focus-visible` 2 px accent
      outline offset 2.
- [x] Highlight never under the pill: follow-scroll keeps it at 35 % of the height; reader bottom
      padding clears the pill (148 dp; 152 option 2; 140 option 3).
- [x] 360 width checked for R1 (all options), R4, L1, E2, Now playing, stage, option 4 grid: no
      overlap or clipping (the pill drops the ⌃ at 360; speed chips scroll).
- [x] R12 frames (reader and library at 200 %) show no overlap: status line 1 only with
      ellipsis, pill 72 tall, Continue card stacks the play button.
- [x] `prefers-reduced-motion` zeroes every transition/animation inside phones; JS waits use
      0 ms and the teleprompter jumps.
- [x] All text is from §10 or the named mock files; no text from the user's books.
- [x] Self-review: unused CSS/JS removed (checked class names against usage), no scaffolding.

## Ambiguities resolved (most literal reading, or copied from the web app where silent)

1. **Slider chapter ticks:** level-1 starts only (15 ticks). Level 1 + 2 would be ~90 marks on a
   380 dp track, i.e. a dashed line. Flagging rather than inventing a smaller tick.
2. **R2 "scrolled up by ~400 px":** 400 px leaves the highlight visible just above the pill, so
   the frame scrolls 600 px to make it sit below the view as the chip's ↓ implies.
3. **"S2 (live) with Full quality selected":** the live phone starts on L1 (§11.1), so the
   selected state is the static frame `S2s`.
4. **N1/N2 cover tile on a same-hue card:** the 64 dp tile gets a 1 px border in the tile's text
   colour so it does not vanish into the card.
5. **Sentence padding `0 2px`:** applied to every sentence span, not only the current one, so
   the text does not reflow when the highlight moves.
6. **Progress numbers** (49 %, 38 %, 9 min, 4 h 12 min) stay the fixed §10 values in the live
   phone after a jump; the chapter/page in the status do update.
7. **Option 2:** `R4` shows the More menu (the player sheet is replaced by Now playing `R13`);
   `R5` is a Material modal sheet with radio items (not specified; mirrors the sleep sheet);
   `R8b` is the tree with chapter 4 expanded (no drilling in a tree); `L2` (not specified) is the
   small app bar + extended FAB; export `E1–E5` are modal sheets with a close button (option 2
   has no player sheet to push into); `R13s` (speed dialog) and `L1b`/`L1c` (collapsed bar, mini
   player) are extra frames for specified states; warn/err/ok/inverse tokens are not in the
   option-2 table, so the base pairs are used.
8. **Option 3:** `R8b` is the same flat outline (there is no drill-in), scrolled to show level-3
   entries; `E2–E5` render inline in the export row (the brief only specifies E1 inline and
   "no sub-pages"); `L2` is the dense header + empty text + FAB; `R1t` (morph), `R1s` (scrubber)
   are extra frames because those states are otherwise only visible in the live phone.
9. **Option 4:** `L2` is copied from option 1; `R13c` (code card), `R13s` (speed sheet in stage
   colours) and `R8L` (chapters in Listen mode) are extra frames for states the brief describes.
10. **Cover hue hash:** a simple string hash; the hue a title gets differs from any other
    implementation.
11. **Status scrim:** `color-mix(in srgb, var(--bg) 94%, transparent)`.
12. **Speed chips at 412:** with the fallback sans the sixth chip clips by a few px; the row
    scrolls horizontally (as specified for 360). Roboto fits.
13. **Fonts in the screenshots:** Roboto is not installed locally and the screenshots were taken
    with the network blocked (metered), so they show the system sans and the local Noto Serif.
14. **Option 2 live:** the floating toolbar is not draggable (there is no sheet to pull up).

## Screenshots (52, device pixel ratio 2, via solo mode)

Written to `design/screens/`:

- Per option, light + dark: `android-{1,2,3,4}-L1-{light,dark}.png`,
  `android-{1,2,3,4}-R1-{light,dark}.png`, `android-{1,3,4}-R4-{light,dark}.png`,
  `android-2-R13-{light,dark}.png`, `android-{1,2,3,4}-R8-{light,dark}.png` (32).
- 360 width, light: `android-{1,2,3,4}-R1-light-360.png` (4).
- Option 1 extra: `android-1-S2-light.png`, `android-1-S3-light.png`, `android-1-L2-light.png`,
  `android-1-L3-light.png`, `android-1-R2-light.png`, `android-1-R7-light.png`,
  `android-1-E2-light.png`, `android-1-E4-light.png`, `android-1-T1-light.png`,
  `android-1-N2-dark.png`, `android-1-R5-dark.png` (11).
- Option 3 extra: `android-3-R1c-dark.png`, `android-3-R8s-light.png` (2).
- Option 4 extra: `android-4-R13-light.png`, `android-4-R13-dark.png`,
  `android-4-L1-light-360.png` (3).

How they were made (one frame; 412 → window 432×935, 360 → 380×820):

```
google-chrome --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
  --window-size=432,935 --virtual-time-budget=2500 --screenshot=design/screens/android-1-R1-light.png \
  'file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-options.html?opt=1&theme=light&w=412&screen=R1'
```
