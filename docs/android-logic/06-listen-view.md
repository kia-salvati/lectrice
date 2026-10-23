# 06 · Listen view

Status: **decided**.

## 1. Goal

A calm, eyes-free "stage" for listening on the move: the current sentence large in the middle, the previous and next
ones smaller around it, and big controls in the thumb zone. Tap anywhere on the text to pause, swipe sideways to step a
sentence. It lives *inside the pill*: tapping the pill's status grows the pill into it, and back shrinks it into the pill
again, returning to the text at the same sentence. Optionally (a setting, off by default) it stays above the lock screen.

## 2. User flow

1. In the reader, tap the pill's status → the pill grows into the Listen view (container transform, 320 ms).
2. The first time: a hint above the thumb zone (R13h): "Tap the text to pause. Swipe sideways for the next or previous
   sentence." + "Got it".
3. Tap the text → play/pause (a 72 dp round icon flashes 600 ms in the middle). Swipe left/right → next/previous sentence
   (the teleprompter glides 320 ms).
4. Thumb zone: chapter progress; ⏮ ⏯ ⏭ (64 / 88 / 64); "≪ Section" / "Section ≫"; a row of **Speed · Sleep · Chapters ·
   More**.
5. **Speed** → a small speed sheet (R13s). **Sleep** → a small sleep sheet. **Chapters** → the outline (R13k); picking an
   entry jumps there and **stays in the Listen view**. **More** → the player sheet at half height (R13m) with everything
   (voice, export, library…).
6. Back to reading: the chevron (top left), the system back gesture (predictive preview R13b: the stage shrinks toward the
   pill), or dragging the top bar down. All three return to the reader **at the current sentence** with follow-scroll on.
7. With the setting "Listen view over the lock screen" on: if the Listen view is open when the screen turns off, it shows
   above the lock screen when the screen turns on again (N2L).

## 3. Rules & logic

### 3.1 Layout (412 × 915; 360 × 800 values in brackets; v2 brief §5.3)

- Full screen, `bg` (white / dark / black, same as the reader), `fg`. Status bar on top; no scrim needed.
- **Top bar** (y = status inset, height 56): `expand_more` icon button 48 at x 4 ("Back to reading"); text block from x 56 to
  right 16: line 1 `body-s` `fg2` = book title; line 2 16/22 500 `fg` = **current section title** (the TOC path entry after
  the chapter; else the chapter title; else "p. {label}"), 1 line, ellipsis. The whole bar is the **drag-down handle**.
- **Teleprompter** (top 104, left/right 16, bottom 336 [320]): a column centred vertically, gap 16:
  - previous playable sentence: Noto Serif 20/30 400 `fg2`, max 3 lines, clipped **at the top**;
  - **current**: Noto Serif 28/40 500 `fg` [26/36], with a **3 dp `accent` bar** 8 dp left of the text (top 6 to bottom 6,
    radius 2); if it runs past 6 lines it drops to 24/34;
  - next playable sentence: 20/30 `fg2`, max 3 lines, clipped **at the bottom**;
  - 24 dp fades at top and bottom (gradient from `bg` to transparent, drawn over the text, never text opacity).
- **Thumb zone** (left/right 16, bottom 40 + gesture inset, column, gap 16):
  1. Chapter progress: 4 dp track `line`, active `accent`, 2 × 8 dp `fg2` ticks at the starts of the chapter's child
     entries; under it, left "38 % of chapter 7" (chapter without a number: "38 % of {title}", ellipsis; no TOC: "49 % of the
     book" and the bar shows the book), right "9 min left" (preparing: 16 spinner + "Preparing audio…").
  2. Transport (gap 24 [16]): ⏮ 64 circle (`bg2`, 1 dp `line`, icon 32 `fg`) · **⏯ 88** circle (`accent`, icon 36
     `onAccent`; preparing = the R3 ring scaled) · ⏭ 64.
  3. Section row: two buttons, each flex 1, height 48, radius full, `bg2` + 1 dp `line`, `label` 14/20 500: "≪ Section"
     ("Previous section") and "Section ≫" ("Next section"). Rules: file 07 §3.5.
  4. Tool row: four 64 × 64 buttons, `space-around`: **Speed** (shows the value "1×" 16/24 500 over the caption "Speed"),
     **Sleep** (`bedtime`; on: icon `accent`, caption "28 min"), **Chapters** (`toc`), **More** (`more_horiz`, "Player and
     settings").
- No shadows, no mode toggle, no extra tokens.
- Landscape: teleprompter in the left 55 %, thumb zone in the right 45 %, same order top to bottom (README §4.3).

### 3.2 What the teleprompter shows

- The three sentences are **neighbours in playback order** (`playback_order`), so cues and read-code settings apply
  exactly as playback does.
- **Code or table as the current item (R13c):** the current slot shows a card (`bg2`, 1 dp `line`, radius 12, padding 16):
  title `label` "Code example" (or **new** "Table"), the block's first 3 lines in Roboto Mono 14/20 `fg2` (right-edge fade
  if cut), and the caption `caption` `fg2` "Read as “Code example.”" (or "Read as “Table.”"). With "Read code & tables
  aloud" on, the card shows the **current code sentence** in mono (3 lines max) and no "Read as" caption.
- A **heading** as the current item: shown in the current slot at the same size, weight 600.
- When paused, nothing moves; the current sentence stays.

### 3.3 Gestures (thresholds README §4.1)

| Gesture | Where | Result |
|---|---|---|
| Tap | teleprompter | play/pause + 600 ms centred icon flash (72 dp `bg2` circle with the new state's icon) |
| Horizontal swipe ≥ 48 dp | teleprompter | next (left) / previous (right); 320 ms glide (sentences slide up for next, down for previous) |
| Tap | ⏮ ⏯ ⏭, section, tool buttons | as labelled |
| Long-press 500 ms | ⏯ | speed scrubber above the 88 button (same as the pill) |
| Vertical drag down | top bar | the view follows (translateY; scale 1 → 0.9 over the first 300 dp); release past 30 % of the screen height or a downward fling → close; else spring back |
| Back gesture | anywhere | predictive: the stage scales to 0.9 toward the pill (`transform-origin` bottom centre, translateY 24, radius 32, 1 dp `line`), the reader behind; release → close; cancel → spring back |

No double-tap. Vertical swipes on the teleprompter do nothing (they'd be confused with the drag-down and with scrolling).

### 3.4 Opening and closing (container transform)

- Open: measure the pill's bounds; the stage starts clipped to the pill's rounded rect (radius 28) and grows to full screen
  in **320 ms** (default spring, slight overshoot). Its content fades in from 120 to 320 ms; the play circle moves from the
  pill's ⏯ to the 88 position, scaling (shared bounds). R13t shows the 50 % frame.
- Close: the reverse, into the pill. Before it starts, the reader is already scrolled so the current sentence sits at 35 %
  with follow-scroll on, so the text is right when the stage is gone.
- Reduced motion: both instant.
- The view can only be opened from the expanded pill (the collapsed button has no status). After closing, the pill is
  **expanded**.

### 3.5 Sheets over the Listen view

| Button | Sheet | Behaviour |
|---|---|---|
| Speed | R13s: small auto-height sheet (`bg2`, top radius 28, handle, header "Speed" + close) with the 6-segment control | a tap applies at once and closes the sheet |
| Sleep | small sheet with the sleep chips (file 15) | a tap applies and closes |
| Chapters | the full-height outline (file 07) | an entry → jump, the sheet closes, **the Listen view stays** with that entry's first sentence current |
| More | the player sheet at half (file 05 §3.6) | as in the reader; its "Library" row closes the Listen view and goes to the Library |

### 3.6 Over the lock screen (setting, off by default)

- Setting: Settings → Listening → "Listen view over the lock screen" / "Keeps the Listen view up when the screen turns off
  while it's open." (switch, **off**).
- When on **and** the Listen view is open, `MainActivity.setShowWhenLocked(true)`; when it closes, or the setting is turned
  off, `setShowWhenLocked(false)`. Never `setTurnScreenOn`. (Apps can't put themselves over the lock screen otherwise;
  full-screen intents are for calls and alarms, v2 brief §2.)
- While shown over the lock screen (`KeyguardManager.isKeyguardLocked`), N2L: a 16 dp `lock` icon in the status area,
  "Swipe up to unlock" (`caption` `fg2`) at the very bottom; the thumb zone moves up by 24.
- **Allowed while locked:** play/pause, ⏮ ⏭, swipes, section jumps, Speed, Sleep, Chapters (jumping inside the book).
- **Needs unlocking:** the chevron / back / drag-down (going to the reader), **More** (the player sheet holds library,
  export, save/share). These call `KeyguardManager.requestDismissKeyguard`; on success the action continues; on cancel
  nothing happens.
- The window is dimmed by the system's normal screen timeout (we don't keep the screen on). The lock-screen media card
  still exists (file 09).

### 3.7 Live region and TalkBack

- The current sentence is a live region **only while paused** (when the user steps with ⏮ ⏭ or a swipe). While playing, no
  announcements (the audio is already speaking).
- The teleprompter's tap target is labelled "Play" / "Pause" with the custom actions "Next sentence", "Previous sentence".
- Focus starts on the chevron when the view opens.

### 3.8 Hint (R13h)

Shown once ever, on the first open, 12 dp above the thumb zone (`inverseBg` tooltip + "Got it"). Counts as seen after
any teleprompter tap or swipe. DataStore `hint.stage`.

## 4. States

| State | What shows |
|---|---|
| Playing / paused | R13 / R13p |
| Preparing | right progress text "Preparing audio…" with spinner; ⏯ with the ring |
| Catching up | right progress text **new** "Catching up…" with spinner |
| Finished | current slot: **new** "Finished" (Noto Serif 28 `fg`) with the last sentence above it; ⏯ = play from the start |
| No voice / engine failed | the Listen view can't open (the status tap goes to S2 / R9, file 05) |
| No TOC | progress shows the book; section buttons disabled; Chapters opens R8c |
| Offline | no difference |
| 200 % font | the current sentence 28 × 2 drops to the 24/34 step first, then clips at 6 lines with an ellipsis; thumb-zone labels wrap to 2 lines |

## 5. Edge cases

- **A very long sentence** (a run-on paragraph cleaned as one sentence): 24/34 and clipped at the bottom with the fade; the
  full text stays readable in the reader.
- **The last / first sentence:** the next / previous slot is empty.
- **A jump from the outline to a heading:** the heading becomes current; the previous slot shows the sentence before it.
- **Screen off with the setting off:** the Listen view is still open after unlock (we didn't close it); playback continued
  in the background.
- **Rotation:** the view stays open (`rememberSaveable listenOpen`), re-laid out per orientation.
- **Process death while open:** restored as reader state; reopens the Listen view at the saved sentence, paused.
- **Notification tap while the Listen view is open:** brings the app to the front, unchanged.
- **Over the lock screen, the user taps More and cancels the unlock:** stays on the Listen view.

## 6. Where it lives

| Piece | Module |
|---|---|
| `ListenView` (top bar, `Teleprompter`, `ThumbZone`, `ToolRow`), `ContainerTransform` helper | `feature:reader` (screen) / `core:designsystem` (parts) |
| State: `listenOpen` (view state), playback from `PlaybackController` | `ReaderViewModel` |
| Show-when-locked switch | `app` (`MainActivity` observes `listenOpen` + the setting) |
| Neighbour sentences | pure helper `neighbours(order, currentId)` in `core:model` |
| Core | `playback_order` (existing) |

## 7. Data and persistence

- DataStore: `listenOverLockScreen` (default false), `hint.stage`.
- View state: `listenOpen` (`rememberSaveable`).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Option 4's stage in Quiet tokens, opened from the pill's status, closed by chevron / back / drag-down | user (approved v2) | 2026-10-23 |
| Listen view over the lock screen: a setting, **off by default** | user | 2026-10-23 |
| Which actions need unlocking (§3.6) | Opus | 2026-10-24 |
| Live region only while paused | Opus | 2026-10-24 |
| "Table" card title, "Finished" and "Catching up…" texts | Opus (new copy) | 2026-10-24 |
| Landscape two-column layout | Opus (not drawn yet) | 2026-10-24 |

## 9. Done when

- [ ] Status tap opens the view with the transform; all three ways back return to the reader at the current sentence at
      35 %, follow-scroll on.
- [ ] Tap pauses/plays with the flash; swipes step one sentence; vertical motion on the text does nothing.
- [ ] Chapters from the view jumps and stays in the view.
- [ ] Code cue shows the R13c card; with read code on, the card shows the code sentence.
- [ ] With the setting on: open the view, press power, press power again → the view is above the lock screen with N2L
      marks; play/pause works; More asks to unlock. With the setting off: the normal lock screen.
- [ ] Reduced motion: no transform, no glide, no flash animation (the icon still shows 600 ms).
- [ ] 360 dp matches R13-light-360; black matches R13-black.

## 10. Design screens

R13, R13p, R13c, R13h, R13t, R13b, R13s, R13m, R13k, N2L.
