# F: frontend on mocks (done 2026-10-23)

Built from `docs/2026-10-21-frontend-brief.md` against the mock API. Fable (builder), no research.

## How to run
```bash
cd frontend
npm run dev        # http://localhost:5173/  (VITE_API_MODE=mock in .env)
npm run build      # tsc -b && vite build
npm run lint       # oxlint, zero warnings
npm test           # node --test tests/ (12 tests)
```
Mock scenarios go in the query *before* the hash: `http://localhost:5173/?mock=noText,engineFailed#/`.
Switching to the real server later is `VITE_API_MODE=http` (same `src/api/` signatures; the dev
server proxies `/api` to `localhost:7878`).

## What was built
- `frontend/` (Vite 8.3.5, React 19.3.1, TS 6.0.3, oxlint; no other dependencies): 94 files
  (config included), exactly the brief's file tree. The ~37 COPY VERBATIM files were copied as given (one fix, below).
- Library (`#/`): drop zone / "Open PDF…", page-wide drag and drop, recent documents with
  progress and "Continue", client-side PDF checks (type, size, `%PDF-` magic).
- Reader (`#/read/<id>`): option D. Large serif text, page markers in the margin (inline chips on
  phones), code/table boxes with cue labels, captions, the floating pill (prev / play-pause / next /
  status / locate / more) and the popover (slider, time left, location, speed, voice, "Read code &
  tables aloud", export with MP3/M4B, "Open another PDF…"). Sentence-by-sentence playback with
  prefetch, position saving, follow-scroll, Space / ← / → shortcuts, engine and audio error banners.
- Tests: the brief's cases plus `playerStatusText`, `exportPhase`, table+note order, `safeFileName`,
  `percentRead`, `groupVoices`/`formatVoiceOption`.
- Screenshots: `design/screens/{reader-light,reader-dark,reader-phone-375,library-light}.png`.

## Acceptance checklist (brief §11)
All items were run in headless Chrome 154 against the mock (a CDP script in the agent's scratchpad;
nothing added to the project): library (3 rows, `.txt` rejected without an API call, drop → "Reading
…" → reader, re-import, `noText`, `empty`, `offline`, "Document not found"), reader (saved position
23 highlighted, "Paused · p. 1 of 5", all markers incl. the mid-paragraph "p. 3", cue playback with
the accent border, caption skipped, switch on/off mapping cue ↔ first line, 63 ↔ 69, preparing →
playing, grey read sentences, Space/←/→, click-to-jump while playing, text selection, fast jumps,
2× speed ≈ half the time and halved time left, voice change re-prepares, follow-scroll + locate
button, popover contents, Escape/outside close, slider seeks once, position saved → 73 %,
"Finished" → restart), errors (`engineFailed`, `audioError` skip), export (MP3 → "Download MP3"
`Designing Calm Software (Heart).mp3`, M4B with 69 sentences, `exporterMissing` alert, cancel,
leave-and-return mid-export, `exportFailed` at 40 %), Long Book (opens in ≈0.7 s incl. mock latency,
2,888 blocks in the DOM, 7,939 playable), 375 px (no horizontal scroll, pill spans the width, status
hidden, popover fits), keyboard (Tab order and visible focus ring on every control), light and dark.
Contrast re-measured for every pair in use: lowest 5.47:1 (status on the pill, light).

## Not verifiable in mock mode
- "Audio requests carry `rate=` and `readSkipped=`": the mock never touches the network. The
  URL is built in the verbatim `src/api/audio.ts` and `usePlayback` passes both values via
  `buffer.setContext`; check it in the Network tab once `http` mode is wired (step W).
- React DevTools "Highlight updates": not installed in headless Chrome. `BlockView` is memoized and
  only the active block receives `activeSentenceId`, as the brief specifies.

## Deviations from the brief (and why)
- **`src/api/mock/latency.ts` (COPY VERBATIM) fixed:** `abort()` on an already-aborted signal read
  `timer` before its `const` initialisation (TDZ `ReferenceError`). `timer` is now a `let` declared
  before `abort()`. Everything else is as given.
- `MiniPlayer` takes a `popoverId` prop (the brief's `aria-controls` needs the id both in the pill
  and in `PlayerPopover`).
- `ExportPanel` takes `fileFormat` (the finished job's format) so "Download M4B" and the `.m4b` name
  are right after reopening the reader, when the select has reset to MP3. The Format select is
  always shown and disabled while starting/queued/running (checklist wording).
- `StatusBanner` has no `tone` prop: nothing in the brief uses the warning tone, so it was removed
  (rules §11).
- `useLibrary` returns `recentLoadedAt` (the fetch time) for "opened 2 hours ago", because
  `new Date()` in render fails the `react(purity)` lint.
- A caption's page marker is placed before its first sentence only (a page change inside a
  one-line caption doesn't happen in practice).
- The mock document has two level-1 headings (Preface, Chapter 1), so the reader shows two `<h1>`;
  the heading-level mapping is the brief's `Record<HeadingLevel, tag>`.

## Next
Step W: set `VITE_API_MODE=http`, run the backend on 7878, and go through §11 once more against
real audio (the mock's "tick" WAV only proves the state machine).
