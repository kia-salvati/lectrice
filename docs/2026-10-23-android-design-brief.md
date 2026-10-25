# Lectrice for Android: UI/UX options and build brief for Fable (2026-10-23)

Status: **options for the user to choose from.** Nothing is built. Opus wrote this (research, ideas, decisions, exact
specs); **Fable builds the HTML mockup from §8–§11 and makes no design decisions of its own.** Where this brief is silent,
Fable copies the web app (`frontend/src`) and says so in its progress note.

Inputs: `docs/2026-10-23-android-spec.md` ("the spec"), `research/2026-10-23-android-options.md` §4.1 (Telegram's
principles), `docs/2026-10-23-android-m1-spike-plan.md` §6 (speed bands), `design/2026-10-06-reader-options.html`
(option D, chosen and called "perfect"), `design/2026-10-23-chapter-options-v2.html` + `docs/2026-10-23-chapters-brief.md`
(chapters: take 1 + label A), `frontend/src/styles/tokens.css`, `frontend/src/components/MiniPlayer/*`.

Reading guide for the user: §1–§3 are the shared facts and the research; **§4 the four options, §5 my recommendation,
§6 your questions.** §7 onwards is the build spec for Fable.

---

## 1. Fixed facts the design must respect

From the decision log and the spec (not up for debate in this brief):

- Kotlin + Jetpack Compose; Telegram's **principles** (one screen host, our own small theme, nothing heavy on the main
  thread, few dependencies, tuned springs, measured smoothness), never its code.
- Everything on the phone. The voice model is a **one-time download the user picks**: Full quality (fp32, ~370 MB) or
  Smaller (int8, ~126 MB). Nothing is pre-selected. Import from a file is possible for offline installs.
- Reading works without the voice (the voice is only needed to listen).
- **Export on Android is M4B only** (spec §1: Android has no MP3 encoder; LAME is LGPL). The task note said "MP3/M4B";
  the spec wins, and the mockup shows M4B only.
- Lock-screen / Bluetooth "next" = **next sentence**; "previous" = previous sentence (symmetry, spec §7.1).
- Our own colours (spec §10): the web tokens, light and dark. No wallpaper colours in the spec (option 2 challenges this;
  question Q2).
- Touch targets **48 dp** minimum even when the visible icon is smaller (spec §10).
- Chapters on the phone: a **bottom sheet** (no rail). Label rule from the chapters brief §3 ("Ch. 7" from the title's own
  number; current chapter = first chapter-like title on the path).
- Slow phones: the S10+ may be slower than real time. Spike bands: RTF ≤ 0.6 green, 0.6–0.9 amber, > 0.9 red. "Prepare
  while charging" exists **only if** the spike says so, so the mockup shows it in a frame clearly captioned as conditional.
- Public repo: **no text from the user's books.** All placeholder text is ours (§10).

## 2. Research notes (short)

Terms, explained once:

- **dp / sp:** Android's size units. 1 dp is one "density-independent pixel" (a 412 dp wide screen is 412 CSS px in the
  mockup). **sp** is dp for text that also grows with the user's font-size setting.
- **Edge-to-edge:** the app draws behind the status bar (top) and the gesture bar (bottom) and pads its own content by
  the **insets** (the sizes of those bars). Enforced for apps targeting Android 15+ (API 35) and the opt-out is gone at
  API 36. So every screen must look right with text scrolling under a see-through status bar.
- **Predictive back:** when the user starts the back swipe, the app shows a live preview of where back leads (the current
  screen shrinks and slides, revealing the one behind). Releasing commits; sliding back cancels. Default-on for apps that
  target API 36. Our screens and sheets must each have a sensible "back preview".
- **Bottom sheet:** a panel that slides up from the bottom. **Modal** = dims the rest of the screen (a **scrim**) and
  blocks it; **detents** = the heights it snaps to (e.g. half and full).
- **Dynamic colour ("Material You"):** Android 12+ can derive an app's palette from the wallpaper.
- **Material 3 Expressive** (Google, 2025): adds **floating toolbars** (a pill of actions floating over content, which
  is close to our option D pill), **button groups** whose buttons change shape when pressed, a bouncier spring motion
  scheme, and deprecates the old bottom app bar. Useful because our pill is now an "official" pattern, not an oddity.
- **Media3 `MediaSession`:** the one object that drives the notification player, the lock screen, Bluetooth buttons and
  Android Auto. Its **media button preferences** decide which buttons appear (back slot, forward slot, overflow). The
  system draws the card; we only choose title, subtitle, artwork, buttons and whether a seek bar appears.
- **TalkBack:** Android's screen reader. Every control needs a spoken label; changing text (our status) is announced
  through a **live region**.

What good apps do (from knowledge of the apps plus light reading; no installs on the metered network):

| App | What to take | What to avoid |
|---|---|---|
| Speechify | Tap a sentence to jump; highlight follows; speed always one tap away | Busy upsell chrome, a large fixed player that eats a third of the text |
| Voice Dream Reader | Sentence/paragraph navigation, sleep timer, "end of chapter" | Dense toolbars of tiny icons |
| Pocket Casts / Libby | Mini player that expands into a full player; chapter list; sleep timer with "end of chapter"; chapter ticks on the progress bar | A second "now playing" world that hides the content |
| Kindle / Moon+ Reader | Chrome disappears while reading; a thin location line; big typography controls | Tap-zone page turns (conflict with our tap-to-jump) |
| Telegram | Instant start, sheets that follow the finger, a "jump to latest" floating button, dense but calm lists, true-black night theme | Custom everything (GPL, and too costly for us) |

One-hand reach: on a 6.4–6.9" phone the comfortable thumb zone is the **bottom third, centre to the thumb's side**. Top
corners are a stretch. So: primary actions at the bottom (pill, "Open PDF", sheet buttons), nothing important only in
the top bar.

Accessibility targets: text ≥ 4.5:1 contrast (measured; table in §9.2), 48 dp targets, layouts that survive **200 %
font scale** (Android 14+ allows it), TalkBack labels, `prefers-reduced-motion` respected in the mockup.

Sources (light web check): [Compose Material 3 package (FloatingToolbar)](https://developer.android.com/reference/kotlin/androidx/compose/material3/package-summary),
[M3 Expressive toolbars](https://9to5google.com/2025/05/18/material-3-expressive-toolbars/),
[Media controls guide](https://developer.android.com/media/implement/surfaces/mobile),
[MediaSession reference](https://developer.android.com/reference/androidx/media3/session/MediaSession),
[DefaultMediaNotificationProvider](https://developer.android.com/reference/androidx/media3/session/DefaultMediaNotificationProvider),
[Speechify mobile review](https://whatplugin.ai/reviews/my-take-on-speechifys-mobile-app),
[Speechify review/tutorial](https://dhruvirzala.com/?p=3906).

## 3. The app flow (same in every option)

```
First run:  Welcome ─► Choose model ─► (metered? dialog) ─► Downloading ─► Ready (hear a sample) ─► Library
                 └─ "Not now, just read" ──────────────────────────────────────────────────────► Library (banner)
Library ─► Open PDF (system picker) ─► Importing row ─► Reader          (already in library ─► snackbar + Reader)
Library ─► Continue ─► Reader at saved place
Reader  ─► pill: play / prev / next / status(=chapters) / more(=player sheet)
Player sheet ─► Voice page · Sleep timer · Export page · Chapters · Library
Background: notification card, lock screen, export and download notifications
Settings (from Library) ─► Voice model page
```

Screens and states (IDs used in §11; each option renders all of them in its own style):

| Group | IDs |
|---|---|
| First run | S1 Welcome · S2 Choose model · S2b Metered dialog · S3 Downloading (+ states S3a paused, S3b waiting for network, S3c damaged/retry, S3d storage full) · S4 Ready |
| Library | L1 Library · L2 Empty · L3 Importing · L4 Import error · L5 Voice-not-installed banner · L6 Already-in-library snackbar |
| Reader | R1 Playing · R2 Scrolled away (back-to-current) · R3 Preparing · R4 Player sheet · R5 Voice page · R6 Sleep timer · R7 Slow phone · R8 Chapters sheet (+ R8b drilled in, R8c no chapters) · R9 Voice failed · R10 Notification permission · R11 Predictive back preview · R12 200 % font |
| Background | N1 Notification card · N2 Lock screen · N3 Export notification · N4 Download notification |
| Export | E1 Ready to start · E2 Running · E3 Paused by Android · E4 Done · E5 Failed |
| Settings | T1 Settings · T2 Voice model |

Copy that is the same everywhere is in §10.

---

## 4. The four options

### Option 1 "Pill": a faithful port of option D

**Idea.** The web reader you chose, adapted to a thumb. Text fills the screen, no top bar. One floating pill at the
bottom: previous · play · next · a two-line status ("Ch. 7 · p. 265" / "Long Documents") · more. Tapping the status opens
the chapters sheet; "more" opens the player sheet (the web popover as a bottom sheet). A small "Back to current" chip
appears when you scroll away, like Telegram's "jump to latest" button. Library is the web's list with a "Continue" card on
top and the "Open PDF" button at the bottom.

**Suits:** you, if the web app should feel like the same product in your pocket. People who read along while listening.

**Pros:** same product on both platforms (one design language, one set of tokens); least chrome on the reading screen;
closest to spec §11 (cheapest to build for M2); the pill is now a standard Material pattern (floating toolbar).
**Cons:** speed and voice are one tap deeper than in Speechify; the pill always covers ~100 dp of text (the reader pads
for it); no big "now playing" screen for eyes-free listening.

### Option 2 "Material": Material 3 Expressive, the Android-native look

**Idea.** Look like a Google app. Library with a large collapsing title, book tiles, and a floating "Open PDF" button.
Reader with a top app bar (back, title, "Ch. 7 · p. 265" subtitle, chapters, overflow) that hides on scroll, and a
coloured floating toolbar at the bottom (prev · play · next · speed · expand). "Expand" opens a full-screen **Now
playing** screen (big cover tile, scrubber with chapter ticks, large transport buttons, speed / sleep / voice / chapters).
Chapters is an expandable tree. Optional **wallpaper colours**.

**Suits:** people who want the app to look like the rest of their phone; people used to Pocket Casts or Spotify.

**Pros:** instantly familiar; all Material widgets, so less custom code; a real full-screen player for listening without
reading. **Cons:** more chrome, less text on screen; two places show the same controls (toolbar and Now playing); looks
less like the web app; wallpaper colours can clash with the yellow highlight and make contrast harder to guarantee.

### Option 3 "Quiet": Telegram's principles taken literally

**Idea.** Speed and calm through gestures. While you read or listen and scroll down, the pill shrinks to a single 56 dp
play button in the thumb corner plus a 2 dp progress line along the bottom edge; scroll up and the full pill comes back.
The pill takes gestures: **drag it up** and the player sheet follows your finger; **long-press play** for a speed
scrubber. Library is a dense Telegram-style list with progress rings. Chapters is one full-height searchable outline with
sticky chapter headers. Dark theme is **true black** (both test phones have OLED screens). No shadows, hairlines only.

**Suits:** heavy daily use, one-handed, people who like Telegram's feel.

**Pros:** the most text on screen; fastest to operate once learned; search scales to 390-entry books (GoF).
**Cons:** gestures are invisible (need a one-time hint and TalkBack alternatives); more custom gesture code (more to
test on two phones); a collapsed pill hides the chapter label until you scroll up.

### Option 4 "Stage": listen-first, my bolder idea

**Idea.** Two modes for one book, switched by a "Read | Listen" toggle at the top. **Read** = option 1's reader. **Listen**
= a stage: a calm full-screen view where the current sentence is large (28 sp) in the middle, the previous and next ones
smaller above and below, and the sentences glide up like a teleprompter. The bottom 40 % is giant controls for the thumb:
previous / play (88 dp) / next, "‹ Section" and "Section ›" jumps, speed, sleep, voice, chapters. Tap anywhere on the
stage = play/pause; swipe left/right = next/previous sentence (eyes-free on a walk). Library is a shelf: the current book
as a big hero with a play button, then a two-column grid of generated title covers.

**Suits:** listening on the move (walking, commuting, cooking); people who glance rather than read.

**Pros:** the best eyes-free and one-hand experience; a strong identity; works great with the lock screen (same idea).
**Cons:** the most to build (a second reader mode); two modes can confuse; the grid library shows fewer books per screen;
code blocks and tables need a special stage card.

### Side by side

| | 1 Pill | 2 Material | 3 Quiet | 4 Stage |
|---|---|---|---|---|
| Text visible while playing (412 × 915) | ~760 dp | ~650 dp | ~860 dp (collapsed) | read mode ~760 dp / listen mode n/a |
| Taps to change speed | 2 (more → chip) | 1 (toolbar) | 1 (long-press play) | 1 (stage) |
| Chapter label visible | always (pill) | always (app bar, hides on scroll) | when pill expanded | always (stage) |
| Feels like the web app | yes | partly | mostly | read mode yes |
| Build cost (relative) | 1.0 | 1.1 | 1.4 | 1.7 |
| Biggest risk | covers text bottom | chrome-heavy, contrast with dynamic colour | invisible gestures | two modes |

## 5. My recommendation

**Option 1 "Pill" as the base, plus three borrowed pieces:**

1. From option 3: the sheet **follows the finger** when you drag the pill up (Telegram's smoothness, cheap in Compose),
   and the true-black dark theme as a setting (if you want it, Q3). Not the auto-collapsing pill: hiding the chapter label
   costs more than the ~100 dp it saves.
2. From option 4: the **"‹ Section / Section ›" jumps** in the player sheet. The full Listen stage is a good v1.1 idea
   once you've lived with v1; it fits on top of option 1 without changing it.
3. From option 2: nothing visual, but the platform rules (edge-to-edge, predictive back, Material widgets for slider,
   switch and sheet) apply to every option anyway.

Why: you called D "perfect", and option 1 keeps one product across web and phone with one token set; it is closest to
the spec, so M2 has the least risk; the pill is now a standard Android pattern, so it won't feel foreign. The rest of the
options are there so you can see what you'd be giving up.

## 6. Questions for you

1. **Which option** (or which mix)? If a mix, which pieces from the others?
2. **Wallpaper colours** (Material You): keep our own colours only (spec), or offer them as an opt-in setting? I'd say no.
3. **True-black dark theme** for OLED (saves a little battery at night): add it as a third theme choice?
4. **Sleep timer:** I suggest yes for v1 (Off, 15, 30, 45 min, 1 h, End of chapter). It's cheap and every listening app
   has it. OK?
5. **Covers:** generated title tiles (no core change), or a picture of the PDF's first page (a small core change: render
   page 1 with pdfium)? The cover also shows on the lock screen.
6. **Pill status opens the chapters sheet** (saves a button; my proposal), or keep a separate chapters button as in the
   spec? The status would show a small "⌃" hint.
7. **Lock screen without a seek bar:** each media item is one sentence (~5 s), so a scrubber there means nothing. Hide
   it? (Proposed: yes.)
8. **Section jumps** ("‹ Section / Section ›") in the player sheet: want them?
9. **Text size:** only in Settings, or also an "Aa" control in the player sheet?
10. **Tablets/foldables:** phone layout with the text column centred at max 640 dp for v1. OK?
11. **"Not now, just read"** on the first-run screen (use the app without the voice): keep it?

---

## 7. Build instructions for Fable: the output

- **One file:** `design/2026-10-23-android-options.html`. Local, self-contained: inline CSS, inline JS, inline SVG icons.
  The only allowed network asset is **Google Fonts** (optional: the page must still look right with the fallbacks).
  **Never a published Artifact. Don't touch git, `frontend/` or any code.**
- No libraries. Plain HTML + CSS custom properties + vanilla JS.
- Screenshots go to `design/screens/android-*.png` (list in §12). Write a short progress note to
  `docs/progress/android-design-options.md` (what was built, any place you had to guess, the screenshot list).

### 7.1 Page structure

```
<header class="toolbar">  (sticky, 56 px, page chrome — not part of the phone design)
  "Lectrice for Android · design options · 2026-10-23"
  Option tabs: [1 Pill] [2 Material] [3 Quiet] [4 Stage]         (role=tablist; default 1)
  Theme: [Light] [Dark]                                          (default Light)
  Width: [412] [360]                                             (default 412)
  Option 2 only: [Wallpaper: Blue | Green]                       (dynamic-colour demo)
</header>
<main>
  <section class="option" id="opt-N">             (one visible at a time)
    <div class="intro">  idea · suits · pros · cons  (copy §4 text for that option verbatim)
    <div class="live">   ONE interactive phone (§11.x "Live phone") + a "How to try" list beside it
                         (stacks under it below 1100 px page width)
    <h2>First run</h2>   <div class="gallery"> frames S1 … S4 </div>
    <h2>Library</h2>     L1 … L6
    <h2>Reader & player</h2>  R1 … R12
    <h2>Chapters</h2>    R8, R8b, R8c
    <h2>Background</h2>  N1 … N4
    <h2>Export</h2>      E1 … E5
    <h2>Settings</h2>    T1, T2
  </section>
  <section id="recommendation"> copy §5 and §6 verbatim (always visible at the bottom) </section>
</main>
```

- The page itself (outside phones) uses the web tokens (light/dark following the Theme toggle), `system-ui` 16 px,
  max-width 1400 px, 24 px padding. It is not part of the design.
- **Gallery frames** are static states, rendered at **zoom 0.5** (`zoom: .5` on the frame wrapper), in a CSS grid
  `repeat(auto-fill, minmax(230px, 1fr))`, gap 24 px. Each frame has a caption under it: `ID · name` (14 px, 600) and a
  one-line note (13 px), e.g. "R7 · Slow phone — only shown when synthesis falls behind".
- **Solo mode for screenshots:** `?opt=1&theme=dark&w=412&screen=R4` renders only that one frame at zoom 1 on a plain
  page-background, no toolbar, so a headless browser can screenshot it. `screen=live` renders the live phone alone.
- State lives in the URL hash or query only; no `localStorage` needed.

### 7.2 The phone frame

- Screen: **412 × 915** CSS px (= dp) by default; **360 × 800** when Width = 360. `overflow: hidden`, `position: relative`.
- Bezel: 10 px solid `#0B0B0C` border, `border-radius: 44px` outside / 34 px inside (screen corners), no notch drawing
  except a 12 px black circle centred 14 px from the top (punch-hole camera).
- **Status bar** (top inset): 32 px tall, transparent, drawn over the content. Left: "9:41" Roboto 14/20 500. Right:
  three 16 px icons (signal, wifi, battery) as simple SVG. Colour = the option's `fg`. Content scrolling underneath gets a
  **status scrim**: the screen's background colour at 94 % alpha, 32 px tall (opacity on a background is fine; never on text).
- **Gesture bar** (bottom inset): 24 px tall area, transparent; a 108 × 4 px handle, radius 2, centred, 10 px from the
  bottom, colour `fg2`.
- Under each frame (outside the bezel) in the live phone only: a small row of fake system buttons for the mouse:
  `◁ Back` (= system back / predictive back commit). Gallery frames don't need it.

---

## 8. Shared foundations

### 8.1 Fonts

Google Fonts link (one `<link>`): **Noto Serif** (400, 400 italic, 600, 700), **Roboto** (400, 500, 700),
**Roboto Mono** (400). Fallbacks: serif → `"Noto Serif", Georgia, "Times New Roman", serif`; sans → `Roboto,
system-ui, sans-serif`; mono → `"Roboto Mono", ui-monospace, monospace`.
(Why these: Android phones ship Roboto and Noto Serif, so the mockup shows what the real app will render.)

### 8.2 Spacing, radii, elevation, motion

- **Spacing scale (dp):** 4, 8, 12, 16, 24, 32, 48. Screen gutter **16**. Nothing uses other values except where a
  component spec below says so.
- **Radii:** `r-xs 6` (page chip), `r-s 8` (code block, speed chips, snackbar), `r-m 12` (cards, banners), `r-l 16`
  (hero card, FAB in option 2), `r-xl 28` (sheet top corners, dialogs), `r-full 999` (pill, round buttons).
- **Elevation:** options 1, 3, 4 use **no shadows**: separation by `bg2` + 1 px `line` border. Option 2 uses Material
  tonal surfaces plus two shadows only: `e-2: 0 1px 2px rgba(0,0,0,.30), 0 2px 6px 2px rgba(0,0,0,.15)` (FAB, floating
  toolbar) and `e-3: 0 1px 3px rgba(0,0,0,.30), 0 4px 8px 3px rgba(0,0,0,.15)` (Now playing mini bar). In dark, option 2
  drops the shadows and relies on the tonal colour.
- **Scrim** behind modal sheets and dialogs: light `rgba(28,31,36,.32)`, dark `rgba(0,0,0,.50)` (all options).
- **Motion** (fake Compose springs): sheets and screen pushes use `transition: transform 320ms cubic-bezier(.2,.9,.25,1.05)`
  (slight overshoot = spring); fades 150 ms linear; highlight change = background cross-fade 120 ms. Under
  `@media (prefers-reduced-motion: reduce)` every transition is 0 ms and the teleprompter (option 4) jumps.

### 8.3 Icons

Inline SVG, 24 × 24 viewBox, drawn in `currentColor`, Material Symbols **Rounded** look (copy Material Symbols path data,
Apache-2.0, if you have it; otherwise draw simple equivalents). Needed: `skip_previous`, `play_arrow`, `pause`,
`skip_next`, `more_horiz`, `more_vert`, `toc` (list), `expand_less` (⌃), `arrow_back`, `close`, `chevron_right`,
`chevron_left`, `check`, `settings`, `note_add` (open PDF), `my_location`, `arrow_downward`, `arrow_upward`, `bedtime`
(sleep), `record_voice_over` (voice), `speed`, `ios_share` (share), `download`, `pause_circle`, `error`, `warning`,
`wifi`, `signal_cellular_alt`, `battery_full`, `sd_storage`, `check_circle`, `search`, `open_in_full`, `menu_book`,
`headphones`, `volume_up` (sample), `first_page`/`last_page` (section jumps: use `keyboard_double_arrow_left/right`).

### 8.4 Shared reader typography (all options)

| Element | Font | Size / line height (sp) | Weight | Colour |
|---|---|---|---|---|
| Paragraph | Noto Serif | 19 / 32 | 400 | `fg` |
| Heading level 1 (chapter) | Noto Serif | 26 / 34 | 600 | `fg`; margin 48 top, 16 bottom |
| Heading level 2 (section) | Noto Serif | 22 / 30 | 600 | `fg`; margin 32 top, 12 bottom |
| Heading level 3 | Noto Serif | 19 / 28 | 700 | `fg`; margin 24 top, 8 bottom |
| Page chip ("p. 265") | Roboto | 12 / 16 | 400 | `fg2`, 1 px `line` border, `r-xs`, padding 0 6, height 20, inline, `vertical-align: 2px`, margin-right 6, not selectable (`user-select: none`), `aria-label="Page 265"` |
| Code block | Roboto Mono | 14 / 20 | 400 | `fg` on `bg2`, `r-s`, padding 12, `overflow-x: auto`, `white-space: pre`, margin 0 0 16 |
| Skipped cue under code/table | Roboto | 13 / 18 | 400 | `fg2`, text "Read as “Code example.”" |
| Current sentence | inherits | inherits | inherits | `hl` background, `hlFg` text, radius 4, padding 0 2 px, `box-decoration-break: clone` |

Paragraph spacing 16. Text column: left/right padding 16; top padding = 32 (status) + 24; bottom padding = space for
the bottom controls + 48.

---

## 9. Colour tokens

### 9.1 Token sets

**Base set (options 1, 3, 4).** The spec §10 tokens plus five new pairs (marked new). CSS names in brackets.

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg` | `#FFFFFF` | `#121417` | screen background |
| `bg2` | `#F3F3EF` | `#1C1F24` | pill, sheets, cards, chips |
| `fg` | `#1C1F24` | `#E8E6E1` | text, icons |
| `fg2` | `#5B636D` | `#A8ADB5` | secondary text |
| `line` | `#D3D3CD` | `#363A41` | borders, dividers, slider track |
| `accent` | `#1D5FAE` | `#8AB8FF` | play button, links, current-chapter bar, focus |
| `onAccent` | `#FFFFFF` | `#0B1A33` | on `accent` |
| `hl` / `hlFg` | `#FFE58F` / `#1C1F24` | `#4D4214` / `#FFF6D8` | current sentence |
| `errBg` / `errFg` | `#FBE7E5` / `#8B1A12` | `#44181A` / `#F4B1AD` | errors |
| new `accentContainer` / `onAccentContainer` | `#DCE8F7` / `#0E3A6E` | `#22344F` / `#D6E6FF` | selected chips, selected voice row, buffered part of the slider |
| new `warnBg` / `warnFg` | `#FFF1D0` / `#6B4A00` | `#3A2E0E` / `#F5D27A` | warnings (metered network, slow phone, paused by Android) |
| new `okFg` | `#1E6B35` | `#8FD6A3` | "done" icons and text on `bg` |
| new `inverseBg` / `inverseFg` | `#1C1F24` / `#FFFFFF` | `#E8E6E1` / `#121417` | snackbar |

**Option 3 dark = true black** (replaces the dark column for option 3 only): `bg #000000`, `bg2 #121212`,
`fg #E6E3DD`, `fg2 #A3A3A3`, `line #2A2A2A`, `accent #8AB8FF`, `onAccent #0B1A33`, `hl #3D3410`, `hlFg #FFF3CC`; the rest
as base dark.

**Option 4 stage tokens** (the Listen view only; Read view uses the base set):

| Token | Light | Dark |
|---|---|---|
| `stageBg` | `#EAF1FB` | `#0E1B2E` |
| `stageFg` (current sentence) | `#0E1B2E` | `#FFFFFF` |
| `stageFg2` (neighbour sentences, labels) | `#4A5A70` | `#9FB0C7` |
| `stageCtrlBg` (secondary buttons) | `#D9E4F4` | `#1B2B42` |
| `stageAccent` / `stageOnAccent` | `#1D5FAE` / `#FFFFFF` | `#8AB8FF` / `#0B1A33` |

Option 4 **cover tile hues** (library grid, lock-screen art; picked by `hash(title) % 6`), background / text:
light `#DCE8F7/#0E3A6E`, `#E5F0DC/#22461A`, `#F6E3D3/#5A2E0B`, `#EADFF3/#3D2257`, `#DCEFEF/#0F4444`, `#F3E6C4/#4F3A00`;
dark `#22344F/#D6E6FF`, `#25361E/#CDE8BF`, `#40291A/#F6D5BC`, `#33264A/#E6D6F5`, `#163838/#BFE6E6`, `#3B2F10/#F1DFA8`.
Options 1 and 2 use the same hues for their cover tiles.

**Option 2 Material tokens** (Material 3 roles; "Blue" = seeded from our accent, "Green" = a demo wallpaper):

| Role | Blue light | Blue dark | Green light | Green dark |
|---|---|---|---|---|
| `primary` | `#1D5FAE` | `#A9C7FF` | `#3B6939` | `#A1D39A` |
| `onPrimary` | `#FFFFFF` | `#003063` | `#FFFFFF` | `#0A390F` |
| `primaryContainer` | `#D6E3FF` | `#00468C` | `#BCF0B4` | `#23501F` |
| `onPrimaryContainer` | `#001B3E` | `#D6E3FF` | `#002204` | `#BCF0B4` |
| `secondaryContainer` | `#DAE2F9` | `#3E4759` | `#D7E8CD` | `#3C4B37` |
| `onSecondaryContainer` | `#131C2B` | `#DAE2F9` | `#121F0E` | `#D7E8CD` |
| `surface` | `#F9F9FF` | `#111318` | `#F7FBF1` | `#10140F` |
| `surfaceContainer` | `#EDEDF4` | `#1D2024` | `#EBEFE6` | `#1C211B` |
| `surfaceContainerHigh` | `#E7E8EE` | `#282A2F` | `#E5E9E0` | `#272B25` |
| `onSurface` | `#191C20` | `#E2E2E9` | `#181D17` | `#E0E4DB` |
| `onSurfaceVariant` | `#44474E` | `#C4C6D0` | `#424940` | `#C2C9BD` |
| `outline` | `#74777F` | `#8E9099` | `#72796F` | `#8C9388` |
| `outlineVariant` | `#C4C6D0` | `#44474E` | `#C2C9BD` | `#424940` |
| `hl` / `hlFg` (fixed, not from wallpaper) | `#FFDEA6` / `#261900` | `#5B4300` / `#FFDEA6` | same | same |

### 9.2 Measured contrast (WCAG ratio; all text pairs ≥ 4.5)

Base: `fg2/bg2` 5.47 L · 7.33 D; `fg2/bg` 6.09 L; `accent/bg` 6.36 L; `accent/bg2` 8.17 D; `onAccentContainer/accentContainer`
9.16 L · 9.94 D; `warnFg/warnBg` 7.20 L · 9.12 D; `okFg/bg` 6.54 L · 10.82 D; `errFg/errBg` 7.84 L.
Black: `fg2/bg2` 7.43; `fg2/bg` 8.33; `hlFg/hl` 11.16. Stage: `stageFg2/stageBg` 7.83 D · 6.18 L; `stageFg2/stageCtrlBg` 6.47 D;
`stageAccent/stageBg` 5.60 L. Option 2: `onSurfaceVariant/surfaceContainer` 7.98 L · 9.60 D; `primary/surface` 6.07 L ·
10.88 D; `hlFg/hl` 13.30 L · 7.23 D; Green `primary/surface` 6.13 L; `onSurfaceVariant/surfaceContainer` 7.98 L · 9.65 D.
**Rule for Fable:** never mute text with `opacity`; use the `*2`/`Variant` tokens. Any new text/background pair you need
must be one from these tables.

### 9.3 UI typography (options 1, 3, 4)

| Style | Roboto size / line | Weight |
|---|---|---|
| `title-l` (screen titles) | 22 / 28 | 600 |
| `title-m` (sheet titles, card titles) | 18 / 24 | 600 |
| `body` | 16 / 24 | 400 |
| `body-s` | 14 / 20 | 400 |
| `label` (buttons, row titles) | 14 / 20 | 500 (buttons 16 / 24 500) |
| `caption` | 12 / 16 | 400 |

Option 3 uses the same but row titles are 16 / 20 500 and meta 14 / 18 (denser). Option 2 uses the Material type scale:
`headlineMedium 28/36 400`, `headlineSmall 24/32 400`, `titleLarge 22/28 400`, `titleMedium 16/24 500`, `bodyLarge 16/24 400`,
`bodyMedium 14/20 400`, `labelLarge 14/20 500`, `labelMedium 12/16 500`.

### 9.4 Shared components (base style; options restyle as stated in §11)

- **Primary button:** height 56, radius full, `accent` bg, `onAccent` text `label` 16/24 500, 24 px icon + 8 gap when it
  has one, full width minus 2 × 16. Disabled: `line` bg, `fg2` text.
- **Secondary button:** height 48, radius full, 1 px `line` border, `fg` text. **Text button:** height 48, padding 0 12,
  `accent` text, no border.
- **Icon button:** 48 × 48 touch area, 24 icon centred, `fg`. Pressed state: a 40 px circle of `line` behind the icon.
- **List row:** min height 56 (72 with two lines), padding 8 16; leading icon 24 at 16 from the edge, text starts at 56;
  trailing chevron or control at 16 from the right edge. Divider 1 px `line`, inset 16.
- **Switch:** Material 3 shape: track 52 × 32 radius 16; off = 2 px `fg2` outline track on `bg2`, thumb 16 `fg2`; on =
  `accent` track, thumb 24 `onAccent`.
- **Slider:** track 4 px radius 2 `line`; active part `accent`; buffered part (audio already made ahead) `accentContainer`
  between the thumb and the buffered point; thumb 20 px circle `accent` (Material 3's 4 × 44 bar thumb is also fine; pick
  the circle). Chapter ticks: 2 × 8 px marks of `fg2` sitting on the track at each level-1/2 chapter start.
- **Choice chip:** height 32 (48 touch), padding 0 12, radius `r-s`, 1 px `line`, `fg` `label` 14/20. Selected:
  `accentContainer` bg, `onAccentContainer` text, no border, 18 px check icon before the text.
- **Linear progress:** 8 px tall radius 4 for big progress (downloads, export), 4 px radius 2 elsewhere. Indeterminate =
  a 40 %-wide `accent` segment sliding left to right, 1.4 s loop.
- **Spinner:** 16 px ring, 2 px stroke, `accent`, 270° arc rotating.
- **Bottom sheet:** `bg2`, top radius 28, drag handle 32 × 4 radius 2 `fg2` centred 12 from the top (48 px handle area),
  scrim behind it, max height = screen − 32 (status) − 16. Content padding 0 16, bottom padding = 24 + 24 (gesture inset).
- **Dialog:** `bg2`, radius 28, padding 24, width 412 − 2 × 24; title `title-m`; body `body` in `fg` (it's the message,
  not skippable); buttons are text buttons right-aligned, 8 gap.
- **Banner (inline message):** radius 12, padding 12 16, leading 24 icon, text `body-s`; variants `warn`
  (`warnBg/warnFg`), `err` (`errBg/errFg`); optional text button in the same `*Fg` colour, `label` 14/20 500.
- **Snackbar:** `inverseBg` / `inverseFg`, radius 8, min height 48, padding 14 16, `body-s`, sits 12 above the bottom
  controls; an optional action uses `inverseFg` 500 underlined (`accent` on `inverseBg` would fail contrast).
- **Cover tile:** a rectangle of a cover hue (§9.1) with the title's first two initials ("LB") in Noto Serif 600,
  `onTile` colour, size relative to the tile (40 % of the tile height). Radius 8 (small), 12 (grid), 28 (Now playing).

---

## 10. Placeholder content (ours, safe for a public repo)

- **Books** (invented; titles may be used anywhere):
  - "The Long Book" — 589 PDF pages, printed labels i–xiii then 1–575; 15 chapters, structure = `LONG_BOOK_CHAPTERS`
    + `LONG_BOOK_SUBSECTIONS` in `frontend/src/api/mock/sampleText.ts` (copy as plain JS). **Current place: chapter 7
    "Long Documents", section 7.3 "Tables of contents that tell the truth", printed page 265 (PDF page 279), 49 % of the
    book, 38 % of the chapter, 9 min left in the chapter, 4 h 12 min left in the book, about 8 h 10 min total.**
  - "Designing Calm Software" — 18 pages, 34 %, 6 TOC entries (Preface, 1. Quiet Interfaces › 1.1–1.3 › What to try next).
  - "Meeting Notes (no chapters)" — 18 pages, not started, empty TOC.
  - "Quarterly Notes" — 42 pages, 35 %.
  - "A Field Guide to Small Gardens" — 214 pages, 72 %, opened last week.
  - Import examples: `garden-plans.pdf` (importing), `scan-2019.pdf` (no text), `atlas.pdf` (312 MB).
- **Reader text:** use the sentences of `PREFACE` and `CHAPTER` in `frontend/src/api/mock/sampleText.ts` (invented by us),
  and for section 7.3 this passage (ours):

  > **7.3 Tables of contents that tell the truth**
  > A table of contents is a promise. It tells the reader what is inside, in what order, and roughly how far away each
  > part is. When the promise is kept, people jump around a long document without fear. When it is broken, they stop
  > trusting every other signpost too.
  > The most common way to break it is drift. Headings change during editing, but the contents page was typed once and
  > never touched again. A generated outline cannot drift, because it is read from the headings themselves.
  > *(code block)* `outline = [h for h in headings if h.level <= 2]` / `for entry in outline: print(entry.page, entry.title)`
  > Length is the other trap. A section title that runs to three lines is no longer a signpost; it is a paragraph wearing
  > a heading's clothes. Keep titles short enough to scan, and let the first sentence of the section do the explaining.

  Put a page change (chip "p. 266") before "The most common way". The **current sentence** in R1 is "When the promise
  is kept, people jump around a long document without fear."
- **Voices:** copy the 28 English voices from `frontend/src/api/mock/voices.ts` (grouped American/British × female/male;
  default "Heart"). Sample line (ours): "This is how I sound when I read your books aloud."
- **Times/sizes:** download 370 MB / 126 MB; downloaded 142 MB of 370 MB, 38 %, 2.1 MB/s, about 2 min left; free space
  23.4 GB; audio cache 312 MB of 1 GB; library 3 books · 48 MB; export about 8 h 10 min of audio, about 231 MB, about
  2 h to make, running at 42 %, part 21 of 49, about 1 h 10 min left. App version "0.1.0 · core 0.2.0".

### 10.1 Copy (use verbatim)

| Where | Text |
|---|---|
| S1 title / tagline | "Lectrice" / "Your PDFs, read aloud." |
| S1 points | "Cleans up the text: no running headers, page numbers or footnotes." · "Reads with a natural voice that runs on this phone." · "Works offline. Your books never leave your device." |
| S1 buttons | "Get the voice" (primary) · "Not now, just read" (text) |
| S2 title / body | "Choose a voice model" / "The voice is a one-time download. You can switch later in Settings." |
| S2 card 1 | "Full quality" · "370 MB" · "The same voice as the desktop app. Sounds the most natural." · "Quality: best · Speed on this phone: measured after setup" |
| S2 card 2 | "Smaller" · "126 MB" · "Saves space. A little less natural." · "Quality: good · Usually slower on phones" |
| S2 hints | "Not sure? Pick Full quality." · "Free space: 23.4 GB" · "Wi-Fi recommended" |
| S2 buttons | disabled "Choose a model" → "Download 370 MB" / "Download 126 MB" · text "Import from a file…" |
| S2b dialog | "You're on mobile data" / "This download is 370 MB and may use your data plan. If you wait for Wi-Fi, it starts by itself when you connect." / "Wait for Wi-Fi" · "Download now" |
| S3 | "Downloading the voice" · "142 MB of 370 MB · 38 %" · "2.1 MB/s · about 2 min left" · reassurance (its own line, `fg`, with an info icon): "You can leave this screen. The download keeps going in the background and shows in your notifications." · buttons "Pause" (secondary), "Cancel" (text) · "Meanwhile, open a PDF" (text) |
| S3a | "Paused · 142 MB of 370 MB" · "Resume" |
| S3b | warn banner: "Waiting for a network. The download continues on its own." |
| S3c | err banner: "Part of the download was damaged. Retrying…" (then) "Try again" |
| S3d | err banner: "Not enough space. The voice needs 570 MB free (370 MB plus 200 MB to work with). Free up space and try again." |
| S4 | "The voice is ready" · sample card "Heart · American English" + the sample line in Noto Serif italic · "Open a PDF" (primary) · "Go to library" (text) |
| L1 | "Lectrice" · "Continue listening" · "Recent" · "Open PDF" |
| L2 | "Open a PDF to start listening" / "Lectrice cleans up the text and reads it aloud with a voice that runs on this phone. Nothing leaves your device." / "PDFs with a text layer, up to 200 MB" |
| L3 | "Reading… cleaning text" · then "Page 120 of 214" |
| L4 (sheet, err icon) | "Can't read this PDF" / "“scan-2019.pdf” is a scanned image with no text inside, so there's nothing to read aloud. Lectrice can't read scans yet." / "OK". Caption lists the others: encrypted "“report.pdf” is password-protected. Save an unprotected copy in another app and try again." · too large "“atlas.pdf” is 312 MB. Lectrice opens PDFs up to 200 MB." · damaged "“file.pdf” looks damaged or isn't a PDF." · storage "Your phone is out of space. Free up about 400 MB and try again." |
| L5 | warn banner: "Voice not installed. You can read now and listen after the download." + "Download" |
| L6 snackbar | "Already in your library. Continuing where you left off." |
| Pill statuses | playing "Ch. 7 · p. 265" / "Long Documents" · with speed "Ch. 7 · p. 265 · 1.5×" · no chapter "p. 12 of 18" (one line) · preparing line 2 "Preparing audio…" + spinner · slow phone line 2 "Catching up…" + spinner · ended "Finished" / "Play to start over" · no voice "Voice not installed" / "Tap to download" (`accent`) · voice failed "Voice unavailable" / "Tap for details" |
| R2 chip | "Back to current" with ↓ or ↑ icon |
| R7 tip (warn, dismissible) | "This phone makes speech a little slower than you listen at 1.5×, so Lectrice pauses now and then to catch up. 1× or 1.25× plays smoothly." + "Set 1.25×" · conditional extra line (only if Prepare exists): "Prepare this chapter while charging" |
| R9 | err banner: "The voice couldn't start. Try again, or download it again in Settings." + "Try again" |
| R10 sheet | "Show the player outside the app?" / "Lectrice shows the player in your notifications and on the lock screen, so you can pause it without opening the app." / "Continue" (then the system dialog: "Allow Lectrice to send you notifications?" · "Allow" · "Don't allow") |
| R8c | "This PDF has no chapters or headings, so there's nothing to list. The page numbers in the text show where you are." |
| E1 | "Export audiobook" · rows "Format: M4B with chapters" · "Length: about 8 h 10 min" · "Size: about 231 MB" · "Time to make: about 2 h on this phone" · "Voice: Heart at normal speed. Set the speed in your audiobook app." · reassurance: "Keep using your phone. The export runs in the background, and if Android pauses it, it picks up where it stopped." · primary "Start export (about 2 h)" |
| E2 | "Exporting · 42 %" · "Part 21 of 49 · about 1 h 10 min left" · "Listening comes first: the export slows down while you listen." · "Pause" · "Cancel export" |
| E3 | warn banner: "Paused by Android. Long background jobs have a daily time limit. It continues while charging, or when you open Lectrice." · "Continue now" · text "Samsung phones may stop it sooner: allow unrestricted battery" |
| E4 | ok icon "Audiobook ready" · "8 h 10 min · 231 MB" · primary "Save to Audiobooks" + caption "Audiobook players will find it there" · secondary "Save as…" · text "Share" |
| E5 | err banner: "The export stopped: the phone ran out of space. 21 of 49 parts are kept, so nothing is lost. Free up space and continue." · "Continue" · "Cancel export" |
| N1/N2 | title "7.3 Tables of contents that tell the truth", subtitle "The Long Book" |
| N3 / N4 | "Exporting The Long Book · 42 %" (actions Pause, Cancel) / "Downloading the voice · 38 %" (action Pause) |
| T1 | see §11.1 T1 |

---

## 11. Screen specs per option

Measurements are dp = CSS px inside the phone screen. "Gesture inset" = 24, "status inset" = 32. Every frame gets
light + dark through the token sets. TalkBack labels go into `aria-label`s (listed per control where not obvious).

### 11.1 Option 1 "Pill" (the full spec; options 2–4 list only what differs)

**Live phone (option 1)** starts on L1 and supports:
1. Tap the "Continue" card's play button or a recent row → push transition to R1 (reader slides in from the right 24 px +
   fades, 320 ms), already playing if the play button was used, paused otherwise.
2. Play/pause toggles; while playing, the highlight moves to the next sentence every 2.5 s, skipping the code block
   (show the cue line instead), and the text auto-scrolls so the highlight sits at 35 % of the screen height.
3. Tap any sentence → highlight jumps there, play/pause state unchanged, follow-scroll resumes.
4. Scroll the text by hand → follow-scroll stops; the "Back to current" chip fades in (↓ if the highlight is below the
   view, ↑ if above). Tap it → smooth scroll back, chip hides.
5. Prev/next → previous/next sentence.
6. Tap the pill status → chapters sheet (R8) slides up; tap "›" on chapter 7 → R8b; tap "‹ All chapters" → back; tap a
   row → highlight jumps to that entry's first sentence, sheet closes, status updates.
7. Tap "more" → player sheet (R4) opens at its half detent; drag its handle up → full; down past 30 % → close.
   **Drag the pill itself upward** → the sheet follows the pointer and snaps (pointer events; this is the Telegram-style
   piece in the recommendation).
8. In the sheet: speed chips change the speed (status gains "· 1.5×"); "Voice" pushes R5 inside the sheet (slide from the
   right) with a back arrow; "Sleep timer" chips select (status line 1 gains a 16 px moon icon + "28 min");
   "Export audiobook" pushes E1 → "Start export" animates E2 progress from 42 % to 100 % over 6 s → E4.
9. `◁ Back` closes the topmost sheet/page; on the reader with nothing open it shows R11 (predictive back preview) for
   400 ms and then returns to L1.
10. Library settings icon → T1 (push); `◁ Back` returns.

**S1 Welcome.** Column, padding 16, content starts at 32 + 96. App mark: 64 × 64, radius 16, `accent` bg, a Noto Serif
600 "L" 36 sp in `onAccent`. 24 below: "Lectrice" Noto Serif 32/40 600 `fg`; 8 below: tagline `title-m` 18/24 400 `fg`.
32 below: three rows, each 24 icon (`accent`: `menu_book`, `headphones`, `check_circle`) + 16 gap +
`body` text, rows 16 apart. Bottom (anchored to bottom − 24 − 16): primary "Get the voice", 8 below: text button
"Not now, just read" centred.

**S2 Choose model.** Top: back arrow icon button at (4, 32+4). Title `title-l` at 16 below the arrow; body `body` `fg2`
8 below. 24 below: two **radio cards**, 12 apart, each: `bg2`, radius 16, 1 px `line`, padding 16; row 1 = 24 radio
circle (2 px `fg2` ring; selected: `accent` ring + 12 px `accent` dot) + 12 gap + name `title-m` + size right-aligned
`label` 16/24 600; row 2 (indent 36) `body` `fg`; row 3 (indent 36) `body-s` `fg2`. Selected card: 2 px `accent` border
(padding 15 so nothing shifts) and `accentContainer` background with `onAccentContainer` text for rows 1–3 (contrast ok).
Under the cards, 16 below: "Not sure? Pick Full quality." `body-s` `fg2`; then two 20-icon rows `body-s` `fg2`:
`sd_storage` "Free space: 23.4 GB", `wifi` "Wi-Fi recommended". Bottom: primary button (disabled "Choose a model" until a
card is picked, then "Download 370 MB" or "Download 126 MB"), 8 below text button "Import from a file…".
States to show: S2 nothing selected; S2 (live) with "Full quality" selected.

**S2b Metered dialog** over S2 with scrim. Dialog per §9.4; buttons "Wait for Wi-Fi" and "Download now".

**S3 Downloading.** Title `title-l` "Downloading the voice" at 32 + 48. 32 below: progress 8 px bar; 12 below
`label` 16/24 "142 MB of 370 MB · 38 %"; 4 below `body-s` `fg2` speed line. 24 below: the reassurance line (20 info icon
+ `body` `fg`), in a `bg2` card radius 12 padding 12 16. Bottom: row of "Pause" (secondary, flex 1) + "Cancel" (text);
above them, 8 up, text button "Meanwhile, open a PDF". Variants S3a–S3d as separate frames (banner placed between the
speed line and the reassurance card; S3a shows "Resume" as primary instead of "Pause").

**S4 Ready.** 48 px `check_circle` in `okFg` at 32 + 96; title `title-l` 16 below; 24 below: sample card `bg2` radius 16
padding 16: 56 round play button (`accent`) left, right of it "Heart · American English" `label` and the sample line in
Noto Serif italic 17/26 `fg`. Bottom: primary "Open a PDF", text "Go to library".

**L1 Library.** Top row at 32 + 8: "Lectrice" `title-l` left (x 16), `settings` icon button right (x 412 − 4 − 48).
16 below: **Continue card**: `bg2`, radius 16, 1 px `line`, padding 16, margin 0 16. Left column: caption "Continue
listening" (`caption` 500 `fg2`), title "The Long Book" `title-m` (2 lines max), `body-s` `fg2` "Ch. 7 · Long Documents ·
p. 265" (1 line, ellipsis on the name only), 12 below a 4 px progress bar (49 %) and `caption` `fg2` "49 % · 4 h 12 min
left". Right column: 56 round play button (`accent`, `onAccent` icon), vertically centred; `aria-label="Play The Long
Book from chapter 7"`. 24 below the card: "Recent" `label` 14/20 600 `fg` at x 16. Rows (72 tall): left 40 × 56 cover tile
(radius 8) at x 16; text at x 72: title `label` 16/24 500 1 line + `body-s` `fg2` "589 pages · opened just now"; right:
"Continue · 34 %" `label` `accent` above a 56 × 4 progress bar (or "Start" when 0 %). Divider inset 72. Rows: Designing Calm
Software (34 %, 2 hours ago), Meeting Notes (Start, yesterday), Quarterly Notes (35 %, last week), A Field Guide to Small
Gardens (72 %, last week). **Bottom bar:** `bg` with a 1 px `line` top border, padding 12 16 (12 + 24 gesture at the
bottom), primary button with `note_add` icon "Open PDF". The list scrolls under nothing; its bottom padding clears the bar.

**L2 Empty.** No Continue card, no Recent. Centred column (vertical centre of the area above the bottom bar): title
`title-l` centred, 8 below body `body` `fg2` centred max-width 320, 12 below `caption` `fg2` limits line. Bottom bar as L1.

**L3 Importing.** L1 with a first row in "Recent": cover tile placeholder (`line` bg), title "garden-plans.pdf", second
line = indeterminate 4 px progress (width 160) + 8 gap + `body-s` `fg2` "Reading… cleaning text". A second frame
"L3 (long import)" shows determinate progress + "Page 120 of 214".

**L4 Import error.** Modal bottom sheet (auto height): 24 `error` icon in `errFg` + title `title-m` on one row, 12 below
body `body` `fg`, 24 below primary "OK". Caption lists the other error texts.

**L5 Voice-not-installed.** L1 with a warn banner between the top row and the Continue card (margin 0 16 16), text +
"Download" text button aligned right on its own line.

**L6 Snackbar.** R1 (reader) with the snackbar 12 above the pill, left/right 16.

**R1 Reader, playing.** Text per §8.4 and §10, edge-to-edge (status scrim at top). **Pill:** absolute, left 12, right 12,
bottom 12 + 24 (gesture inset) = 36; height 64; radius full; `bg2`; 1 px `line`; padding 0 8; flex row, centred items:
- prev icon button 48 (`aria-label="Previous sentence"`)
- play/pause: 56 touch, 48 visible `accent` circle, 24 icon `onAccent` (`aria-label="Play"`/`"Pause"`)
- next icon button 48
- **status button**: flex 1, min-width 0, height 48, margin 0 4, padding 0 8, radius 12, text left-aligned, two lines:
  line 1 `label` 14/18 500 `fg` (fixed parts never shrink: "Ch. 7", "p. 265", "1.5×"; joined by " · "), line 2 13/16 400
  `fg2` chapter name with ellipsis; trailing 16 px `expand_less` icon `fg2`. One-line states centre vertically. Whole
  status is one button (`aria-label="Chapter 7, Long Documents, page 265. Show chapters"`), `aria-live="polite"` on the
  text.
- more icon button 48 (`more_horiz`, `aria-label="Player and settings"`, `aria-expanded`).
At 360 width the pill keeps all parts but drops the ⌃ icon (status text ≈ 96 dp wide; line 1 must fit "Ch. 7 · p. 265"; with speed, drop to two
fixed parts by moving "1.5×" to line 2 before the name: "1.5× · Long Documents").
Reader bottom padding = 36 + 64 + 48 = 148.

**R2 Scrolled away.** R1 scrolled up by ~400 px so the highlight is below the view. **Back-to-current chip**: right 16,
bottom = pill top + 12; height 40 (48 touch), radius full, `bg2`, 1 px `line`, padding 0 16 0 12, 20 `arrow_downward`
icon + 8 + `label` 14/20 500 `fg` "Back to current".

**R3 Preparing.** R1 paused-to-playing transition: status line 2 = 16 spinner + "Preparing audio…"; the play button shows
a 2 px `accent` ring spinning around the 48 circle (circle becomes `accentContainer` with `onAccentContainer` pause icon).

**R4 Player sheet** (half detent = 600 high at 412×915; full = to 32 + 16 from the top). Content top to bottom:
1. Handle area 48.
2. **Transport row** (the pill "grows" into this): centred, prev 48 · play 64 touch / 56 visible · next 48, 16 gaps;
   left of prev and right of next: **section jumps** `keyboard_double_arrow_left/right` 48 (`aria-label="Previous
   section"`/`"Next section"`).
3. 16 below: **book slider** full width (with buffered part from 49 % to 53 % and chapter ticks); 4 below: row `caption`
   `fg2` "Ch. 7 · 49 %" left, "4 h 12 min left" right.
4. 16 below: **chapter block** as a tappable card (`bg`, radius 12, padding 12 16): line 1 `label` 15/20 500 `fg`
   "7. Long Documents › 7.3 Tables of contents that tell the truth" (2 lines max); line 2 `body-s` `fg2` "38 % of this
   chapter · 9 min left in it"; line 3 `body-s` `fg2` "Page 265 · PDF page 279 of 589"; trailing `toc` icon + "Chapters"
   `label` `accent`. Tap → R8.
5. 24 below: "Speed" `label` 600; 8 below a single row of 6 chips (0.75×, 1×, 1.25×, 1.5×, 1.75×, 2×; 8 gap; scrolls
   horizontally at 360).
6. 16 below: list rows (56, no leading inset; divider 1 px `line`):
   - `record_voice_over` "Voice" / "Heart · US · female" › → R5
   - `bedtime` "Sleep timer" / "Off" — tapping expands chips inline (R6)
   - "Read code & tables aloud" / `body-s` `fg2` "Otherwise Lectrice says “Code example.” or “Table.”" + switch
   - `download` "Export audiobook" / "One M4B file with chapters" › → E1
   - `menu_book` "Library" › (back to L1)
   - `note_add` "Open another PDF…"
Show R4 at half detent (rows 1–5 visible) and at full.

**R5 Voice page** (inside the sheet, full detent): header row 56: back arrow 48 + "Voice" `title-m`. Group headers
`label` 600 `fg2` padding 16 16 8 ("American English · female", "· male", "British English · female", "· male"). Rows 56:
radio 24 + 16 + name `body`; trailing 48 icon button `volume_up` (`aria-label="Hear Bella"`). Selected row:
`accentContainer` bg, `onAccentContainer` text. Show "Heart" selected and "Bella" sample playing (its icon replaced by
a 16 spinner-like equaliser: three 3 px bars in `accent`).

**R6 Sleep timer** (R4 with the sleep row expanded): chips "Off", "15 min", "30 min", "45 min", "1 h", "End of chapter";
"30 min" selected; the row's subtitle becomes "Stops at 23:41"; pill line 1 shows the moon icon + "28 min".

**R7 Slow phone.** R1 at 1.5× with status line 2 "Catching up…" + spinner; the warn tip card above the pill: left/right
16, bottom = pill top + 12, radius 12, padding 12 16, `body-s` text, row of text buttons "Set 1.25×" and a `close` icon
button (dismiss, `aria-label="Dismiss tip"`). Second frame "R7 + Prepare (only if the spike needs it)": the same tip with
a second text button "Prepare this chapter while charging". Caption: "Shown once per book when audio runs out; never
while it keeps up."

**R8 Chapters sheet.** Modal, height 85 % of the screen. Header 56: "Chapters" `title-m` at x 16, `close` 48 right. Rows
(min 56, padding 12 16): title `body` 15/20 (2 lines max), right column 72 wide: page label `body-s` `fg2`
right-aligned, under it for the current chapter "38 %" `caption` 500 `accent`; trailing 48 "›" drill button when the
entry has children (`aria-label="Show sections of Long Documents"`); meta under a collapsed chapter "7 sections"
`caption` `fg2`. Read rows: 16 `check` icon before the page label, title in `fg2`. **Current row:** 3 px `accent` bar on
the left edge (full row height), title weight 600. The list opens with the current row centred. Rows: Preface, 1.–15.,
using the mock titles (chapter 4's long title wraps to 2 lines then ellipsis).
**R8b drilled in:** header becomes "‹ All chapters" (text button, `accent`) + chapter title `title-m` (1 line) + close;
rows 7.1–7.7 (7.1, 7.2 read; 7.3 current with "38 %"; others unread), sub-sections (level 3) drill one more level the
same way. (This matches `design/screens/reader-sheet-375.png`.)
**R8c no chapters:** header + the empty text (`body` `fg2`, padding 24 16).

**R9 Voice failed.** R1 paused; err banner fixed under the status bar (margin 8 16), with "Try again"; pill status "Voice
unavailable" / "Tap for details".

**R10 Notification permission.** Two frames: our small sheet (title `title-m`, body `body`, primary "Continue"); then a
system-style dialog (`#2B2D31` dark / `#FFFFFF` light card, radius 28, app mark 32 at the top, text 16/24, two full-width
stacked buttons "Allow" / "Don't allow" with `accent` text) over a scrim.

**R11 Predictive back.** L1 fully drawn behind; the reader drawn on top scaled to 0.9, shifted 24 px right, radius 32,
with a 1 px `line` border. Caption: "Back swipe in progress: release to go to the library."

**R12 200 % font.** R1 and L1 with UI text doubled (reader text 38/64): pill status shows line 1 only (ellipsis);
pill grows to 72 tall; Continue card stacks the play button under the text; nothing overlaps. Two frames.

**N1 Notification card** (inside a mock notification shade: `#E9EAEE` light / `#1B1C20` dark panel with radius 28,
quick-settings tiles as plain grey rounded rectangles): media card height 168, radius 24, background = the book's cover
hue, left 64 × 64 cover tile (radius 12), title `label` 16/24 500 (1 line), subtitle `body-s`, controls row: prev,
play/pause (56 circle in the tile's text colour with the hue as icon colour), next. **No seek bar** (Q7). Caption: "System-
drawn; we only set title, subtitle, artwork and buttons. Next = next sentence."
**N2 Lock screen:** a dark gradient wallpaper (`#1B2635` → `#3A2C3F`), clock "9:41" Roboto 300 72/80 white centred at
y 140, date `body` white below, the same media card at y 520, a lock icon and "Swipe up to unlock" at the bottom.
**N3 / N4:** standard notifications in the shade (app icon 24, "Lectrice · now", title, a 4 px progress bar, action
text buttons in `accent`).

**E1–E5 Export** (pushed inside the player sheet at full detent, header 56 with back arrow + "Export audiobook"):
E1: four info rows (`body`, label `fg2` 120 wide + value `fg`), the voice note `body-s` `fg2`, the reassurance card
(`bg`, radius 12, info icon, `body` `fg`), primary "Start export (about 2 h)". E2: 8 px progress + "Exporting · 42 %"
`label` + `body-s` `fg2` parts line, the "Listening comes first" line, buttons "Pause" (secondary) + "Cancel export"
(text). E3: E2 with the warn banner above the progress, buttons "Continue now" (primary) + battery text button.
E4: 48 `check_circle` `okFg`, "Audiobook ready" `title-m`, summary line, then primary "Save to Audiobooks" + its caption,
secondary "Save as…", text "Share" with `ios_share`. E5: E2 with the err banner and buttons "Continue" / "Cancel export".

**T1 Settings** (pushed screen): top bar 64: back arrow 48 at x 4 + "Settings" `title-l`. Sections (header `label` 600
`accent` padding 24 16 8; this is `accent` on `bg`, 6.36:1):
- *Listening:* Default voice › "Heart · US · female" · Default speed › "1×" · Read code & tables aloud [switch] ·
  Read ahead › "10 min of listening"
- *Reading:* Text size: a 5-stop slider (S, M, **L**, XL, XXL) with a live preview line "The quick reader turns the page."
  in Noto Serif at the chosen size · Theme: segmented control (height 40, radius full, 1 px `line`; segments "System",
  "Light", "Dark"; selected segment `accentContainer` with a check)
- *Voice model:* "Full quality · 370 MB · in use" › T2
- *Storage:* "Audio cache" with a 4 px bar "312 MB of 1 GB" and text button "Clear audio cache" + under it `body-s` `fg2`
  "Your books and reading places stay." · "Library: 3 books · 48 MB"
- *Battery:* "Keep exports running" / "Allow unrestricted battery use" › (opens the system page)
- *About:* "Version 0.1.0 · core 0.2.0" · "Open-source licenses" › · "Source code on GitHub" ›
Show T1 scrolled to the top, and a second frame scrolled to the bottom.

**T2 Voice model:** two cards like S2 but with state: Full quality "Installed · in use" (no button); Smaller "Not
installed" + secondary "Download 126 MB". A second frame with both installed: Smaller gets "Use this" + text button
"Delete (126 MB)" (`errFg`), and a confirm dialog frame "Delete the smaller model? / You can download it again later." ·
"Cancel" · "Delete".

### 11.2 Option 2 "Material": what differs

Use the Option 2 tokens and type scale; shadows `e-2`/`e-3`; "Wallpaper: Blue | Green" toggle swaps the palette.
- **S1–S4:** same content; buttons are Material filled buttons (height 40, radius full, `labelLarge`) with **48 touch**;
  S2 cards are Material **outlined cards** (radius 12) with radio buttons; selected = `secondaryContainer`.
- **L1:** **large top app bar**: "Library" `headlineMedium` at 32 + 64, collapsing to a 64 small bar with `titleLarge`
  when scrolled (show both frames); actions: `settings`. Continue = **filled card** `surfaceContainerHigh`, radius 16,
  with a 72 × 96 cover tile, title `titleMedium`, supporting `bodyMedium` `onSurfaceVariant`, a 4 px progress, and a
  filled tonal play button. Recent = Material list items 72: 40 × 56 tile, headline + supporting text, trailing **32 px
  circular progress** (3 px stroke, `primary` on `outlineVariant`) with "34" `labelMedium` inside. **Extended FAB**
  bottom-right (right 16, bottom 16 + 24): height 56, radius 16, `primaryContainer`, `onPrimaryContainer`, `note_add` +
  "Open PDF" `labelLarge`, `e-2`. When a book is playing, a **mini player** sits above the FAB area: left/right 8, bottom
  8 + 24, height 64, radius 16, `surfaceContainerHigh`, `e-3`: tile 48, title + "Ch. 7 · p. 265", play 48; FAB moves up
  above it.
- **R1:** **top app bar** 64 (`surface`, becomes `surfaceContainer` once the text scrolls under it): back arrow, two-line
  title ("The Long Book" `titleMedium` / "Ch. 7 · p. 265" `bodyMedium` `onSurfaceVariant`), actions `toc` and
  `more_vert`. It hides on scroll down and returns on scroll up (show both). Bottom: **horizontal floating toolbar,
  vibrant**: centred, bottom 16 + 24, height 64, radius full, `primaryContainer`, `e-2`, padding 0 8, items 48: prev,
  **play = 56 square-ish FAB** (radius 16, `primary`, `onPrimary`; when pressed/playing radius animates to 28),
  next, speed text button "1×" (`labelLarge`), `open_in_full` (expand → Now playing). No status text in the toolbar.
- **Now playing (new frame R13):** full screen `surface`: top row `expand_more` (close) + "Now playing" `titleMedium` +
  `more_vert`; cover tile 280 × 280 radius 28 centred at y 120; 24 below title `headlineSmall` 1 line + `bodyLarge`
  `onSurfaceVariant` "7.3 Tables of contents that tell the truth"; current-sentence preview card (`surfaceContainer`,
  radius 16, padding 16, Noto Serif 17/26, 3 lines, ellipsis); slider with chapter ticks + times "3 h 58 min" left /
  "−4 h 12 min" right; **button group**: prev 56, play 96 × 72 (radius 28, `primary`), next 56, 8 gaps; secondary row of 4
  tonal icon buttons 48 (`speed`, `bedtime`, `record_voice_over`, `toc`) with `labelMedium` captions under each.
- **Player sheet (R4):** replaced by Now playing; "more" overflow menu (Material menu, radius 4, `surfaceContainer`,
  items 48: Export audiobook, Read code & tables aloud ✓, Text size, Settings).
- **Speed:** a **dialog** with a slider 0.75–2× (step 0.25 snaps + labels) and chips. **Sleep timer:** a modal bottom sheet
  list with radio items.
- **R8 Chapters:** modal bottom sheet with an **expandable tree**: level-1 rows have a trailing `expand_more` that opens
  their children in place (indent 16 per level); the current chapter is expanded on open; current row =
  `secondaryContainer` rounded 28 "pill" highlight (Material navigation-drawer style) instead of the left bar.
- **N1/N2:** same; artwork = cover tile with `primaryContainer`.
- **T1:** Material list with `titleSmall`-style section headers in `primary`; theme stays three segments, plus a separate switch row "Wallpaper colours" (Android 12+) under Theme.
- **Live phone:** as option 1, but: toolbar instead of pill; tap `open_in_full` → Now playing slides up (320 ms); `toc`
  in the app bar → chapters; scrolling hides/shows the app bar.

### 11.3 Option 3 "Quiet": what differs

Base tokens, dark = black set; no shadows anywhere; dividers are 1 px `line`; denser type (§9.3).
- **L1:** header 56: "Lectrice" 20/24 600 + `search`-less, `settings` right. No Continue card: the **first row** is the
  current book, marked by a 3 px `accent` bar on its left edge. Rows 72: a **48 progress ring** at x 16 (3 px stroke,
  `accent` on `line`, the percentage `caption` 500 inside; 0 % shows a play triangle); title 16/20 500 + meta 14/18
  `fg2` "Ch. 7 · Long Documents"; top-right `caption` `fg2` "now", "2 h", "Tue". **FAB:** 56 circle, `accent`,
  `note_add` icon, right 16, bottom 16 + 24, no shadow but a 1 px `bg` ring so it separates on `bg2` rows.
- **R1 expanded pill:** as option 1 but height 56, play 48 touch / 40 visible, status **one line** "Ch. 7 · p. 265".
  **R1 collapsed** (after scrolling down while playing or after 4 s without touches): the pill shrinks into a **56 round
  play button** (`bg2` + 1 px `line`, `accent` icon) at right 16, bottom 12 + 24, and a **2 px progress line** (`accent`
  on `line`) across the very bottom above the gesture handle. **Tap it = play/pause; swipe up on it or scroll
  the text up = expand.** Show both frames plus a transition frame (pill half-morphed).
- **Gestures (live phone):** drag the pill up → the player sheet follows the pointer (rubber-band past full); swipe the
  pill left/right ≥ 48 px → next/previous sentence with a 120 ms horizontal nudge of the status text; long-press play
  (500 ms) → a **speed scrubber** pops above the button: vertical stack of 6 chips (2× at the top … 0.75× at the bottom),
  slide the pointer to pick, release to apply. One-time **hint frame (R1h)**: a small `inverseBg` tooltip above the pill
  "Drag up for more. Long-press play for speed." with "Got it".
- **R4 player sheet:** single page, no sub-pages: transport row; slider; chapter block as plain text (no card); speed =
  **segmented scrubber** (one 40 px row of 6 segments, the selected one `accentContainer`); voice = an inline expanding
  list (collapsed shows the current voice; expanded shows 6 rows + "All 28 voices" which expands further); rows 48 for
  sleep (chips inline), read code switch, export (expands inline to the E1 summary + "Start"), library.
- **R8 chapters:** full-height sheet (to 32 + 8) with a **search field** at the top (height 48, radius full, `bg`, 1 px
  `line`, `search` icon, placeholder "Find a chapter or section" in `fg2`), then a **flat outline of every entry**
  (level 1 rows 48 with 16/20 500; level 2 indented 16, 15/20 400; level 3 indented 32, 14/20 400 `fg2`), **sticky level-1
  headers** (`bg2` with bottom 1 px `line`) as you scroll. Current entry: `accent` 3 px bar + 600 weight. Second frame:
  search "summ" typed → results grouped under their chapter headers, match bolded.
- **T1:** rows 48, no section cards; Theme segments "System · Light · Dark · Black".

### 11.4 Option 4 "Stage": what differs

Base tokens + stage tokens + cover hues.
- **L1 shelf:** header as option 1. **Hero** (margin 0 16): cover tile 120 × 160 radius 12 left; right: title `title-m`,
  "Ch. 7 · Long Documents" `body-s` `fg2`, 4 px progress + "49 %"; below spanning the width a primary button with
  `headphones` "Listen from p. 265" (56) and a secondary `menu_book` "Read" (48). Then "Recent" header and a **2-column
  grid** (gap 12, padding 16): tiles 3:4 ratio radius 12 in their hue, title on the tile (Noto Serif 18/22 600, 3 lines
  max, padding 12), a 4 px progress bar along the tile bottom (inset 12) in the tile's text colour; under the tile
  `caption` `fg2` "34 % · 2 h ago". The first grid cell is "Open PDF": same size, `bg`, 2 px dashed `line` border, centred
  `note_add` 32 + `label` "Open PDF".
- **Reader mode toggle:** a segmented control centred at 32 + 8: "Read" | "Listen" (height 40, width 200, radius full,
  `bg2` with 1 px `line`; selected `accentContainer`). In Read mode it sits on the status scrim and the reader is option 1
  (pill and all). In Listen mode it uses stage colours (`stageCtrlBg`, selected `stageAccent` with `stageOnAccent`).
- **R13 Listen stage:** full screen `stageBg`. Top block (at 32 + 64): book title `body-s` `stageFg2`; chapter line
  "7.3 Tables of contents that tell the truth" `label` 16/22 500 `stageFg`. **Teleprompter** (centre area, y 200–540):
  previous sentence Noto Serif 20/30 `stageFg2` (max 3 lines, clipped at the top with a 24 px fade made by a gradient
  from `stageBg` — a background gradient overlay, not text opacity); current sentence **Noto Serif 28/40 500 `stageFg`**;
  next sentence 20/30 `stageFg2`. **Thumb zone** (bottom 40 %): chapter progress bar 4 px with section ticks (full
  width − 32) + row `caption` `stageFg2` "38 % of chapter 7" / "9 min left"; transport centred: prev 64 (`stageCtrlBg`
  circle), **play 88** (`stageAccent` circle, 36 icon `stageOnAccent`), next 64; 16 below: "‹ Section" and "Section ›"
  pill buttons (height 48, `stageCtrlBg`, `label` `stageFg`); 16 below: a row of four 48 icon buttons in `stageFg`:
  speed showing "1×" text, `bedtime`, `record_voice_over`, `toc`.
  **Code block** current: the current-sentence slot shows a card (`stageCtrlBg`, radius 12, padding 16) with "Code
  example" `label` and 3 lines of Roboto Mono 14/20 `stageFg2`; caption "Read as “Code example.”".
- **Stage gestures (live phone):** tap anywhere in the teleprompter = play/pause (show a 600 ms centred 72 round
  play/pause flash); swipe left/right = next/previous sentence (the teleprompter slides up/down 320 ms); the "Read"
  segment returns to the reader at the same sentence.
- **R4 player sheet:** exists only in Read mode, as option 1. In Listen mode the speed/sleep/voice buttons open small
  sheets (speed chips; sleep chips; voice page R5) in stage colours (`stageCtrlBg` sheet, `stageFg` text).
- **R8 chapters:** option 1's sheet; in Listen mode it uses stage colours.
- **N1/N2:** artwork = the book's cover tile at 1:1.

---

## 12. Checklist for Fable (must all be true before handing back)

- [ ] One file `design/2026-10-23-android-options.html`; opens from disk with no network and still renders (fallback
      fonts); no external assets except the optional Google Fonts link; no libraries; no Artifact published; git untouched.
- [ ] Four option tabs; Light/Dark; 412/360 width; option 2's Blue/Green switch; solo mode `?opt&theme&w&screen` works for
      every screen ID.
- [ ] Every screen ID in §3 exists in every option (shared screens restyled with the option's tokens, plus R13 in options
      2 and 4, R1h in option 3), each with a caption.
- [ ] One live phone per option with every interaction listed for it (§11.1 list 1–10, adapted per option).
- [ ] All colours come from §9 tables via CSS custom properties; no hard-coded hex outside the token blocks
      (`grep -c '#[0-9a-fA-F]\{6\}'` hits only token blocks, the bezel, the lock-screen wallpaper and the system dialog).
- [ ] No text muted with `opacity`; all text pairs are from §9.2.
- [ ] Every interactive element ≥ 48 × 48 touch area, has an `aria-label` when it has no visible text, and shows a
      `:focus-visible` outline (2 px `accent`, offset 2).
- [ ] The highlight never hides behind the pill or the chips: auto-scroll keeps it at 35 % height.
- [ ] At 360 width nothing overlaps or clips (pill, Continue card, sheets, export buttons, option 4 grid and stage).
- [ ] R12 (200 % font) frames show no overlap.
- [ ] `prefers-reduced-motion` turns all motion off.
- [ ] All text is from §10 or the mock files named there. **No text from the user's books.**
- [ ] Self-review: no unused CSS classes or JS, no leftover scaffolding.

**Screenshots** (`design/screens/android-<option>-<screen>-<theme>.png`, one frame each via solo mode, at device
pixel ratio 2):
- For each option 1–4, light and dark: `L1`, `R1`, `R4` (option 2: `R13`), `R8` → 32 files.
- For each option, light, 360 width: `R1` → `android-<n>-R1-light-360.png` (4 files).
- Option 1 extra, light: `S2`, `S3`, `L2`, `L3`, `R2`, `R7`, `E2`, `E4`, `T1`; dark: `N2`, `R5`.
- Option 3 extra: `R1` collapsed (`android-3-R1c-dark.png`), `R8` with search (`android-3-R8s-light.png`).
- Option 4 extra: `R13` light and dark, `L1` at 360.
- List every file written in `docs/progress/android-design-options.md`.
