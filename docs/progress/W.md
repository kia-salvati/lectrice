# W: frontend wired to the real backend (done 2026-10-23)

## How to run the app for real
```bash
# terminal 1: backend on 127.0.0.1:7878
cd backend && . "$HOME/.cargo/env" && cargo run --release -p lectrice-api
# terminal 2: frontend on http://localhost:5173/ (VITE_API_MODE=http from frontend/.env)
cd frontend && npm run dev
```
- Mock mode (no backend): `npm run dev:mock` (`vite --mode mock`, reads `frontend/.env.mock`). Mock scenarios work
  as before: `http://localhost:5173/?mock=noText,engineFailed#/`.
- The Vite proxy sends `/api` to `http://127.0.0.1:7878` (was `localhost`; the backend binds IPv4 loopback only, and
  `localhost` can resolve to `::1` first).
- Quick export tests: `LECTRICE_EXPORT_MAX_SENTENCES=150` on the backend (test servers only).

## What changed (all in `frontend/`)
- `types/document.ts`: `Sentence.speechText?: string` (`""` = nothing to say).
- `utils/playbackOrder.ts`: silent sentences (`speechText === ""`) are left out of the playback order. They are
  still shown, greyed when passed and clickable: `resolvePlayable` maps a silent id to the **next** playable sentence
  (not back to its paragraph's start), via the new `nextPositionOfSilent`. "Time left" (`charsFrom`) now counts
  `speechText ?? text`. Matches the backend read-ahead (`read_ahead_plan.rs`).
- `types/errors.ts`, `api/errors.ts`: `RequestInvalid` (1000). The message comes from the server body
  ("This request isn't valid."). `docs/progress/B-fixes.md` did not exist when I finished, so no other new names.
- `hooks/usePlayback.ts`: the "last saved" ref starts at the opening sentence. Before, just opening a book
  saved sentence 0 (`PUT position`), so a never-played book showed "Continue · 1%" instead of "Start".
- `api/mock/exports.ts`: like the server (B6), starting an export while one for the same document + voice + format
  is queued or running returns that job.
- `.env` → `VITE_API_MODE=http`, new `.env.mock`, `package.json` script `dev:mock`, `vite.config.ts` proxy target.
- `tests/utils.test.ts`: a silent-sentence case (order with switch off/on, time left uses `speechText`,
  resolve mid-paragraph, code line → cue, silent at the end → null, step skips silent).

Export contract (B6) checked: `api/exports.ts` and `useAudioExport` need no change. A duplicate start returns the
running job (202, same id) and the hook just shows it. With the engine down, `GET …/exports` gives 503
`SpeechEngineUnavailable` and the panel shows that message under the disabled "Needs the speech engine" button.

## Checks
`npm run build` OK, `npm run lint` 0 warnings (exit 0), `npm test` 13/13 pass.

## Real-backend test (headless Chrome 154, release backend, scratch data dir)
CDP scripts in the session scratchpad (`/tmp/claude-1000/-home-kia-Desktop-learning/f725dd85-ffd9-4d95-94f2-655bf09d100f/scratchpad/p*.mjs`), nothing added to the project.

| Item | Result |
|---|---|
| Import DDS through the file input | OK: 201, reader in 0.44 s; 2104 sentence spans, 932 blocks, 99 code/table boxes |
| Import LinkedIn Posts 2024 | OK: 201, reader in 1.5 s; 3694 spans, 2616 blocks |
| Page markers | OK, both books: the markers in the DOM are exactly the page changes in the document JSON (DDS 140, LinkedIn 361), labels match `pages` ("p. Cover", "p. i" … "p. 1" on PDF page 15). LinkedIn has no labels, so PDF numbers are shown |
| Play makes audio requests with `voice`, `rate`, `readSkipped` | OK: `…/sentences/3/audio?voice=af_heart&rate=1&readSkipped=false` → 200 `audio/wav` |
| Audio really plays | OK: the highlight moves on each `ended` event, 0→1→2→3→4→30→31 |
| Silent sentences skipped | OK: DDS 5–29 (ISBN/copyright, `speechText: ""`) never requested, playback jumps 4 → 30 |
| Prefetch | OK: each step requests the current sentence + the next 2 |
| 2× | OK: every later request has `rate=2`; status shows "· 2×", time left halves |
| Jump (click sentence 500) | OK: playing after 2.4 s (uncached, at 2×); 501, 502 prefetched |
| Position saved | OK: `PUT …/position` 204 after 1.5 s; server list `lastSentenceId` updated. Opening a book no longer saves |
| Continue from the library | OK: DDS row "Continue · 15%" → reader at sentence 356, scrolled into view, first request is 356 |
| Recent documents | OK: both books, newest first ("Linkedin Posts 2024 · Start", "Designing Distributed Systems · Continue · 15%") |
| Code cue | OK: clicking the code box (block 150) plays its cue 355 ("Code example.", 47 KB ≈ 1 s), box gets the active style, then 356 |
| "Read code & tables aloud" | OK: with the switch on, the same block plays its own line 354 with `readSkipped=true` |
| MP3 export: start, progress | OK: POST 202, progress bar + "71 of 150 sentences · under a minute left" |
| Duplicate start | OK: a second POST while running → 202 with the same job id |
| Cancel | OK: DELETE 204 → "Export cancelled." + "Export whole book" |
| Finished export + download | OK: done after 174 s (150 sentences); link `download="Designing Distributed Systems (Heart).mp3"`; server `Content-Type: audio/mpeg`, `Content-Disposition` same name; Chrome saved `Designing Distributed Systems (Heart).mp3` (5.7 MB) |
| Engine down (`LECTRICE_SPEECH_MODEL_DIR` bad) | OK: health `failed`, banner "Speech engine failed to load…", Play disabled, export panel disabled with the 503 message |
| Console errors | None in normal use. With the engine down: the expected 503 lines plus "Voice list failed" / "Prefetch failed" logs |

Not a problem: Chrome's network log marks every `204` (position PUT, export DELETE) as `ERR_ABORTED (canceled)`
right after the fetch resolved 204. It happens through the proxy and directly against 7878 too, and the server saved
each one. The other "canceled" entries are React StrictMode's double effects in dev, and polls aborted on purpose.

## For the backend agent
1. **Low: the export time estimate is too low early on.** At 71/150 DDS showed "under a minute left", but the
   rest took longer (the full 150 took 174 s). The first sentences were cached or silent, so chars/elapsed is too fast.
   You could leave cached and silent sentences out of the rate, or measure the rate only on synthesized ones.
2. Jumps wait behind abandoned `Now` jobs (review M1). I saw 2.4 s for one jump at 2×. Fast repeated jumps were not
   measured.
3. Already fixed on the backend side: unknown paths now return a registry JSON 404 (92 bytes) instead of an empty body.

## Frontend follow-ups (small, not done)
- While the engine is down, the reader still requests the current sentence and the 2 prefetches (three 503s).
  `usePlayback` could skip loading when `canPlay` is false.
- With the engine down, the export panel says it twice ("Needs the speech engine" and the 503 message).
- The pill shows "p. 15 of 150" (the last printed label), and the popover shows "PDF page … of 164". That's by design
  from F, noted only because the two totals differ.
