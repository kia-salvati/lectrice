# Lectrice for Android: "Quiet v2" brief for Fable (2026-10-23)

Status: **design decisions made; ready to build.** Opus wrote this (decisions, exact specs). **Fable builds the mockup from
§3–§7 and makes no design decisions of its own.** Where this brief is silent, Fable keeps what option 3 already does in
`design/2026-10-23-android-options.html` ("the options file") and says so in its progress note.

Inputs: the user's feedback of 2026-10-23 (option 3 "Quiet" is the base, five changes), `docs/2026-10-23-android-design-brief.md`
("the first brief"; its §8–§10 tokens, type, components and copy still apply), the options file, `docs/2026-10-23-android-spec.md`
("the spec").

Reading guide for the user: **§1 what changes and why, §2 the lock-screen research, §8 questions.** §3 onwards is for Fable.

---

## 1. The five changes, with the reasons and the decisions I made

### 1.1 No visible scrollbars anywhere

Every scrolling area keeps scrolling (finger, wheel, keyboard, scroll chaining), but no scrollbar is ever drawn: not in
the phone screens, not on the mockup page around them. This matches Android, where scrollbars only flash while you
scroll, and Telegram, which hides them.

Where hiding the bar would hide content, a cue takes its place: **code blocks** (they scroll sideways) get a 24 dp fade on
the right edge while more code is off to the right. Lists that are cut at the bottom already show it (a half row).

### 1.2 Library: option 1's layout in Quiet's style, with Quiet's "Open PDF" button

Option 1's library shows the current book as a **Continue card** with a big play button, then **Recent** rows with cover
tiles and "Continue · 34 %". It answers "what was I doing?" faster than Quiet's flat list. It's drawn with Quiet's rules
(hairlines, no shadows, denser row type, true-black dark), and option 1's full-width bottom "Open PDF" bar is replaced by
Quiet's round **floating button** in the thumb corner (more list visible, same reach).

### 1.3 Reader and player: Quiet's reader plus a **Listen view** (option 4's stage), opened by tapping the pill

**How the pill's gestures fit together** (no two share a trigger):

| On the expanded pill | Does |
|---|---|
| Tap the **status area** ("Ch. 7 · p. 265") | **Opens the Listen view** (new; this was "open chapters") |
| Tap ⏮ / ⏯ / ⏭ | Previous sentence / play-pause / next sentence (unchanged) |
| Tap ⋯ | Player sheet (unchanged) |
| Drag up (anywhere on the pill) | Player sheet follows the finger (unchanged) |
| Swipe left / right ≥ 48 dp | Next / previous sentence (unchanged) |
| Long-press ⏯ (500 ms) | Speed scrubber (unchanged) |

A tap is "finger up, moved less than 8 dp, within 500 ms" (Android's touch slop). Anything that moves more is a drag or a
swipe, so a tap can never be confused with them. The collapsed pill (just a round play button) keeps "tap = play/pause"
and "swipe up = expand"; to open the Listen view from it, you expand first. That keeps the one-thumb play/pause, which is
the collapsed button's only job.

The status area gets a small `open_in_full` icon (instead of ⌃) so the tap has a visible hint, and the one-time hint now
says it.

**Chapters** move off the pill tap: they are one tap away in the Listen view (bottom row) and in the player sheet (the
chapter line, as in option 3). From the plain reader that is two taps (⋯ → chapter line). See question Q3.

**The Listen view** is option 4's stage, restyled in Quiet tokens:

- Same background as the reader (white / dark / true black), no blue tint. The current sentence is large (Noto Serif 28)
  with Quiet's **3 dp accent bar** beside it (the same "you are here" mark as the library and the outline); the sentences
  before and after are smaller, in `fg2`.
- Controls sit in the **bottom 40 %** (thumb zone): chapter progress, ⏮ ⏯ ⏭ (64 / 88 / 64), "Section" jumps, and a row of
  four labelled buttons: **Speed, Sleep, Chapters, More**. "More" opens the same Quiet player sheet (voice, export,
  library, read code aloud…), so the stage has every setting without duplicating any. Long-press ⏯ opens the speed
  scrubber here too, so the gesture means the same everywhere.
- **Tap the text = play/pause** (with a short icon flash); **swipe left/right = next/previous sentence**.
- **Getting back to reading:** a chevron-down button top-left ("Back to reading"), Android's back gesture, or dragging the
  top of the stage down. All three return to the reader **at the current sentence** (follow-scroll on). Predictive back
  shows the stage shrinking down towards the pill while the reader appears behind it.
- Opening is a **container transform**: the pill grows into the stage (320 ms spring), and closing reverses it. This
  tells people where the stage lives (inside the pill).
- No "Read | Listen" toggle (option 4's): the pill tap and back replace it, so there is one way in and one way out.

### 1.4 Export: option 4's design, extended

Option 4 (like option 1) pushes an **Export page inside the player sheet** with a header and back arrow, instead of
Quiet's inline expanding row. It gets four choices and a **live estimate** that stays visible while you pick:

- **Voice** (default: the voice you listen with), **Quality** (Full fp32 / Smaller int8; a model that isn't downloaded
  shows "Download · 126 MB" instead of being selectable), **Format** (M4B / MP3), **Speed** (0.75–2×, default 1×), plus
  the existing "Read code & tables aloud" switch.
- A footer pinned to the bottom of the page: "about 8 h 10 min · about 231 MB", "Takes about 2 h on this phone", and the
  Start button. Length and size follow the speed; time follows the quality. Numbers change with a short cross-fade.

**Impact notes** (also shown in the mockup next to the export frames):

- **MP3 is a spec change.** The spec (§1, §8.4; ffi `ExportFormat { M4b }`) says M4B only. Android's `MediaCodec` has no
  MP3 **encoder** (decoders only). The real choices are **LAME** (LGPL-2.0; the standard, best quality) or **shine**
  (LGPL too, lower quality); there's no permissive MP3 encoder of LAME's quality. MP3 patents expired in 2017, so licence
  is the only issue. LGPL is fine in our MIT/Apache app if LAME is a **separate shared library** (`libmp3lame.so`, loaded
  dynamically, replaceable) with its notice and source offer. **APK size:** about 0.3–0.5 MB for arm64-v8a (UNVERIFIED;
  measure in M4). **Work:** an MP3 `AudioEncoderSink`; parts join by appending frames (simpler than the M4B remux);
  chapters only as ID3 `CHAP` frames, which few players read. About 2 days of work plus a licences entry (estimate).
  My view: M4B stays the default; MP3 only if you want "plays everywhere" (Q1).
- **Speed baked into the file is new for the core.** Today audio is always made at 1× and the player changes speed
  (ExoPlayer's Sonic time-stretch on Android, the browser on the desktop). An export at 1.5× needs a **time-stretch step in
  the export runner** before each part is encoded. Proposed: Sonic's algorithm (Apache-2.0, the same one ExoPlayer uses, so
  the file sounds exactly like in-app listening) behind a small port in the core, so the desktop gets it too (it could
  also use ffmpeg `atempo`). The export record gains `speed`; chapter times are divided by the speed; resume must keep the
  same speed; `start_export(…)` gains `speed` and `model`. Cached 1× clips stay valid and can be reused. (Kokoro's own
  `speed` input would sound slightly more natural but can't reuse the cache; not chosen.)
- **Quality choice:** the core holds one loaded engine. If the export uses the model you are *not* listening with,
  running both at once would need a second engine in memory (about +300 MB RAM for fp32, UNVERIFIED). Decision: in that
  case the export **waits while you listen** and continues when playback stops; the page says so.

### 1.5 Lock screen and media notification: show the current sentence

What real Android can do is in §2. The design: **the sentence becomes the media title**, updated per sentence (each
sentence is already its own media item, so this costs nothing extra), and the chapter and book become the second line.
It's on by default with a Settings switch to show the section title instead (anyone glancing at the phone sees the text).

Plus one optional extra that real Android allows: **the Listen view over the lock screen**, like map navigation (off by
default, Q2).

---

## 2. What Android's media surfaces really show (research)

- **One `MediaSession` feeds everything:** the notification-shade player, the lock-screen player, Samsung's Now Bar,
  Bluetooth car displays, watches. We set metadata and buttons; the system draws the card. We can't change its layout,
  font or number of lines.
- **Text: two short lines.** Android's System UI shows the **title** (`METADATA_KEY_DISPLAY_TITLE`, else `TITLE`) and the
  **artist** (`ARTIST`), each on **one line, cut with "…"**. `subtitle`, `description` and `albumTitle` are not shown on
  phones (cars may show the album). Media3's `MediaMetadata.title` / `.artist` map to these.
- **Per-sentence updates are fine.** Each sentence is one `MediaItem` (spec §7.2), so setting `title = sentence` on each
  item updates the card on every transition (every ~5 s), well under the system's notification rate limit. Pixel
  cross-fades the text; Samsung's behaviour (whether it scrolls a long title) is UNVERIFIED.
- **So a whole sentence never fits:** at 412 dp the title shows about 35–40 characters. The mockup shows exactly that cut.
  A multi-line sentence on the system card is **not possible**; the mockup doesn't pretend it is.
- **Samsung One UI 7+ Now Bar** (S26 Ultra): a pill at the bottom of the lock screen for media and live activities;
  expanded, it shows artwork, title, artist and controls. Media from apps with a MediaSession shows there; that it shows
  ours with no Samsung-specific work is UNVERIFIED (test on the S26 Ultra). The S10+ stays on Android 12 / One UI 4.1:
  the classic lock-screen media card.
- **Android 16 Live Updates are not for media:** `MediaStyle` notifications can't be promoted (only standard, BigText,
  Call and Progress styles). They suit the **export** notification instead (a status-bar chip with progress, Android 16
  QPR1+ with `POST_PROMOTED_NOTIFICATIONS`); not drawn in this round, noted for M4.
- **Fuller text on the lock screen is only possible with our own screen:** an activity with `setShowWhenLocked(true)`
  stays above the lock screen if it was open when the screen went off (how navigation apps do it). That is the optional
  "Listen view over the lock screen" (N2L). Apps can't show themselves over the lock screen on their own (full-screen
  intents are limited to calls and alarms on Android 14+).
- **Mapping we use:** `title` = the sentence (cue items: "Code example." / "Table."); `artist` = "Ch. 7 · The Long Book"
  (no chapters: "p. 12 · Meeting Notes"); `albumTitle` = the book title (for cars); one shared `artworkUri` for the cover
  so it isn't decoded per sentence. With the setting off: `title` = the section title ("7.3 Tables of contents that tell
  the truth"), `artist` = the book title (today's behaviour). **Logging rule:** media items carry book text, so Media3's
  `EventLogger` must never be on in release builds.

Sources: [Android Authority: Live Updates exclude media](https://androidauthority.com/android-16-live-updates-music-player-3573487),
[Create Live Update notifications](https://developer.android.com/develop/ui/compose/notifications/live-update?hl=en),
[9to5Google: One UI 8 Now Bar and third-party apps](https://9to5google.com/2025/07/09/samsung-one-ui-8-now-bar-third-party-apps/),
[SammyFans: One UI 9 media player](https://www.sammyfans.com/2026/06/21/samsung-media-player-gets-5-new-changes-in-one-ui-9-0/amp/),
[Media controls guide](https://developer.android.com/media/implement/surfaces/mobile) (from the first brief).
The one-line title/artist behaviour is from Android's System UI media controls (known behaviour, not re-checked on a device).

---

## 3. Build instructions for Fable: the output

- **One new file:** `design/2026-10-23-android-quiet-v2.html`. Self-contained like the options file: inline CSS, vanilla
  JS, inline SVG icons; the only network asset is the optional Google Fonts link. **Don't change the options file. No
  Artifact, no git, no code outside `design/` and your progress note.**
- **How to start:** copy the options file, then **delete** everything that only options 1, 2 and 4 use, and change what
  §4–§6 say. Keep (as they are unless §4–§6 change them):
  - CSS: page chrome, tokens (base light, base dark, the option-3 black block), phone frame, type, layouts, buttons, rows,
    controls, surfaces, reader, first run / library / settings base, background, export pages, the whole option 3 block,
    and the option 4 `.stage`/`.tp`/`.tz`/`.sbtn` rules (to be restyled per §5.2).
  - JS: `ICONS`, all placeholder content (`BOOKS`, `VOICES`, `BOOK`, `STATS`, `COPY`, …), view helpers, `S1`–`S4`,
    `continueCard`, `recentRow`, `importingRow`, `libTop`, `importErrorSheet`, `docHtml`, `readerText`, `status`, `pill`,
    `backChip`, `slowTip`, `snackbar`, `reader3`, `hintTip`, `scrubber`, `playerSheet3`, `transportRow`, `speedChips`,
    `sleepChips`, `sheetRow`, `voiceRows`, `exportBody`, `exportPage`, `outline3`, `chaptersSheet` (R8c only),
    `mediaCard`, `notif`, `shade`, `N1`–`N4`, `settingsRow`, `segmented`, `T1`, `T2`, `stageSentence`, `stage`, the frame
    registry helpers, the `Live` class, solo mode.
  - Delete: option 2 (`appBar`, `floatingToolbar`, `nowPlaying`, `menu2`, `speedDialog`, `radioSheet`, `chaptersTree`,
    `L1m`, `voiceSheet2`, `sleepSheet2`, `exportSheet2`, its tokens and CSS, the wallpaper switch), option 4's `L1s`,
    `modeSeg`, `.hero`, `.grid`, `.gtile`, `.modeseg`, the stage tokens (`--stage*`), option 1's `libBar`/`.libbar`,
    the `INTRO` for options 1, 2, 4, and every `v.opt === 1|2|4` branch in `Live`.
- Screens keep `data-opt="3"` so the option 3 CSS selectors still apply.
- Screenshots: `design/screens/android-q2-<ID>-<theme>[-360].png` (list in §7). Progress note:
  `docs/progress/android-quiet-v2.md` (what was built, any guess you made, the screenshot list).

### 3.1 Page structure

```
<header class="toolbar">  "Lectrice for Android · Quiet v2 · 2026-10-23"
  Theme: [Light] [Dark] [Black]        (default Light)
  Width: [412] [360]                   (default 412)
<main>
  <div class="intro">   "What changed from option 3" (the 5 headings of §1 as a list, each one sentence from §1)
  <div class="live">    the live phone + "How to try" (§6.3) + ◁ Back under the phone
  <h2>First run</h2> … <h2>Library</h2> … <h2>Reader & player</h2> … <h2>Listen view</h2> … <h2>Chapters</h2>
  <h2>Background</h2> … <h2>Export</h2> (+ the Impact box, §5.5) … <h2>Settings</h2>
  <section id="questions">  copy §8 verbatim
</main>
```

- Same gallery rules as before (zoom 0.5 frames, `ID · name` + one-line note captions).
- **Solo mode:** `?opt=3&theme=black&w=412&screen=R13` renders one frame at zoom 1; `screen=live` the live phone (with
  `&play=1` opening the reader playing, as before). `opt` is accepted and ignored (always 3); `theme` is `light | dark |
  black`.
- **Themes:** `light` = base light, `dark` = base dark (`#121417`…), `black` = the option-3 true-black set (move the
  `.screen[data-opt="3"][data-theme="dark"]` block to `.screen[data-theme="black"]`, with the rest of the dark values
  inherited: write the black block so it sets every dark token too, or apply it after the dark block with both
  attributes). Page chrome: `black` uses the page's dark tokens.

### 3.2 Hidden scrollbars (CSS, in the page's base styles)

```css
/* Scrolling stays; only the bar is hidden. Firefox and Chrome 121+ use scrollbar-width;
   older Chrome/Safari/Edge use the ::-webkit-scrollbar rule. */
html, body, * { scrollbar-width: none; -ms-overflow-style: none; }
*::-webkit-scrollbar { display: none; width: 0; height: 0; }
```

- Remove the existing per-element `scrollbar-width:none` (now global). Don't use `overflow: hidden` on anything that
  scrolls today. Keyboard scrolling (arrows, Page Down, Space) must still work on the page.
- **Code-block fade** (§1.1): `.code` gets `position: relative`; a JS check on render and on `scroll` sets
  `data-more="right"` when `scrollLeft + clientWidth < scrollWidth - 1`. While set, an `::after` overlay (24 px wide, full
  height, `linear-gradient(to right, transparent, var(--bg2))`, `pointer-events: none`, sticky to the right edge) shows.
  It's a background overlay, never text opacity.
- Check in **Chrome and Firefox**: no bar on the page, galleries, reader, sheets, outline, voice list, export page,
  settings; wheel and drag still scroll. Write the result in the progress note (Firefox may not be installed headless;
  if not, say so and check `getComputedStyle(el).scrollbarWidth === 'none'` in Chrome).

---

## 4. Tokens, type and copy

- Tokens: the base light/dark sets and the black set from the first brief §9.1. **No new colour pairs** except the three
  system-drawn colours below (kept in the token block like `--shadeBg`):

| Token | Light | Dark / Black | Use |
|---|---|---|---|
| `--nowBarBg` | `#26303D` | `#26303D` | Samsung Now Bar pill and card (system-drawn, always dark on the lock screen) |
| `--nowBarFg` | `#FFFFFF` | `#FFFFFF` | its text (13.4 : 1) |
| `--nowBarFg2` | `#C9D1DC` | `#C9D1DC` | its second line (8.7 : 1) |

- Type: first brief §9.3 with option 3's denser rows (row title 16/20 500, meta 14/18).
- **Mockup annotations** (not part of the app): a "Spec change" badge = `caption` 500, `warnFg` on `warnBg`, radius 6,
  padding 2 8, 1 px dashed `warnFg` border. Used only where §5 says.
- New copy (verbatim; everything else from the first brief §10.1):

| Where | Text |
|---|---|
| Pill status label (TalkBack) | "Chapter 7, page 265. Open the Listen view" |
| Hint (R1h) | "Tap here for the Listen view. Drag up for more. Long-press play for speed." + "Got it" |
| Stage back button (TalkBack) | "Back to reading" |
| Stage labels | "Speed" (shows the value, e.g. "1×", with "Speed" under it) · "Sleep" (or "28 min" when on) · "Chapters" · "More" |
| Stage section buttons | "Section" with a ≪ icon (`aria-label="Previous section"`) · "Section" with a ≫ icon (`aria-label="Next section"`) |
| Stage hint (R13h) | "Tap the text to pause. Swipe sideways for the next or previous sentence." + "Got it" |
| Stage progress | "38 % of chapter 7" · "9 min left" (preparing: "Preparing audio…") |
| Player sheet export row | "Export audiobook" / "Pick the voice, quality, format and speed" › (running: "Exporting · 42 %") |
| Export headers | "Voice" · "Quality" · "Format" · "Speed" |
| Export quality rows | "Full quality" / "In use · 370 MB" · "Smaller" / "Not downloaded" + text button "Download · 126 MB" · when installed: "Installed · 126 MB" |
| Export quality note (other model picked) | "This export uses the smaller model. While you listen, it waits and continues when you stop." |
| Export format | "M4B" · "MP3"; notes: M4B "One file with chapter marks. Audiobook apps show the chapters." · MP3 "Plays everywhere. Most players don't show chapters." |
| Export speed notes | at 1×: "Normal speed. Your audiobook app can still change it." · other: "Baked into the file: it plays at 1.5× everywhere, at the same pitch." (the chosen value) |
| Export footer | "about 8 h 10 min · about 231 MB" / "Takes about 2 h on this phone" / button "Start export (about 2 h)" |
| Export running summary (E2–E5, under the header) | "Heart · Full quality · M4B · 1×" |
| E4 summary | "8 h 10 min · 231 MB · M4B" (MP3: "… · MP3") |
| Media card (N1/N2/N2s) | title "When the promise is kept, people jump around a long document without fear." (shown cut to one line) · artist "Ch. 7 · The Long Book" |
| N3 export notification | "Exporting The Long Book · 42 %" + second line "Heart · 1× · M4B" |
| T1 new rows (Listening section, after "Read ahead") | "Sentence on the lock screen" / "Shows what's being read. Off: the section title." [switch, on] · "Listen view over the lock screen" / "Keeps the Listen view up when the screen turns off while it's open." [switch, off] |

### 4.1 Export estimate table (the fake model behind the live numbers)

Length and size depend only on speed (AAC/MP3 at 64 kbit/s mono, both ~0.47 MB per minute); time depends only on quality
(the time-stretch is cheap). Voice and format change nothing.

| Speed | Length | Size |
|---|---|---|
| 0.75× | about 10 h 53 min | about 308 MB |
| 1× | about 8 h 10 min | about 231 MB |
| 1.25× | about 6 h 32 min | about 185 MB |
| 1.5× | about 5 h 27 min | about 154 MB |
| 1.75× | about 4 h 40 min | about 132 MB |
| 2× | about 4 h 5 min | about 116 MB |

| Quality | Time to make |
|---|---|
| Full quality | about 2 h |
| Smaller | about 3 h |

The Start button repeats the time: "Start export (about 3 h)".

---

## 5. Screen specs (new or changed)

Measurements in dp = CSS px inside the phone. Status inset 32, gesture inset 24. All frames in light, dark and black.

### 5.1 Library (L1, L2, L3, L3b, L4, L5, R12b)

**L1.** Option 1's `L1` with these changes:
- Header: Quiet's `.qhead` (88 tall incl. status; "Lectrice" 20/24 600; `settings` icon button right).
- Continue card: as option 1 (`bg2`, 1 px `line`, radius 16, padding 16, margin 0 16; caption "Continue listening",
  title `title-m`, "Ch. 7 · Long Documents · p. 265", 4 dp progress + "49 % · 4 h 12 min left", 56 play button right).
  Quiet tweak: a **3 dp `accent` bar** on the card's left edge inside the radius (`::before`, top 16 / bottom 16, radius 2),
  the same "current" mark as Quiet's first row. No shadow.
- "Recent" header and rows as option 1 (40 × 56 cover tile, title, "18 pages · opened 2 hours ago", right column
  "Continue · 34 %" + 56 × 4 bar / "Start"), but row title 16/20 500 and meta 14/18 (Quiet density), row min-height 72,
  divider inset 72.
- **No bottom bar.** Quiet's `.qfab` (56 round, `accent`, `note_add`, right 16, bottom 16 + 24, 1 px `bg` ring,
  `aria-label="Open PDF"`). List bottom spacer 120 so the last row clears it.
**L2 Empty:** Quiet's current L2 (header + centred empty text + FAB). **L3 / L3b:** the new L1 with option 1's
`importingRow` first under "Recent". **L4:** new L1 + `importErrorSheet`. **L5:** new L1 with the warn banner between
the header and the card (margin 0 16 16). **R12b (200 %):** card stacks the play button under the text (as option 1's
R12b); FAB stays 56.

### 5.2 Reader and the pill (R1, R1c, R1t, R1h, R1s, R2, R3, R4, R5, R6, R7, R7p, R9, R10, R10b, R11, R12)

All as option 3, except:
- **Pill status button:** `data-act="listen"` (was `chapters`), `aria-label` per §4, trailing icon `open_in_full` 16 `fg2`
  (was `expand_less`; dropped at 360 as before).
- **R1h:** the new hint copy (§4).
- **R4 player sheet (Quiet's single page):** the export row becomes `sheetRow('download', 'Export audiobook', 'Pick the
  voice, quality, format and speed', chevron_right)` and **pushes the export page** (§5.4) with option 1's
  `pushPage` mechanics (slide in from the right, sheet goes to full). No inline export any more. Everything else
  (segmented speed, inline voice list, inline sleep, read code switch, library) unchanged.
- **New R4x:** R4 while an export runs: the export row subtitle "Exporting · 42 %" with a 4 dp bar (width 120) under it.

### 5.3 Listen view (new group: R13, R13p, R13c, R13h, R13t, R13b, R13s, R13m, R13k)

Rewrite `stage(v, { overlay, state })` with this layout (412 × 915; at 360 × 800 the same structure, numbers in brackets):

- Container `.stage`: `position:absolute; inset:0; background:var(--bg); color:var(--fg)`; status bar on top (no scrim
  needed, nothing scrolls under it).
- **Top bar** at y 32, height 56, flex row: `expand_more` icon button 48 at x 4 (`aria-label="Back to reading"`,
  `data-act="stage-back"`); text block from x 56 to right 16: line 1 `body-s` `fg2` "The Long Book", line 2 16/22 500
  `fg` "7.3 Tables of contents that tell the truth" (1 line, ellipsis). The whole bar is also the **drag-down handle**
  (`data-drag="stage"`).
- **Teleprompter** `.tp`: top 104, left 16, right 16, bottom 336 [bottom 320]. Column, centred vertically, gap 16:
  previous sentence Noto Serif 20/30 400 `fg2` (max 3 lines, clipped at the top); **current** Noto Serif 28/40 500 `fg`
  [26/36] with a **3 dp `accent` bar** at x −8 relative to the text (a `::before` from top 6 to bottom 6, radius 2); next
  sentence 20/30 `fg2` (max 3 lines, clipped at the bottom). Top and bottom 24 dp fades: gradient overlays from `bg` to
  transparent (not text opacity). If the current sentence runs past 6 lines it drops to 24/34. `aria-live="polite"` on
  the current sentence only.
- **Thumb zone** `.tz`: left 16, right 16, bottom 40, column, gap 16:
  1. Chapter progress: 4 dp track `line`, active `accent`, section ticks 2 × 8 `fg2`; row under it `caption` `fg2`
     "38 % of chapter 7" left, "9 min left" right (preparing: right text = 16 spinner + "Preparing audio…").
  2. Transport, centred, gap 24 [16]: prev 64 circle (`bg2`, 1 px `line`, 32 icon `fg`), **play 88** circle (`accent`,
     36 icon `onAccent`; preparing = option 3's R3 ring treatment scaled up), next 64.
  3. Section row: two buttons, flex 1 each, gap 16, height 48, radius full, `bg2` + 1 px `line`, `label` 14/20 500
     `fg`; left = `keyboard_double_arrow_left` 20 + 8 + "Section", right = "Section" + 8 + `keyboard_double_arrow_right`.
  4. Tool row: four buttons, `justify-content: space-around`, each 64 wide × 64 tall: top a 24 icon (or for Speed the
     value text 16/24 500 "1×"), 4 below a `caption` `fg2` label. Speed (`data-act="speed-sheet"`), Sleep `bedtime`
     (when on: icon `accent`, label "28 min"), Chapters `toc`, More `more_horiz` (`data-act="more"`).
- No mode toggle, no stage tokens, no shadows.
- **Code block as the current item (R13c):** option 4's card restyled: `bg2`, 1 px `line`, radius 12, padding 16;
  "Code example" `label`; 3 lines Roboto Mono 14/20 `fg2` (with the right-edge fade if cut); `caption` `fg2`
  "Read as “Code example.”".

Frames:
- **R13** playing. **R13p** paused (play icon; otherwise the same). **R13c** code block current.
- **R13h** first-open hint: `.hintt` style tooltip placed 12 above the thumb zone, copy §4.
- **R13t** transition frame: the reader with the pill **half-grown** into the stage (a `bg` rectangle from the pill's
  rect to full screen at 50 %: left/right 6, top 420, bottom 18, radius 24, 1 px `line`; inside, the 88 play circle
  midway between the pill's play position and its stage position; other stage content not yet drawn).
- **R13b** predictive back: the reader fully drawn behind (playing, highlight at 35 %, pill expanded); the stage on top at
  `scale(.9)`, `transform-origin: 50% 100%`, `translateY(24px)`, radius 32, 1 px `line` border. Caption: "Back swipe:
  release to return to the text at the same sentence."
- **R13s** speed: a small auto-height sheet (`bg2`, top radius 28, handle, header "Speed" + close) with Quiet's 6-segment
  control (`.segc`), over the stage with scrim.
- **R13m** More: the Quiet player sheet (`playerSheet3`) at half detent over the stage.
- **R13k** Chapters: Quiet's full-height outline (`outline3`) over the stage. Choosing an entry closes the sheet and the
  stage shows that entry's first sentence (the user stays in the Listen view).

### 5.4 Export page (E1, E1v, E1q, E1x, E2, E3, E4, E5)

Pushed page inside the player sheet at full detent (option 1's page mechanics). Header 56: `arrow_back` 48 +
"Export audiobook" `title-m`. The page is a column: a scrolling body and a **pinned footer**.

**Body** (padding 0 16, bottom padding = footer height + 16):
1. **Voice** header (`label` 14/20 600 `fg`, padding 16 0 8). A 56 row (radius 12, `bg`, padding 8 12): `record_voice_over`
   24 + 16 + "Heart" `body` 500 / "American English · female" `body-s` `fg2`; trailing `expand_more`. Tap → expands
   inline into option 3's voice rows (6 rows + "All 28 voices"), same radio/selected styles, each with a 48 `volume_up`
   sample button (`aria-label="Hear Bella"`). Picking a voice collapses the list and updates the row.
2. **Quality** header, then two radio rows (min 56, padding 8 0, divider 1 px `line`): radio 24 + 16 + text.
   - "Full quality" / "In use · 370 MB" (selected by default).
   - "Smaller" / "Not downloaded"; radio drawn disabled (`line` ring); trailing text button "Download · 126 MB"
     (`accent`). In the live phone, tapping it shows a 4 dp determinate bar under the row text going 0 → 100 % in 3 s
     with "Downloading · 38 %" style text, then the row becomes "Smaller" / "Installed · 126 MB" and selectable.
   - If the picked model isn't the one in use: the note (§4) appears under the rows, `body-s` `fg2`, with a 20 `info` icon.
3. **Format** header (+ the "Spec change" annotation badge after it, §4). Quiet `.segc` with 2 segments "M4B" | "MP3",
   height 40. Under it the format note `body-s` `fg2` (8 below).
4. **Speed** header. Quiet `.segc` with the 6 speeds (default 1×). Under it the speed note.
5. "Read code & tables aloud" switch row (48), value = the reader's setting.
6. The reassurance card (`bg`, radius 12, `info` icon, `body`), copy from the first brief.

**Footer** (`position: sticky; bottom: 0` inside the sheet, `bg2`, top 1 px `line`, padding 12 16 calc(12 + 24)):
`label` 16/24 500 `fg` "about 8 h 10 min · about 231 MB"; `body-s` `fg2` "Takes about 2 h on this phone";
8 below primary button "Start export (about 2 h)". The two text lines are one `aria-live="polite"` region; on change,
fade out/in 120 ms (0 under reduced motion).

Frames: **E1** defaults (Heart, Full quality, M4B, 1×). **E1v** voice list open. **E1q** both models installed, Smaller
picked (note visible, time "about 3 h"). **E1x** MP3 + 1.5× picked, Smaller not downloaded (footer "about 5 h 27 min ·
about 154 MB", "about 2 h"). **E2–E5:** option 1's page bodies (`exportBody`) under the same header, with the running
summary line (§4) as `body-s` `fg2` right under the header; no footer. E4 uses the E4 summary from §4.

### 5.5 Impact box (page chrome, in the Export gallery)

After the Export frames, an `<aside class="impact">` in page-chrome style (max-width 760, `bg2`-like page token, radius
12, padding 16 24) with the title "Impact (what these choices cost)" and the three bullets of §1.4 "Impact notes"
verbatim (MP3, speed, quality).

### 5.6 Background (N1, N2, N2s, N2se, N2L, N3, N4)

- **N1 shade card / N2 lock-screen card:** `mediaCard` with title = the §4 sentence on **one line with ellipsis** (white-space
  nowrap) and artist line "Ch. 7 · The Long Book". Controls unchanged (prev = previous sentence, next = next sentence,
  no seek bar). Captions: N1 "System-drawn: title = the current sentence (one line), artist = chapter · book. Updates
  every sentence." N2 "Same card on the lock screen (Android 12+ and the S10+)."
- **N2s Samsung Now Bar (One UI 7+, S26 Ultra):** the lock screen (wallpaper, clock, date as N2) **without** the media
  card; at bottom 96 a pill centred, width 340 [300], height 56, radius full, `--nowBarBg`: 40 cover tile (radius 20) at
  left 8, text block 12 right of it: line 1 `label` 14/20 500 `--nowBarFg` sentence (1 line, ellipsis), line 2 `caption`
  `--nowBarFg2` "Ch. 7 · The Long Book"; trailing 48 pause icon button in `--nowBarFg`. Caption: "Samsung's Now Bar shows
  the same title and artist. UNVERIFIED for our app; test on the S26 Ultra."
- **N2se Now Bar expanded:** a card at bottom 96, left/right 16, height 200, radius 28, `--nowBarBg`: 64 tile + the two
  lines (title 1 line, ellipsis) + a row prev / pause (56 circle `--nowBarFg` with `--nowBarBg` icon) / next.
- **N2L Listen view over the lock screen (optional setting):** the stage (R13, black theme in the screenshot) with a 16
  `lock` icon centred in the status bar area (y 40) and, at the very bottom above the gesture handle, `caption` `fg2`
  "Swipe up to unlock" replacing nothing (the thumb zone moves up by 24). Caption: "Real (setShowWhenLocked): appears only
  if the Listen view was open when the screen went off. Off by default."
- **N3:** the export notification with the second line (§4). **N4:** unchanged.

### 5.7 Settings (T1, T1b, T2, T2b, T2c)

T1 as option 3 (rows 48, theme segments "System · Light · Dark · Black") plus the two new switch rows (§4) in the
Listening section after "Read ahead". Rest unchanged.

### 5.8 Unchanged frames (copy from option 3 as they are)

S1, S2, S2s, S2b, S3, S3a–S3d, S4, L6, R1, R1c, R1t, R1s, R2, R3, R5, R6, R7, R7p, R8, R8s, R8b, R8c, R9, R10, R10b,
R11, R12, N4, T2, T2b, T2c (they pick up the new pill icon and the hidden scrollbars automatically).

---

## 6. The live phone

### 6.1 Starts on L1. Interactions (all from option 3 unless marked new)

1. Continue card play → reader playing; card body or a Recent row → reader paused. FAB → reader (fake import).
2. Reader: 2.5 s playback with code-cue skipping, follow-scroll at 35 %, tap a sentence, manual scroll → "Back to
   current" chip, collapse on scroll-down / 4 s idle, expand on scroll-up / swipe up on the button.
3. Pill: ⏮ ⏯ ⏭, ⋯ → player sheet, drag up → sheet follows the finger, swipe ± sentence (120 ms nudge), long-press ⏯ →
   scrubber. **New: tap the status → Listen view** with the container transform (§6.2).
4. **New, Listen view:** tap the teleprompter → play/pause + 600 ms flash; swipe ≥ 48 dp left/right → next/previous with
   the 320 ms glide; section buttons jump; long-press ⏯ → scrubber; Speed → R13s sheet; Sleep → small sheet with
   sleep chips; Chapters → outline (jump stays in the stage); More → player sheet (export page etc. work there too);
   chevron-down or ◁ Back → closes (◁ Back first shows R13b's preview for 400 ms); **drag the top bar down** → the
   stage follows the finger (translateY, scale from 1 to .9 over 300 px of drag); release past 30 % of the height →
   close, else spring back. The first open shows the R13h hint once.
5. On closing the stage: reverse container transform into the pill; the reader is already scrolled so the current
   sentence sits at 35 % (follow on).
6. **New, export page:** every choice updates the footer per §4.1; voice list expands/collapses; "Download · 126 MB"
   runs the 3 s fake download; Start → E2 animating 42 → 100 % over 6 s → E4. Back arrow pops the page.
7. ◁ Back order: scrubber → export page → sheet → Listen view (with preview) → settings → reader (R11 preview) → nothing.
8. Library gear → Settings; T1 → T2 via the model row.

### 6.2 Container transform (pill ↔ stage)

Measure the pill's rect; insert the stage with `clip-path: inset(top right bottom left round 28px)` set to the pill's
rect, then transition to `inset(0 round 0)` over 320 ms `cubic-bezier(.2,.9,.25,1.05)`. Stage content (top bar,
teleprompter, tool rows) fades in from 120 ms to 320 ms; the play circle animates from the pill's play position/size to
88 (FLIP: `transform` translate + scale). Closing runs it backwards. Under `prefers-reduced-motion` it is instant.

### 6.3 "How to try" list (beside the live phone)

1. Tap the play button on "The Long Book".
2. Tap "Ch. 7 · p. 265" on the pill to open the Listen view.
3. Tap the big text to pause; swipe it sideways to change sentence.
4. Tap More → Export audiobook; change the speed and quality and watch the estimate.
5. Tap the chevron (or ◁ Back) to return to the text at the same sentence.
6. Scroll the text down to shrink the pill; drag the pill up for the player; long-press play for speed.
7. Switch Theme to Black.

---

## 7. Checklist and screenshots

- [ ] One new file `design/2026-10-23-android-quiet-v2.html`; the options file is unchanged; opens from disk with no
      network (fallback fonts); no libraries; no Artifact; git untouched.
- [ ] Toolbar: Light / Dark / Black, 412 / 360; solo mode works for every screen ID with `?opt=3&theme&w&screen`.
- [ ] Every ID in §5 exists with a caption: L1–L6, R12b, R1–R12 (+ R1c, R1t, R1h, R1s, R4x, R7p, R10b), R13 group,
      R8/R8s/R8b/R8c, N1–N4 (+ N2s, N2se, N2L), E1/E1v/E1q/E1x/E2–E5, T1/T1b/T2/T2b/T2c, S1–S4 group.
- [ ] **No scrollbar visible anywhere** (page and phones) in Chrome; Firefox checked or the fallback check reported;
      everything still scrolls with wheel and drag; code blocks show the right-edge fade only when cut.
- [ ] No option 1/2/4 leftovers (no Read|Listen toggle, no stage tokens, no Material code, no bottom "Open PDF" bar).
- [ ] Pill: status tap opens the stage; drag/swipe/long-press still work and never also trigger the tap.
- [ ] Stage: tap, swipe, section jumps, four tool buttons, chevron, ◁ Back with preview, drag-down close; returns to the
      reader at the current sentence.
- [ ] Export: all four choices + switch; footer numbers match §4.1 for every combination; MP3 shows the badge; the
      impact box is on the page.
- [ ] Media cards cut the sentence to one line (no multi-line titles anywhere on system surfaces).
- [ ] All colours from tokens (new ones only `--nowBar*`); no text muted with `opacity`; 48 dp targets; `aria-label`s on
      icon-only controls; `:focus-visible` outline.
- [ ] 360 width: stage, export page (footer + segmented speed), L1, Now Bar fit with no overlap or clipping.
- [ ] `prefers-reduced-motion` turns off the container transform, glides and fades.
- [ ] All text from the first brief §10, §4 here, or the mock files. **No text from the user's books.**
- [ ] Self-review: no unused CSS/JS, no scaffolding.

**Screenshots** (device pixel ratio 2, via solo mode, network blocked as before), `design/screens/android-q2-…png`:
- `L1-light`, `L1-dark`, `L1-black`, `L1-light-360`
- `R1-light`, `R1-black`, `R1-light-360`, `R1h-light`
- `R4-light`, `R4-black`
- `R13-light`, `R13-dark`, `R13-black`, `R13-light-360`, `R13t-light`, `R13b-light`, `R13c-black`, `R13m-black`,
  `R13k-light`
- `E1-light`, `E1-black`, `E1q-light`, `E1x-light`, `E1v-light`, `E2-light`, `E4-light`, `E1-light-360`
- `N1-dark`, `N2-dark`, `N2s-dark`, `N2se-dark`, `N2L-black`
- `T1-light`
- `page-light` and `page-black`: the mockup page itself (window 1400 × 900, top of the page) to show there are no
  scrollbars.

List every file written in `docs/progress/android-quiet-v2.md`.

---

## 8. Questions for the user

1. **MP3 export on the phone:** add it (a spec change: LGPL LAME as a separate library, about 0.3–0.5 MB, about 2 days,
   chapters mostly lost), or keep M4B only? The mockup shows both so you can see it.
2. **Listen view over the lock screen** (N2L): want it as an optional setting (off by default)?
3. **Chapters from the reader** are now two taps (⋯ → chapter line), because tapping the pill opens the Listen view. OK,
   or should I give chapters back a one-tap spot (for example, swap the ⋯ button for a chapters button and rely on
   drag-up and the Listen view's "More" for the player sheet)?
