# 13 · Reading position and progress

Status: **decided**.

## 1. Goal

Never lose the user's place: save the current sentence often and cheaply, reopen every book where it was left, and show
honest progress (percent read, time left) in the Library, the player and the Listen view.

## 2. User flow

1. The user listens, taps a sentence, jumps by chapter or slider: the place follows.
2. They leave (back, home, lock, kill): the place is already saved.
3. Library: the Continue card and rows show "49 % · 4 h 12 min left" / "Continue · 34 %" / "Start".
4. Opening the book (or media "play" after the app was killed) resumes at the saved sentence.
5. At the end of the book: "Finished"; play starts again from the beginning.

## 3. Rules & logic

### 3.1 What a position is

- One **sentence id** per document (`meta.json` `lastSentenceId`), the same as on the desktop. Not a scroll offset, not a
  time.
- The current sentence = the one the player is on (`onMediaItemTransition`), or the one the user tapped/jumped to while
  paused.

### 3.2 When it's saved (`save_position(docId, sentenceId)`, off the main thread)

| Event | Save |
|---|---|
| The current sentence changes while playing | at most once every **5 s** (trailing debounce: the last id in the window is saved) |
| Tap-to-jump, chapter / section / slider jump while paused | at once |
| Pause, stop, sleep-timer stop, audio-focus loss | at once |
| App goes to the background (`ON_STOP`) | at once (flush the pending debounce) |
| Another book opens | the old book at once |
| Reader opened without playing or tapping | nothing (opening doesn't move the place) |

- The core writes atomically (temp + rename), so a crash never corrupts `meta.json`.
- A failed save (`StorageFull`, `StorageFailed`) is logged and retried at the next event; never shown.
- At most 5 s of listening can be lost if the process is killed while playing (accepted).

### 3.3 Progress numbers (README §4.7)

- **Percent read** = `round((lastSentenceId + 1) ÷ sentenceCount × 100)`, capped at 100; `null` (never started) → "Start".
  (Sentence ids count cues too; small skew, same as the web.)
- **Time left in the book** = chars of playable sentences after the current one ÷ 18 ÷ the book's speed → "4 h 12 min left"
  (format: "under a minute left", "{m} min left", "{h} h {mm} min left" with the minutes not zero-padded on the phone, as the
  mockup: "4 h 5 min").
- **Chapter percent / time left in the chapter:** file 07 §3.2.
- The **Library** computes time left for the Continue card only (it needs the document's text, file 02 §3.2); rows show
  percent only.

### 3.4 Finished

- A book is **finished** when the player reaches `STATE_ENDED` after the last playable sentence; the position saved is that
  last sentence (100 %).
- Status "Finished · play to start over"; play → the first playable sentence (position saved at once).
- Library card: "Finished · listen again" (file 02).
- Tapping any sentence of a finished book just moves the place there (not finished any more).

### 3.5 Opening at the saved place

- `get_document` returns `lastSentenceId`; the reader scrolls it to 35 % (file 04) and the player builds its window around
  it (file 09).
- If the saved sentence isn't playable in the current order (e.g. a code sentence while "Read code" is off), the player
  starts at the next playable one (core start rule); the highlight shows that one.
- `null` → the first playable sentence; the reader opens at the top.

### 3.6 Re-cleaned books

When the cleaner version rises, the core re-cleans and keeps `lastSentenceId` if it is still `< sentenceCount`
(`merged_with_saved`), else drops it (the book shows "Start"). Sentence ids can shift a little, so the place may land a few
sentences off. Accepted (rare: only after an app update that raises the cleaner version).

### 3.7 Per-book listening settings

The book's voice and speed are remembered per book (file 16), so "time left" uses the book's own speed.

## 4. States

| State | Shown |
|---|---|
| Never started | "Start" (rows), no Continue card |
| In progress | percent + time left |
| Finished | "Finished · listen again" |
| Save failing | nothing visible; logged |
| Offline | no difference |

## 5. Edge cases

- **Two devices** (desktop and phone): positions are per device; no sync (no server, user decision).
- **The same book opened again after "already in library":** resumes (L6 snackbar).
- **Clock changes** (time zones): `lastOpenedAt` comes from the core clock (UTC); ordering stays correct.
- **Rapid jumps** (slider scrubbing): only the release jumps; one save.
- **Kill during the 5 s debounce:** the previous saved id stays (≤ 5 s lost).
- **A position past the end** (corrupt file): the core filters it (`< sentenceCount`).

## 6. Where it lives

| Piece | Module |
|---|---|
| Debounced saver, lifecycle flush | `core:playback` (it sees every transition) + `ReaderViewModel` (paused jumps) |
| `percentRead`, `timeLeft`, `formatTimeLeft` | `core:model` (ports of `frontend/src/utils/progress.ts`, `playbackOrder.ts`'s `secondsLeft`, `time.ts`) |
| Core | `save_position`, `get_document` (`lastSentenceId`), `list_recent` — existing |

## 7. Data and persistence

Core `library/<docId>/meta.json`: `{ addedAt, lastOpenedAt, lastSentenceId }`. DataStore: `lastPlayedDocumentId`
(file 09).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Save the reading position; recent documents show progress + Continue | user | 2026-10-21 |
| Debounce 5 s while playing, save at once on pause/stop/background | spec §7.2 (proposed there, kept) | 2026-10-23 |
| Opening a book doesn't move its place | Opus | 2026-10-24 |
| Finished = ended after the last sentence; play restarts | Opus (web behaviour) | 2026-10-24 |

## 9. Done when

- [ ] Play 30 s, kill the app (`adb shell am kill`): reopening resumes within 5 s of where it stopped.
- [ ] Pause, tap a sentence, background the app, kill it: reopening is at the tapped sentence.
- [ ] Library percentages match `percentRead` (JVM tests on invented numbers).
- [ ] A finished book shows "Finished · listen again" and play starts at the top.
- [ ] No sentence text in the save log lines (ids only).

## 10. Design screens

L1 (card and rows), R4 (slider and times), R13 (chapter progress).
