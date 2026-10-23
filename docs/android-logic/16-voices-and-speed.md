# 16 · Voices and speed

Status: **decided** (the int8 bundle's voice list is checked by the spike).

## 1. Goal

Let the user hear and pick one of the 28 English voices and a speed from 0.75× to 2×, change them while listening without
a gap longer than one sentence, and have each book remember its own choice. Default voice `af_heart` ("Heart").

## 2. User flow

1. Player sheet → **Voice** / "Heart · US · female" ⌄ → six rows (the current voice first, then the other five of its group)
   + "All 28 voices" → the full list grouped "American English · female / male", "British English · female / male".
2. Each row: a radio, the name, and a 48 dp **sample** button ("Hear Bella"). Tap the sample → it plays the sample line in
   that voice. Tap the row → that voice is now this book's voice.
3. **Speed:** the player sheet's segmented control, the long-press scrubber (pill, Listen view), or the Listen view's Speed
   sheet. Six steps.
4. Settings → Default voice / Default speed set what new books start with.
5. Export picks its own voice (default: the book's) and speed (default 1×), file 10.

## 3. Rules & logic

### 3.1 Voices

- From `list_voices()` (the engine's voices + the default id). Names and groups come from the core (`VoiceRecord { id,
  name, language: EnUs|EnGb, gender }`); the row subtitle is "{US|UK} · {female|male}" in the pill sheet and "{American |
  British} English · {female | male}" in lists.
- Order inside a group: as the core lists them. The current voice is marked (radio on, `accentContainer` row).
- **Per-book voice:** DataStore `voice.<docId>`; a book without one uses `defaultVoice`. Changing the voice in the player
  changes **this book only**.
- Applying a voice while playing: the current sentence restarts in the new voice (the window is rebuilt at the current
  item, file 09); read-ahead re-plans for the new voice; the old voice's queued jobs are cancelled (`stop_read_ahead`). While
  paused: nothing synthesizes until play.
- A saved voice id that the engine doesn't have (`VoiceUnknown`, e.g. a model change) → fall back to the default voice for
  that book, rewrite the per-book value, log.

### 3.2 Samples

- **One fixed sample sentence, the same for every voice and both models** (user 2026-10-24), invented by us (first brief
  §10): **"This is how I sound when I read your books aloud."** Only the voice changes, so voices compare fairly.
- `speak_voice_sample(voiceId)` (core C16), priority Now. **Made on demand, then cached on disk** under
  `cacheDir/lectrice/samples/<engineId>/<voiceId>.v1.wav`: the first tap waits about 1–2 s (spinner; plus the engine load
  if it isn't loaded), every later tap plays at once, also after a restart. If Android clears the cache, the next tap makes
  it again.
- **Why on demand and not all 28 after the download:** making them all costs 40–60 s of full CPU on a green phone (minutes
  on a red one) right when the user wants to start, for voices most people never try, and Android may clear them anyway.
  The default voice's sample is made on S4 (file 01 §3.7), so it is always instant.
- Played by a separate small player (not the MediaSession). If the book is playing, **its audio pauses for the sample and
  resumes after it** (if it was playing). Tapping another sample stops the first.
- While a sample is being made: the button shows the 16 dp spinner; while playing: a 3-bar equaliser in `accent`; tap again
  = stop.
- Without a model there is no list and no sample (see §4).

### 3.3 Speed

- Exactly **0.75×, 1×, 1.25×, 1.5×, 1.75×, 2×** (stored as percent 75…200). No free slider.
- Applied at once with `setPlaybackParameters` (pitch kept, ExoPlayer's Sonic). Audio stays 1× in the cache; the new speed
  goes into the next `SpeakArgs.rate`, so read-ahead and the cache window scale (file 08).
- **Per-book speed:** DataStore `speed.<docId>`; new books use `defaultSpeedPercent`.
- Shown: pill status at ≥ 400 dp ("· 1.5×"), the sheet's segmented control, the Listen view's Speed button.
- TalkBack: "Speed, 1.5 times" on every control.

### 3.4 Defaults (Settings)

- Default voice: the grouped list with samples (same component), `af_heart` until changed.
- Default speed: 1×.
- Changing a default never changes books that already have their own value.

## 4. States

| State | Shown |
|---|---|
| No model | the Voice row shows the book's saved voice name ("Heart" by default, stored with the id); expanding it shows **new** "Voices appear after the voice download." + "Download" (→ S2) |
| Loading the list | at most one engine call; the rows come from cache after the first load |
| Sample failing | the button shows the error icon for 2 s; snackbar "Speech generation failed." (registry) |
| Offline | no difference |

## 5. Edge cases

- **int8 bundle with a different voice set** (UNVERIFIED): the list follows the loaded engine; books whose voice is missing
  fall back (§3.1).
- **British voices use the US lexicon** in v1 (web spec §10); they sound a little American on some words (known).
- **Speed change during "Preparing audio…":** applied when audio starts.
- **Rapid voice switching:** each switch cancels the previous voice's queued read-ahead; at most one wasted sentence per
  switch.

## 6. Where it lives

| Piece | Module |
|---|---|
| `VoiceList` (grouped, with samples), `SpeedSegmented`, `SpeedScrubber` | `core:designsystem` |
| `VoiceRepository` (`voices()`, `sample(id)`) | `core:model` / `core:data` |
| Per-book values | `SettingsStore` (`voiceFor(docId)`, `speedFor(docId)`) |
| Sample player | `core:playback` (`SamplePlayer`, separate from the session) |
| Core | `list_voices` (existing), **new** `speak_voice_sample` (C16) |

## 7. Data and persistence

DataStore: `defaultVoice`, `defaultSpeedPercent`, `voice.<docId>`, `speed.<docId>`.
Samples: `cacheDir/lectrice/samples/<engineId>/<voiceId>.v1.wav` (core C16, C29); shown under Android's "Cache".

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Default voice `af_heart`, 28 English voices | user | 2026-10-21 |
| Speeds 0.75–2× in six steps | web v1 / design | 2026-10-21 |
| Per-book voice and speed; Settings sets defaults for new books | Opus | 2026-10-24 |
| Sample pauses the book and resumes after | Opus | 2026-10-24 |
| One fixed sample sentence for every voice and model | user | 2026-10-24 |
| Samples made on demand and cached on disk (not all after the download) | Opus | 2026-10-24 |

## 9. Done when

- [ ] Picking Bella while playing restarts the current sentence in Bella within one sentence; the next book still uses Heart.
- [ ] Each sample plays its own voice; the book resumes after.
- [ ] Speed changes are instant; the next plan log line shows the new rate.
- [ ] Per-book voice and speed survive a restart.

## 10. Design screens

R4, R5, E1v, S4 (sample card), R1s, R13s.
