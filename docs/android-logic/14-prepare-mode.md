# 14 · Prepare mode (conditional)

Status: **conditional.** Built **only if** the M1 spike puts a phone the user cares about in the **amber or red** RTF band
(user 2026-10-23: "prepare while charging" only if needed; spike plan §6). If both phones are green, this file is not built
and every "Prepare" button elsewhere is left out.

| Spike result (sustained RTF) | What is built |
|---|---|
| green ≤ 0.6 on the phones that matter | nothing from this file |
| amber 0.6–0.9 | Prepare, offered from the slow-phone tip at speeds > 1.2× (spike plan §6) |
| red > 0.9 | Prepare, offered first: also on S4 after the model download and in the player sheet |

## 1. Goal

On a phone that can't make speech as fast as the user listens, make the audio **ahead of time, while charging**, into the
normal audio cache, so later listening plays straight from it with no "Catching up…".

## 2. User flow

1. **From the slow-phone tip (R7p):** "Prepare this chapter while charging" → the chapter from the current sentence to its end
   is queued; a snackbar **new** "Lectrice will prepare this chapter while the phone charges."
2. **Red phones only:** S4 adds a line **new** "This phone makes speech slower than real time. Prepare books while charging
   for smooth listening." and the player sheet gets a row **new** `bolt` "Prepare while charging" / "Off" ⌄ with chips
   **This chapter · Next 3 chapters · As much as fits** (the cache cap limits it).
3. Plug in the charger: the job runs (notification **new** "Preparing The Long Book · 40 %"). Unplug: it pauses.
4. Listen later: prepared sentences play at once.

## 3. Rules & logic

- **Job (core C21):** `StartPrepare { document_id, voice_id, engine_id, read_skipped, from_sentence, to_sentence }`. The
  sentences in playback order in `[from, to)` are synthesized at **priority 3** (same as exports, FIFO with them) and written
  to the **audio cache** (encoded like read-ahead clips), not to a file. Already-cached ones are skipped.
- **Ranges:** "This chapter" = current sentence → the current chapter's end (file 07 §3.2). "Next 3 chapters" = to the end of
  the third chapter-level entry after the current one. "As much as fits" = to the book's end, stopping when the cache
  reaches **cap − 100 MB** (1 GB cap ≈ 5.8 h of 1× WAV).
- **Pinning:** while a prepare range exists for a document + voice + engine, `retain_around` keeps that range
  (core C21 adds `pinned: Option<Range<SentenceId>>` to `AudioRetention::retain_around`), so the 60-minute window doesn't
  delete what was just prepared. The pin is lifted when listening passes the range's end, when the user turns Prepare off,
  or after 14 days.
- **Running:** `PrepareWorker` (WorkManager, unique `prepare`), constraints **charging + battery not low**, foreground type
  `mediaProcessing` (API 35+) / `dataSync`; the same 6-hour / re-enqueue rules as the export worker (file 10 §3.7). Stops
  (pauses) when unplugged.
- **Engine:** uses the listening bundle; while listening, it yields (priority 3).
- **Progress:** sentences done / total of the range; the sheet row shows "Preparing · 40 %" or **new** "Ready up to Ch. 9".

## 4. States

| State | Shown |
|---|---|
| Waiting for the charger | row "Waits for the charger" (**new**) |
| Running | "Preparing · 40 %" + notification |
| Ready | "Ready up to Ch. 9" |
| Cache full | stops; "Ready up to Ch. 9" (no error) |
| Error | as exports: StorageFull pauses; others stop with a log |

## 5. Edge cases

- Voice or speed change: speed doesn't matter (1× clips); a **voice** change makes the prepared clips useless for listening
  (other voice); the row warns **new** "Prepared with Heart. Switch back to use it."
- A jump outside the prepared range: normal read-ahead applies there.
- **Android clears the cache** (its "Clear cache" button or low storage; the audio cache is in `cacheDir`, file 20 §3.2):
  prepared clips may go too. `prepare_status` then reports fewer sentences ready (it counts after the C29 refresh), and the
  `PrepareWorker` makes them again at the next charge. Accepted: prepared audio is still a cache; putting it in `filesDir`
  would hide it from Android's "Cache" and from the system's low-storage cleanup.
- Prepare and export of the same book with the same voice + engine: the export reuses the prepared clips.

## 6. Where it lives

`core:export` (`PrepareWorker`), `feature:reader` (row, tip button), core **C21** (job + pinning), ffi `start_prepare`,
`cancel_prepare`, `prepare_status(docId)`.

## 7. Data and persistence

The clips are ordinary cache files. The pin and the range live in the core (a small `prepare/<docId>.json` record so a
restart keeps them). DataStore: nothing new.

## 8. Decisions and open questions

| Decision | By | When |
|---|---|---|
| Only if the spike says so | user | 2026-10-23 |
| Into the cache, not a file; the window grows for that book | spec §8.5 | 2026-10-23 |
| Ranges, pinning, the 14-day pin limit, red-phone S4 line | Opus | 2026-10-24 |

## 9. Done when (only if built)

- [ ] On the slow phone at 1.5×, a prepared chapter plays with no "Catching up…".
- [ ] Unplugging pauses the job; plugging in resumes it.
- [ ] Prepared clips survive the 60-minute window while pinned.

## 10. Design screens

R7p (the button); the red-phone S4 line and the sheet row are not drawn yet (Fable draws them only if this is built).
