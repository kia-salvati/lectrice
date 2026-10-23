# 15 · Sleep timer

Status: **decided**.

## 1. Goal

Let the user fall asleep listening: playback stops by itself after a chosen time or at the end of the chapter, gently,
at a sentence boundary, without losing the place.

## 2. User flow

1. Player sheet → **Sleep timer** / "Off" ⌄ → chips **Off · 15 min · 30 min · 45 min · 1 h · End of chapter** (R6). Or the
   Listen view's **Sleep** button → a small sheet with the same chips.
2. Pick one → the row reads "Stops at 23:41" (or **new** "At the end of chapter 7"); the pill shows ☾ + "28 min"; the Listen
   view's Sleep button turns `accent` with "28 min".
3. When the time is up, the current sentence finishes, the volume fades out and playback pauses.
4. Pick "Off" to cancel.

## 3. Rules & logic

- **Counting:** the timer counts down **only while playing** (paused time doesn't count). "Stops at HH:MM" =
  now + remaining, recomputed when playback resumes; 24-hour or 12-hour per the phone's setting.
- **The end:** when the remaining time reaches 0, let the **current sentence finish**, then pause. During that last sentence
  the volume fades from 100 % to 0 over its last 3 s (or the whole sentence if shorter); volume is restored after pausing.
- **End of chapter:** pause **before the first sentence of the next chapter-level entry** (file 07 §3.2: the current
  chapter's end). Disabled (chip `aria-disabled`, `line` text) when the book has no TOC, or when the current place is before
  the first entry. Label shows **new** "At the end of chapter 7" (no number: "At the end of {title}").
  - If the user jumps to another chapter, "End of chapter" follows the **new** current chapter.
- **Pill / Listen view text:** remaining minutes, rounded **up** ("28 min", "1 min"); for End of chapter the pill shows ☾ only
  (no minutes) and the Listen view caption reads "Chapter".
- **After it fires:** the timer resets to **Off**; the position is saved (file 13); read-ahead stops **at once** (the read-ahead
  governor's rule 2, file 08 §3.3: the user is going to sleep, so no 60 s wait).
  A snackbar on the next app open is not needed (the user was asleep).
- **Shake to extend:** not in v1.
- **Media buttons while the timer runs:** pause/play keep the timer (counting pauses with playback). Changing book cancels it.
- Not persisted: a process death resets it to Off.

## 4. States

| State | Shown |
|---|---|
| Off | row "Off"; no moon |
| Timed | "Stops at 23:41"; pill "☾ 28 min" |
| End of chapter | "At the end of chapter 7"; pill "☾" |
| Disabled chip | End of chapter without a TOC |

## 5. Edge cases

- Book ends before the timer: Finished; the timer resets.
- Timer of 15 min set while paused: starts counting at play.
- Screen off: the timer runs in `PlaybackService` (not in the UI), so it works with the screen off.
- Speed change: the timer is in wall-clock playing time, unaffected.

## 6. Where it lives

`SleepTimer` in `core:playback` (inside `PlaybackService`, with a coroutine tick every second while playing); its state is
part of `PlaybackState.sleep` (`Off | Timed(remainingMs) | EndOfChapter(chapterEndSentenceId)`). UI: `SleepChips` in
`core:designsystem`; the rows in `feature:reader`. No core change.

## 7. Data and persistence

None (in memory in the service).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Sleep timer in v1 with Off / 15 / 30 / 45 / 1 h / End of chapter | approved design (first brief Q4 drawn in R6) | 2026-10-23 |
| Counts only while playing; finishes the sentence; 3 s fade | Opus | 2026-10-24 |
| Not persisted; resets after firing | Opus | 2026-10-24 |

## 9. Done when

- [ ] 15 min fires after 15 min of playing (a debug speed-up hook for tests), at a sentence end, with the fade.
- [ ] Pausing 5 min in the middle moves "Stops at" by 5 min.
- [ ] End of chapter stops before the next chapter's first sentence; disabled without a TOC.
- [ ] Works with the screen off.

## 10. Design screens

R6, R13 (Sleep button), R1 (pill moon).
