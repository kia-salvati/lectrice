# 09 · Background playback, lock screen and media controls

Status: **decided** (the Samsung Now Bar is UNVERIFIED until tested on the S26 Ultra).

## 1. Goal

Keep reading aloud with the screen off or the app in the background, controllable from the notification, the lock screen
(and Samsung's Now Bar), Bluetooth, earbuds and headsets. The system card shows **the sentence being read** (a setting can
show the section title instead). Next/previous everywhere means the next/previous **sentence**.

## 2. User flow

1. Press play in the reader. A media notification appears; on the lock screen the media card (N2) or the Now Bar (N2s)
   shows the current sentence (one line, cut with "…") and "Ch. 7 · The Long Book".
2. Lock the phone: audio continues. Play/pause, previous and next work from the card, the shade, earbuds and Bluetooth.
3. A phone call or another app's audio pauses Lectrice; it resumes after a short interruption if it was playing.
   Unplugging headphones pauses.
4. Tap the notification → the app opens on the reader (or the Listen view, if it was open).
5. After the app was killed, "play" on the system media controls (or a headset button) resumes the last book at its saved
   place.

## 3. Rules & logic

### 3.1 Pieces (spec §7.1)

- `PlaybackService : MediaSessionService` with one `ExoPlayer` and one `MediaSession`. Manifest
  `foregroundServiceType="mediaPlayback"`; permissions `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MEDIA_PLAYBACK`, `WAKE_LOCK`
  (and `POST_NOTIFICATIONS`, file 17).
- ExoPlayer: `AudioAttributes(USAGE_MEDIA, AUDIO_CONTENT_TYPE_SPEECH)`, `handleAudioFocus = true` (speech content **pauses**
  instead of ducking), `setHandleAudioBecomingNoisy(true)`, `setWakeMode(C.WAKE_MODE_LOCAL)`.
- The UI talks to it through a `MediaController`, adapted to `PlaybackController` (`core:model`), so the reader never sees
  Media3.

### 3.2 The playlist window (spec §7.2)

- One `MediaItem` per playable sentence or cue: `mediaId = "<docId>:<sentenceId>"`, `uri =
  lectrice://sentence/<docId>/<sentenceId>`.
- The source order is `playback_order(docId, readSkipped)` (core), held in memory as an `IntArray`.
- **Window of 200 items** around the current one (50 before, 150 after). When **fewer than 50** remain ahead, append the
  next 200 and drop the oldest beyond 50 behind. A jump outside the window, a voice change or a read-code change **rebuilds**
  the window around the target (`setMediaItems(items, startIndex, 0)`).
- `onMediaItemTransition` → sentence id → `PlaybackState.currentSentenceId` → the reader's highlight, the Listen view, the
  position save (file 13).
- At the end of the window and of the book: `STATE_ENDED` → status Finished; position = the last playable sentence.

### 3.3 Metadata per item (v2 brief §2)

| Field | Setting "Sentence on the lock screen" **on** (default) | **off** |
|---|---|---|
| `title` (`DISPLAY_TITLE`) | the sentence text (cue items: "Code example." / "Table."), whitespace collapsed, cut to **120 chars** + "…" (the system shows ~35–40 anyway) | the current section title (file 07 §3.3 Listen view line 2) |
| `artist` | `Ch. 7 · The Long Book` (file 07 §3.3) | the book title |
| `albumTitle` | the book title (cars may show it) | the book title |
| `artworkUri` | **one** shared `content://` URI for the book's cover tile PNG (file 02 §3.4) so it isn't decoded per item | same |

- Metadata is set when items are built; changing the setting rebuilds the window's metadata
  (`replaceMediaItems`, playback not interrupted).
- Per-sentence title changes happen every ~5 s, well under the notification rate limit (each sentence is already an item).
- **Media items carry book text: Media3 `EventLogger` is never enabled in release builds** (README §4.6).

### 3.4 Commands and buttons

| Control | Action |
|---|---|
| Play / pause (card, shade, headset single press, Bluetooth) | play / pause |
| Next (card, headset double press, Bluetooth next) | **next sentence** (`seekToNextMediaItem`) |
| Previous (card, headset triple press, Bluetooth previous) | **previous sentence**, strictly (`setMaxSeekToPreviousPositionMs(0)` so it never restarts the current item) |
| Seek bar | removed: the session drops `COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM` (per-sentence seeking means nothing). Whether One UI then hides the bar is UNVERIFIED; book-level seeking is in the player sheet |
| Stop / dismiss the notification while paused | stops the service, saves the position, `stop_read_ahead` |

Media3's default notification and button layout are used (no custom layout in v1).

### 3.5 The DataSource (spec §7.3)

`SentenceDataSource : BaseDataSource(isNetwork = false)`:

1. `open(dataSpec)`: parse the URI; read the current **per-book voice**, the **speed** and the global `readSkipped` from the
   playback settings holder (file 16); call `rustCore.speakSentenceBlocking(SpeakArgs(...))`; keep the bytes; return their
   length.
2. `read()` copies from the array; `close()` drops it.

Under `ProgressiveMediaSource.Factory(SentenceDataSource.Factory(...))`; the default WAV extractor reads it.

Errors from `open`:

| Error | Handling |
|---|---|
| `SpeechSynthesisFailed` | retry once; if it fails again, **skip** that item (`seekToNextMediaItem`) and show the snackbar **new** "Couldn't read one sentence, so it was skipped."; 3 failures in a row → pause, status Voice unavailable, R9 |
| `SpeechEngineUnavailable` / `ModelMissing` | pause; status per file 05 §3.2 |
| `SentenceNotFound` | rebuild the window from `playback_order` (the document changed); log |
| `VoiceUnknown` | switch this book to the default voice, rebuild, log (file 16) |

### 3.6 Speed

`player.setPlaybackParameters(PlaybackParameters(rate))` (Sonic inside ExoPlayer, pitch kept). Audio is always made at 1×;
`rate` also goes into `SpeakArgs` to size read-ahead (file 08).

### 3.7 Audio focus and noisy events

- Permanent focus loss (another player starts) → pause; Lectrice doesn't resume by itself.
- Transient loss (a call, a navigation prompt) → pause; resume on gain **if** it was playing (Media3 default).
- "Can duck" loss → pause (speech; Media3 does this for `CONTENT_TYPE_SPEECH`).
- Headphones unplugged / Bluetooth disconnected → pause (`becomingNoisy`).

### 3.8 One book at a time

- The service plays one document. Opening **another** book's reader while one plays: the playing book is **paused**, its
  position saved, `stop_read_ahead` on it; the new book is loaded into the player paused at its saved place.
- Going back to the Library while playing: playback continues; the Continue card shows pause for that book.

### 3.9 Process death and resumption

- `MediaSession.Callback.onPlaybackResumption` (Media3) rebuilds the window from the last played document's saved position,
  so the system's media "play" (Android 13+ media resumption, headsets) continues the last book. The voice/speed come from
  DataStore.
- The last played document id is stored in DataStore `lastPlayedDocumentId` on every play.

### 3.10 Notification tap

The session activity is a `PendingIntent` to `MainActivity` with the document id; the navigator opens (or brings forward)
`Reader(docId)`; if the Listen view was open, it stays open (view state).

### 3.11 The Now Bar (Samsung One UI 7+)

Fed by the same MediaSession (title/artist/artwork/buttons); no Samsung-specific code. UNVERIFIED that our app shows there;
checked on the S26 Ultra in M3 (N2s, N2se). The S10+ (One UI 4.1) shows the classic card (N2).

## 4. States

| State | Notification / lock screen |
|---|---|
| Playing | card with pause; title = sentence |
| Paused | card with play; stays until dismissed (swipe) or 10 min without play (then the service stops itself and the card goes; the position is saved) |
| Preparing / catching up | card shows the item as playing (Media3 shows buffering as playing); no special text |
| Finished | card with play; title "Finished" (**new**), artist = book |
| Error (voice) | paused card |
| Notifications denied (API 33+) | the media card on the lock screen and in the shade still appears if media notifications are exempt (UNVERIFIED; file 17); otherwise only in-app controls |

## 5. Edge cases

- **Screen off for a whole chapter:** the wake lock keeps synthesis going; read-ahead stays ahead (spike M-i).
- **Doze / Samsung app sleeping:** a foreground `mediaPlayback` service is exempt while playing. When paused for 10 min the
  service stops (normal).
- **Bluetooth car display:** shows title/artist (maybe the album); next = next sentence.
- **Very long sentence title:** cut to 120 chars before it reaches the system.
- **Read-code toggled from the lock screen:** not possible; only in the app.
- **App updated while playing:** the process restarts; playback stops; media resumption can continue it.
- **Two quick "next" presses on a headset:** each is a sentence; the window handles it.
- **The book is a cue-only stretch** (several code blocks in a row): each cue is an item; titles "Code example.".

## 6. Where it lives

| Piece | Module |
|---|---|
| `PlaybackService`, ExoPlayer setup, `SentenceDataSource`, playlist window, metadata builder, `MediaControllerPlaybackController` | `core:playback` |
| `PlaybackController`, `PlaybackState` | `core:model` |
| Cover PNG provider (`content://` for artwork) | `core:playback` (a small `FileProvider` path) |
| Core | `playback_order`, `speak_sentence`, `save_position`, `stop_read_ahead` (C13) |

## 7. Data and persistence

DataStore: `lastPlayedDocumentId`, `lockScreenSentence` (default true). Position: file 13. Nothing else.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Lock-screen / Bluetooth next = next sentence | user | 2026-10-23 |
| The sentence is the media title, on by default with a setting for the section title | approved v2 design | 2026-10-23 |
| No seek bar on system surfaces | approved design (first brief Q7) | 2026-10-23 |
| Strict previous (never restart the current sentence) | Opus | 2026-10-24 |
| Opening another book pauses the playing one | Opus | 2026-10-24 |
| The service stops after 10 min paused | Opus | 2026-10-24 |
| Skip a sentence after a failed retry, stop after 3 in a row | Opus (new copy) | 2026-10-24 |

## 9. Done when

- [ ] A whole chapter plays with the screen off on both phones, controlled from earbuds (spec M3 line).
- [ ] Lock-screen card: title = the current sentence (one line), artist = "Ch. 7 · …"; with the setting off: section title
      + book.
- [ ] Next/previous from the card, a headset and Bluetooth step exactly one sentence.
- [ ] A phone call pauses and resumes; unplugging pauses.
- [ ] Kill the app (swipe away while paused, then `adb shell am kill`), press play on the media controls → the last book
      continues at its place.
- [ ] Release builds: no `EventLogger`, and logcat has no sentence text (grep test over a 10-minute session).
- [ ] Now Bar checked on the S26 Ultra; result written in the M3 progress report.

## 10. Design screens

N1, N2, N2s, N2se, N2L (file 06), R10 / R10b (file 17).
