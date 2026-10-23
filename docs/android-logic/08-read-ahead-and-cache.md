# 08 · Read-ahead and the audio cache

Status: **decided** (whether read-ahead alone is enough on a given phone depends on the spike; if not, file 14 adds
Prepare).

## 1. Goal

Make sentence audio before the listener needs it, so playback never waits after the first sentence, while keeping the
phone's storage, battery and CPU use small. The rules are the desktop's (shared core): **10 listening-minutes ahead,
about 1 listening-hour kept per book, a global size cap**, all scaled by playback speed.

## 2. User flow

Invisible. The user notices it only as:

1. Press play → "Preparing audio…" for the first sentence (and the engine load the first time), then continuous audio.
2. The player sheet's slider shows a lighter "buffered" stretch ahead of the thumb.
3. On a slow phone at a high speed: "Catching up…" and the slow-phone tip (file 12).
4. Settings → Read ahead (5 / 10 / 20 min of listening) and Settings → Storage → "Audio cache 312 MB of 1 GB" + "Clear audio
   cache" (file 20). Android's own Settings → Apps → Lectrice → Storage shows it as **Cache**, and its "Clear cache" button
   clears it.

## 3. Rules & logic

### 3.1 Two layers of "ahead"

| Layer | Who | How far | Priority on the speech worker |
|---|---|---|---|
| **Near prefetch** | ExoPlayer, through `SentenceDataSource.open` on its loader thread | the next few playlist items (its default buffer, ~50 s of audio) | `Now` (1) — they are needed soon |
| **Read-ahead** | Rust `ReadAhead`, triggered by every `speak_sentence` | `read_ahead_minutes × rate` minutes of **1× audio** after the requested sentence, in playback order | `ReadAhead` (2) |
| Export / Prepare | Rust export jobs (file 10, 14) | the whole book / a chapter | `Export` (3) |

One speech worker thread, one synthesis at a time; same-key jobs merge (a read-ahead job that playback needs now is moved up).

### 3.2 Formulas (README §4.7)

- `ahead_1x_seconds = read_ahead_minutes × 60 × rate` (rate = the listener's speed, 0.75–2). Default 10 min → at 1.5× the
  core keeps 15 min of 1× audio ahead.
- Estimated length of a sentence = `chars(speech text) ÷ 18` s.
- **Cache window per book + voice + engine:** `keep_minutes_per_document × 60 × rate` seconds (default 60 listening-minutes),
  deleting clips **farthest behind** the current sentence first, then the farthest ahead.
- **Global cap:** 1024 MB on the phone (`cache_max_mb`; the desktop uses 2048). Above it, clips of other books/voices go,
  oldest first. Clips being read or written are never deleted.
- The core refuses a read-ahead that is more than `keep ÷ 1.5` (`MIN_KEEP_TO_READ_AHEAD_RATIO`); with keep 60, the choices
  5 / 10 / 20 pass.
- A WAV clip is ~48 KB per second of audio: 1 hour ≈ 170 MB, so one book's window is ~170 MB at 1×, ~340 MB at 2×.

### 3.3 When it runs and when it stops (the read-ahead governor)

A small Kotlin `ReadAheadGovernor` in `core:playback` owns one value, the **ahead mode**, and is the only code that
starts, shrinks or stops read-ahead. It recomputes the mode on every input event below and applies a change at once.

| Mode | Meaning | Core calls (C13, C14) |
|---|---|---|
| **Full** | the user's setting (5 / 10 / 20 listening-minutes) | `set_read_ahead_minutes(setting)`; `SpeakArgs.plan_ahead = true` |
| **Reduced** | **2 listening-minutes** ahead: enough to bridge sentences and short stalls, about a fifth of the CPU bursts | `set_read_ahead_minutes(2)`; `plan_ahead = true` |
| **Off** | nothing planned; only the items ExoPlayer itself loads (its ~50 s buffer) are made | `stop_read_ahead(docId)` once on entering Off; `plan_ahead = false` on every `speak_sentence` until the mode leaves Off |

`plan_ahead = false` matters because ExoPlayer keeps loading its next items even while paused; without the flag each of
those `speak_sentence` calls would plan a new read-ahead and undo the stop.

**Rules, first match wins (top = strongest):**

| # | Condition | Mode | Enters | Leaves |
|---|---|---|---|---|
| 1 | No current book (stopped, notification dismissed, app swiped away while stopped), **another book opened**, the book was removed (file 21) | Off | at once | play in a book |
| 2 | Sleep timer fired (file 15) | Off | at once (the user is going to sleep) | play |
| 3 | Paused (by the user, audio focus loss, headphones unplugged) for **≥ 60 s** | Off | 60 s after the pause | play |
| 4 | `onTrimMemory(TRIM_MEMORY_BACKGROUND)` or higher while **not playing** | Off, then `unload_engine()` | at once | play (the engine loads again: "Preparing audio…") |
| 5 | Thermal status **CRITICAL or higher** (`PowerManager.getCurrentThermalStatus() ≥ THERMAL_STATUS_CRITICAL`, API 29+, listener `addThermalStatusListener`) | Off | at once | status ≤ MODERATE for **2 min** (then Full, or Reduced if rule 6/7 holds) |
| 6 | Thermal status **SEVERE** while playing | Reduced | at once | status ≤ MODERATE for 2 min |
| 7 | **Battery saver on** (`PowerManager.isPowerSaveMode()`, `ACTION_POWER_SAVE_MODE_CHANGED`), or battery **≤ 15 % and not charging** (sticky `ACTION_BATTERY_CHANGED`; `ACTION_BATTERY_LOW` / `ACTION_BATTERY_OKAY` registered at runtime) | **while playing:** Reduced · **while paused:** Off at once (no 60 s wait) | at once | saver off and (battery > 20 % **or** charging); charging overrides low battery, not the saver |
| 8 | Playing, preparing, catching up, or paused < 60 s | Full | — | — |

**Changes that re-plan without a mode change** (the mode stays Full/Reduced; the old plan's queued jobs are dropped by
`stop_read_ahead(docId)` and the next `speak_sentence` plans the new one):

| Event | What happens |
|---|---|
| `speak_sentence` (each item the player opens) | the core re-plans from that sentence; already-cached sentences are skipped; stale queued jobs are cancelled |
| Speed change | the next `speak_sentence` carries the new rate → re-plan (more or less ahead) |
| Jump (tap, chapter, section, slider) | the next `speak_sentence` is far from the old plan → old queued jobs cancelled, new plan |
| Voice change | `stop_read_ahead(docId)` (drops the old voice's queue), then a plan for the new voice |
| Model (bundle) change | `stop_all_read_ahead()` (C13), `reload_engine(bundle)`, then a plan for the new engine id |
| "Read code & tables aloud" change | `stop_read_ahead(docId)`, new playback order → new plan |

Also: **no playback and no job for 5 min** → `unload_engine()` (spec §5.4); the next play loads it again.

**Exports** follow rules 5–7 too (file 10 §3.7): thermal CRITICAL pauses a running export (reason `System`) until status
≤ MODERATE for 2 min; battery saver or low battery while not charging pauses it as well and it continues on charging.
Rules 1–4 don't touch exports.

In every case the sentence being synthesized **finishes and is cached** (synthesis can't be interrupted); at most one
sentence of work is wasted per stop.

Each mode change is logged once: `read_ahead_mode from=Full to=Off reason=paused_60s` (no book content).

### 3.4 Battery behaviour

- Read-ahead works in **bursts**: it fills to the target and then stops until playback moves on (only new sentences at the
  far end are needed, about one per sentence played). The CPU can sleep in between.
- `ExoPlayer.setWakeMode(C.WAKE_MODE_LOCAL)` keeps the CPU awake **while playing** (synthesis needs it with the screen off).
  When paused, no wake lock; read-ahead stops after 60 s (rule 3), at once in battery saver (rule 7).
- Thread count: the spike's best RTF-per-watt count per phone class (spike plan §5.3), passed in `EngineConfig.num_threads`.

### 3.5 The DataSource contract (summary; file 09 has the playlist)

- `SentenceDataSource.open(uri)` → `speak_sentence(SpeakArgs { document_id, sentence_id, voice_id, rate, read_skipped })`
  blocking on the loader thread → WAV bytes. A cache hit returns in milliseconds.
- While `open` waits, ExoPlayer reports `BUFFERING`: right after play/jump = **Preparing**, mid-play = **Catching up**
  (file 05 §3.8).
- A blocking call can't be cancelled: after a jump, the old sentence finishes and is cached (at most one sentence of wasted
  work; accepted, spec §7.3).
- `SpokenAudio.ready_ahead_ms` (core C25) feeds the buffered part of the slider.

### 3.6 Settings

- **Read ahead:** 5 / 10 / 20 min of listening (default 10). Changing it calls `set_read_ahead_minutes(m)` (core C14) and
  applies from the next sentence.
- **Audio cache:** on the Storage screen (file 20 §3.5): `storage_usage().cache_bytes` of the 1 GB cap; **Clear audio
  cache** (core C15) deletes every clip not in use; per book, "Clear this book's audio" (C28). Playing continues: the next
  sentences are made again. Books, places and exports stay.
- **Android may clear it too** (system "Clear cache", or low storage): handled like a user clear (core C29 treats a missing
  or empty clip as not cached).
- The cap is not user-settable in v1.

## 4. States

| State | What the user sees |
|---|---|
| Cache cold, first play | "Preparing audio…" for the engine load + one sentence (spike targets: first sentence ≤ 1.5 s green, ≤ 3 s amber) |
| Warm | nothing; continuous audio |
| Falling behind | "Catching up…" (file 12) |
| Storage full while caching | silent: the core logs the failed `put`, the clip is still played from memory; read-ahead keeps trying the next ones (each fails fast) |
| Offline | no difference |

## 5. Edge cases

- **Two books in the cache:** the window applies to the current book + voice; the other book's clips stay until the global
  cap removes them (oldest first).
- **Same book, two voices:** separate windows; the unused voice's clips age out under the cap.
- **Speed 2× with 20 min read-ahead:** 40 min of 1× audio ahead (~115 MB); keep window 120 min of 1× (~340 MB): fits under 1 GB.
- **Android deletes clips on its own** (low storage): the same as a clear; the core's index re-checks the disk before it
  reports usage or estimates an export (C29); a `get` of a vanished clip is a miss.
- **Clearing the cache during playback:** the clip being played is in memory (ExoPlayer holds its bytes); the next items are
  made again; the slider's buffered part drops to 0 and grows back.
- **Bundle switch:** clips are keyed by engine id (core C11), so fp32 and int8 clips never mix; the old bundle's clips age out.
- **Cleaner version bump:** keys include the cleaner version, so old clips are never played for a re-cleaned book; they
  age out.
- **Process killed while synthesizing:** a partly written clip is a temp file (atomic write), never read; swept later.

## 6. Where it lives

| Piece | Where |
|---|---|
| Planning, cache, retention, worker priorities | **Rust core** (`lectrice-app/src/speech/read_ahead*.rs`, `worker.rs`; `lectrice-infra/src/storage/audio_cache.rs`) — unchanged logic |
| `stop_read_ahead`, `stop_all_read_ahead`, `SpeakArgs.plan_ahead` | **new core C13** + ffi |
| `set_read_ahead_minutes` | **new core C14** + ffi |
| `storage_usage`, `clear_audio_cache` | **new core C15** + ffi |
| `ready_ahead_ms` | **new core C25** + ffi |
| `ReadAheadGovernor` (the mode rules of §3.3: pause timer, battery, thermal, trim-memory), idle unload (5 min) | Kotlin `core:playback` (`PlaybackService` owns it) |
| Audio cache root in `cacheDir`, missing-clip tolerance | **new core C29** + ffi `CoreConfig.cache_dir` |
| Settings rows | `feature:settings` |

## 7. Data and persistence

- Clips: **`cacheDir/lectrice/audio/<docId>/c<cleanerVersion>/<engineId>/<voiceId>/<sentenceId>.wav`** (the web spec §6
  layout under the new cache root, C29). Shown by Android as the app's "Cache"; cleared by its "Clear cache" and, on low
  storage, by the system. Excluded from Auto Backup. (Changed 2026-10-24 from `filesDir`, at the user's request: file 20
  §3.2 has the reasons.)
- DataStore: `readAheadMinutes` (5 | 10 | 20).

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| 10 listening-minutes ahead, ~1 hour kept, both scaled by speed | user | 2026-10-21 |
| Phone cap 1024 MB | spec §5.2 (proposed there, kept) | 2026-10-23 |
| Stop read-ahead after 60 s paused; unload the engine after 5 min idle | spec §5.4, §8.1 (proposed there, kept) | 2026-10-23 |
| ~~Clips in `filesDir`, not `cacheDir`~~ → clips in **`cacheDir`**, visible and clearable from Android's Settings | user request, Opus (file 20) | 2026-10-24 |
| The governor's modes Full / Reduced (2 min) / Off and the rules of §3.3 (pause 60 s, sleep timer, battery saver or ≤ 15 %, thermal SEVERE / CRITICAL) | Opus, from the user's request for a concrete stop rule | 2026-10-24 |
| Read-ahead setting 5 / 10 / 20 | spec §11.5 | 2026-10-23 |

Open (spike): whether some phones need Prepare (file 14).

## 9. Done when

- [ ] After the first sentence, a 30-minute listen at 1× and at 1.5× on the S26 Ultra has no buffering (spike M-i green).
- [ ] Logs show read-ahead stops 60 s after pause and the engine unloads after 5 min idle.
- [ ] While paused for 2 min with ExoPlayer still loading, no new read-ahead plan appears in the logs (`plan_ahead = false`).
- [ ] Battery saver on while playing → the plan log shows 2 min ahead; off → back to the setting. Paused in saver → Off at once.
- [ ] A thermal test hook (debug) reporting SEVERE → Reduced; CRITICAL → Off and a running export pauses; MODERATE for 2 min →
      back.
- [ ] Android's "Clear cache" during playback: playback goes on (next clips made again); `storage_usage` shows ~0 after.
- [ ] `storage_usage` matches `du` of the audio folder within 1 %; "Clear audio cache" brings it near 0 while playing.
- [ ] Changing Read ahead to 20 min doubles the planned seconds in the next plan log line.
- [ ] Switching fp32 ↔ int8 never plays a clip of the other bundle (engine id in every clip-hit log line).

## 10. Design screens

R3 (preparing), R7 (catching up), R4 (buffered slider), T1 (read ahead), T3 (Storage screen, file 20: new, needs a design
pass). The governor has no UI of its own.
