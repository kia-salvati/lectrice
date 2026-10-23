# Android "Quiet v2" mockup: progress note (Fable, 2026-10-23)

Built from `docs/2026-10-23-android-quiet-v2-brief.md` §3–§7 on top of option 3 of
`design/2026-10-23-android-options.html` (that file is unchanged; md5 checked). Nothing outside
`design/` and this file was touched; no git commands, no Artifact, nothing downloaded.

## Open it

```
xdg-open /home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html
```

- Toolbar: Theme (Light / Dark / Black), Width (412 / 360). State is kept in the URL hash.
- Page: the five changes from §1, the live phone with "How to try" and `◁ Back`, then the galleries
  (First run, Library, Reader & player, Listen view, Chapters, Background, Export + the Impact box,
  Settings) at zoom 0.5 with `ID · name` + note captions, and §8's questions at the bottom.
- Solo mode: `?opt=3&theme=black&w=412&screen=R13` renders one frame at zoom 1 (`theme` = light |
  dark | black; `opt` is accepted and ignored); `screen=live` renders the live phone (`&play=1`
  opens the reader playing).

## What was built

One self-contained file (152 KB): inline CSS, vanilla JS, inline Material Icons paths. The only
network asset is the Google Fonts link; without network the page uses the system sans and the
local Noto Serif.

- **Scrollbars:** the global rule from §3.2 (`scrollbar-width: none` + `::-webkit-scrollbar`), the
  per-element rule on `.chips` removed, no `overflow: hidden` added to anything that scrolls.
  Code blocks: `div.code` (position relative) wraps the scrolling `pre`; JS sets
  `data-more="right"` on render and on scroll while `scrollLeft + clientWidth < scrollWidth − 1`,
  and the CSS draws the 24 px `transparent → bg2` overlay.
- **Themes:** base light; dark and black share the base dark block, then the black block
  overrides bg/bg2/fg/fg2/line/accent/onAccent/hl/hlFg. Black uses the page's dark chrome. New
  tokens: `--nowBarBg/--nowBarFg/--nowBarFg2` only.
- **Library (L1–L6, R12b):** option 1's Continue card (with the 3 dp accent bar on its left edge)
  and Recent rows with cover tiles, Quiet's `.qhead` header, row title 16/20 500 + meta 14/18,
  min-height 72, divider inset 72, Quiet's round FAB, list spacer 120; no bottom bar.
- **Reader and pill (R1 group, R2–R12):** option 3's reader; the pill status is `data-act="listen"`
  with the `open_in_full` icon (dropped at 360) and the §4 TalkBack label; R1h has the new hint;
  R4's export row pushes the export page (option 1's page mechanics); new R4x shows the running
  export in the row (subtitle + 120 × 4 bar).
- **Listen view (R13 group):** `stage(v, { overlay, state })` rewritten per §5.3: top bar (chevron
  "Back to reading", book + section title, also the drag-down handle), teleprompter (previous 20/30
  clipped at the top, current 28/40 with the 3 dp accent bar, next 20/30 clipped at the bottom,
  24 px fades, drops to 24/34 past six lines), thumb zone (chapter progress with section ticks,
  64 / 88 / 64 transport, two "Section" buttons, Speed · Sleep · Chapters · More). Frames R13, R13p,
  R13c (code card with the right-edge fade), R13h, R13t (half-grown rectangle + the play circle
  midway), R13b (scale .9, translateY 24, radius 32), R13s (small speed sheet with `.segc`), R13m
  (player sheet at half detent), R13k (outline).
- **Export page (E1 group, E2–E5):** header + scrolling body + pinned footer. Voice row expands into
  six voice rows with sample buttons + "All 28 voices"; Quality radio rows (Smaller shows "Download
  · 126 MB" until installed, then becomes selectable; the wait-while-listening note when it is
  picked); Format segmented M4B | MP3 with the "Spec change" badge and the per-format note; Speed
  segmented with the per-speed note; the read-code switch; the reassurance card. Footer numbers
  come from the §4.1 table (length/size by speed, time by quality), Start repeats the time. E2–E5
  show the running summary ("Heart · Full quality · M4B · 1×") under the header; E4's summary is
  "8 h 10 min · 231 MB · M4B" (MP3 → "… · MP3").
- **Background:** N1/N2 media card title = the sentence on one line (ellipsis), artist "Ch. 7 · The
  Long Book"; N2s Samsung Now Bar pill (340 / 300 wide); N2se the expanded Now Bar card; N2L the
  stage over the lock screen (lock icon at y 40, "Swipe up to unlock" at the bottom, thumb zone up
  by 24); N3 with the second line "Heart · 1× · M4B".
- **Settings:** T1 gets "Sentence on the lock screen" (on) and "Listen view over the lock screen"
  (off) after "Read ahead"; theme segments System · Light · Dark · Black.
- **Live phone (§6):** everything from option 3 (collapse / expand, pill drag → sheet follows the
  finger, swipe ± sentence with the nudge, long-press play → scrubber, one-time hint, inline voice /
  sleep, outline, Settings) plus: pill status tap → container transform into the stage (clip-path
  from the pill's rect, play circle FLIP, content fade 120–320 ms; instant under reduced motion);
  stage tap = play/pause + flash, swipe ≥ 48 dp, section jumps, long-press play → scrubber above
  the big button, Speed / Sleep sheets, Chapters (jump stays in the stage), More → player sheet
  (export page works there too); chevron, ◁ Back (R13b preview 400 ms) and drag-down (follows the
  finger, scale 1 → .9 over 300 px, close past 30 %) all return to the reader with the current
  sentence at 35 %; export page choices update the footer with the 120 ms cross-fade; "Download ·
  126 MB" runs the 3 s fake download; Start → E2 42 → 100 % over 6 s → E4 (the player row follows).
  ◁ Back order: scrubber → export page → sheet → Listen view → settings → reader → nothing.
- **Removed:** everything only options 1, 2 and 4 used (Material CSS/JS, option 4's shelf, grid,
  mode toggle and stage tokens, option 1's bottom bar, the per-option intros and branches).

Headless smoke tests (Chrome 154, CDP, scratchpad only) drove the flow above; all pass with no
console errors or exceptions, also with `prefers-reduced-motion: reduce` emulated (stage opens and
closes instantly) and with keyboard scrolling of the page (PageDown / ArrowDown / Space scroll it).

## Scrollbar check (§3.2)

- **Chrome 154:** no scrollbar on the page, the galleries, the reader, the sheets, the outline, the
  voice list, the export page or Settings (`page-light` / `page-black` screenshots, taken without
  `--hide-scrollbars`); every overflowing element reports `scrollbarWidth === 'none'`; wheel,
  drag and keyboard scrolling still work; code blocks show the right-edge fade only while cut
  (it disappears at the end of the line).
- **Firefox 150 (headless):** no scrollbar on the page or inside the frames. Firefox blanked all
  text in the frames because the Google Fonts request stays pending with no network (it hides text
  while a web font loads); a scratchpad copy without the `<link>` renders every frame correctly,
  so this is font loading, not the design.

## Checklist (§7)

- [x] One new file `design/2026-10-23-android-quiet-v2.html`; the options file is unchanged; opens
      from disk with no network (fallback fonts); no libraries; no Artifact; git untouched.
- [x] Toolbar: Light / Dark / Black, 412 / 360; solo mode works for every screen ID (the
      screenshot batch used it).
- [x] Every ID in §5 exists with a caption: L1–L6 + R12b; R1, R1c, R1t, R1h, R1s, R2, R3, R4, R4x,
      R5, R6, R7, R7p, R9, R10, R10b, R11, R12; R13, R13p, R13c, R13h, R13t, R13b, R13s, R13m,
      R13k; R8, R8s, R8b, R8c; N1, N2, N2s, N2se, N2L, N3, N4; E1, E1v, E1q, E1x, E2–E5; T1, T1b,
      T2, T2b, T2c; S1, S2, S2s, S2b, S3, S3a–S3d, S4 (69 frames).
- [x] No scrollbar visible anywhere in Chrome; Firefox checked; everything scrolls with wheel,
      drag and keyboard; code blocks fade only when cut.
- [x] No option 1/2/4 leftovers (no Read | Listen toggle, no stage tokens, no Material code, no
      bottom "Open PDF" bar; a grep for `modeseg`, `stageBg`, `libbar`, `appbar`, `surface` finds
      nothing; "Material" survives only in the icon-paths comment).
- [x] Pill: status tap opens the stage; drag, swipe and long-press still work and never also
      trigger the tap (a press over 500 ms or a move over 8 dp is not a tap; the suppressed-click
      flag is cleared right after that gesture's own click, so it can't swallow the next tap).
- [x] Stage: tap, swipe, section jumps, four tool buttons, chevron, ◁ Back with preview, drag-down
      close; returns to the reader at the current sentence (35 %, checked in the test).
- [x] Export: all four choices + switch; footer numbers match §4.1 (checked 1×, 1.5×, Full,
      Smaller in the test and E1q / E1x frames); MP3 shows the badge (the badge sits on the Format
      header, so it shows for both formats); the Impact box is on the page.
- [x] Media cards cut the sentence to one line (`white-space: nowrap` + ellipsis on N1, N2, N2s,
      N2se).
- [x] All colours from tokens (new ones only `--nowBar*`; the hex grep hits the token blocks, the
      bezel and the punch-hole); no text muted with `opacity` (opacity only in the teleprompter
      glide and the stage content fade); 48 dp targets; `aria-label`s on icon-only controls;
      `:focus-visible` outline.
- [x] 360 width: stage, export page (footer + segmented speed), L1 and Now Bar measured headlessly:
      every control inside the screen, no overlaps, segment labels unclipped, current sentence at
      26 px.
- [x] `prefers-reduced-motion` turns off the container transform, glides and fades (CSS rule + JS
      guards; emulated in the test).
- [x] All text from the first brief §10, §4 of this brief, or the mock files. No text from the
      user's books.
- [x] Self-review: a class-name scan finds no unused CSS; no unused JS; no scaffolding.

## Ambiguities resolved (most literal reading; say if you want another)

1. **Kept-list items that became unused were dropped:** `speedChips` (every speed control in v2 is
   the segmented one: R4, R13s, export), option 1's grouped `voiceRows` and `.ghead` (the export
   page uses option 3's flat six-row list, with sample buttons added), `chapterRow`/`chapterList`
   (`chaptersSheet` is now only the R8c empty sheet), Quiet's old L1 rows (`ring48`, `.qrow`,
   `.ring`) since L1 is option 1's layout, option 1's chapter card and E1 rows (`.chcard`,
   `.krow`).
2. **Code fade structure:** the fade `::after` sits on a `div.code` wrapper around the scrolling
   `pre`; an absolute overlay inside the scrolling element itself would scroll away with the code.
3. **Teleprompter:** `.tp` starts at left 4 with 12 padding so the accent bar at x −8 is not
   clipped by `overflow: hidden` (text still starts at 16). "Clipped at the top" for the previous
   sentence = a bottom-aligned flex column with max-height 3 lines.
4. **R13t geometry:** pill play centre (92, H − 64), stage play centre (W/2, H − 228), a 64 circle
   at the midpoint, drawn inside the half-grown rectangle.
5. **Stage stacking:** z-index 18 (above the pill 15, chip 14, hint 16, scrubber 17). The stage's
   scrubber sits above the 88 button (bottom 280, centred). The stage hint sits at bottom 332
   (40 + thumb zone 280 + 12).
6. **Now Bar frames:** the lock screen's lock icon + "Swipe up to unlock" move to bottom 24 so the
   pill at bottom 96 doesn't overlap them.
7. **N2L:** lock icon 16 at y 32–48 (centre 40, between the top bar's chevron and its text);
   "Swipe up to unlock" at bottom 20; thumb zone bottom 64; teleprompter bottom 360 (344 at 360).
8. **Export footer:** sticky at the bottom of the scrolling body with `margin-top: auto` so it is
   pinned when the content is short; the body's bottom padding is 0 and the last card keeps 16
   above the footer (instead of "footer height + 16").
9. **Summaries:** the E2–E5 line and E4's summary are composed from the chosen voice, quality,
   format and speed (the §4 strings for the defaults).
10. **Export switch row** keeps the `menu_book` icon, as in the player sheet.
11. **◁ Back on the stage:** R13b's preview for 400 ms, then the reverse container transform from
    the un-scaled stage.
12. **Drag-down close:** release past 30 % runs the same reverse transform; a shorter drag springs
    back (320 ms).
13. **Not-downloaded model:** its radio is `aria-disabled` with no action; only the text button
    works. The fake download marks the model installed for the session.
14. **Theme `black` in the URL** also darkens the page chrome (same dark page tokens as `dark`).
15. **Pre-existing:** the outline's chapter-4 sticky header (two-line title) overlaps the chapter
    below when the list is scrolled programmatically (R8 / R13k); identical in option 3's
    `android-3-R8-light.png`, left as is.
16. **Fonts in the screenshots:** Roboto isn't installed and the network was blocked, so they show
    the system sans and the local Noto Serif (as before).

## Screenshots (35, device pixel ratio 2, via solo mode, network blocked)

Written to `design/screens/`:

- `android-q2-L1-light.png`, `android-q2-L1-dark.png`, `android-q2-L1-black.png`,
  `android-q2-L1-light-360.png`
- `android-q2-R1-light.png`, `android-q2-R1-black.png`, `android-q2-R1-light-360.png`,
  `android-q2-R1h-light.png`
- `android-q2-R4-light.png`, `android-q2-R4-black.png`
- `android-q2-R13-light.png`, `android-q2-R13-dark.png`, `android-q2-R13-black.png`,
  `android-q2-R13-light-360.png`, `android-q2-R13t-light.png`, `android-q2-R13b-light.png`,
  `android-q2-R13c-black.png`, `android-q2-R13m-black.png`, `android-q2-R13k-light.png`
- `android-q2-E1-light.png`, `android-q2-E1-black.png`, `android-q2-E1q-light.png`,
  `android-q2-E1x-light.png`, `android-q2-E1v-light.png`, `android-q2-E2-light.png`,
  `android-q2-E4-light.png`, `android-q2-E1-light-360.png`
- `android-q2-N1-dark.png`, `android-q2-N2-dark.png`, `android-q2-N2s-dark.png`,
  `android-q2-N2se-dark.png`, `android-q2-N2L-black.png`
- `android-q2-T1-light.png`
- `android-q2-page-light.png`, `android-q2-page-black.png` (window 1400 × 900, top of the page)

How they were made (one frame; 412 → window 432×935, 360 → 380×820; no `--hide-scrollbars`):

```
google-chrome --headless=new --disable-gpu --host-resolver-rules="MAP * ~NOTFOUND" \
  --force-device-scale-factor=2 --window-size=432,935 --virtual-time-budget=3000 \
  --screenshot=design/screens/android-q2-R13-light.png \
  'file:///home/kia/Desktop/learning/lectrice/design/2026-10-23-android-quiet-v2.html?opt=3&theme=light&w=412&screen=R13'
```
