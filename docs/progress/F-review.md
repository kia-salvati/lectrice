# F-review: frontend end-of-task review (done 2026-10-23)

Reviewed every file under `frontend/src/` against `rules/CLAUDE_FRONTEND_RULES.md` (§1, §2, §5, §6, §7, §8, §10, §11),
fixed the two follow-ups from `W.md`, and added the backend's new error names. Behaviour and the API contract
(fields, endpoints, query names) are unchanged. No git, no Artifacts, nothing touched outside `frontend/` and this file.

## Checks
- `npm run build` OK (tsc -b + vite; 275 kB JS, 8.7 kB CSS), `npm run lint` 0 warnings (exit 0), `npm test` 13/13.
- Visual pass in mock mode, headless Chrome 154 (CDP scripts in the session scratchpad, nothing added to the project):
  reader light and dark at 1280 px, 375 px (no horizontal scroll, popover fits), library. All match option D.
  **Nothing visible changed, so the 4 screenshots in `design/screens/` were left as they are.**
- Contrast, computed from the live tokens: light `fg2/bg2` (pill status, help lines, captions, markers) 5.47, `fg2/bg` 6.09,
  `accent/bg2` 5.72, `on-accent/accent` 6.36, `err-fg/err-bg` 7.84, `hl-fg/hl` 13.26; dark lowest is `fg2/bg2` 7.33.
  Everything in use is ≥ 4.5:1.

## W.md follow-ups
1. **Audio requested while the engine is down** (`hooks/usePlayback.ts`). The load/prefetch effect now returns early
   when `canPlay` is false (engine checking, failed or unreachable) and re-runs when it becomes true, so a recheck
   that succeeds loads the current sentence as before. Measured in mock mode by counting `AbortController`s created
   inside `sentenceAudioBuffer`: engine down 0 (was 3), engine ready 3 (6 with StrictMode's double effects in dev).
   Play stays disabled (`PLAY_BUTTON.engineFailed`), status "Speech engine unavailable", console clean.
2. **Engine-down message shown twice in the export panel** (`hooks/useAudioExport.ts`, `pages/ReaderPage/ReaderScreen.tsx`).
   `useAudioExport` now takes `onEngineUnavailable` like `usePlayback` does and has one `fail` path for every request
   (list, start, poll, cancel): an abort is ignored, `SpeechEngineUnavailable` goes to the engine (banner + disabled
   "Needs the speech engine" button), anything else becomes `error`. The panel shows the engine message once, and a
   stale 503 from the initial `GET …/exports` no longer lingers under an enabled button after the engine recovers.

## Coordinator addition: new registry errors
`types/errors.ts` and `api/errors.ts`: added `RouteNotFound` (1006, 404), `MethodNotAllowed` (1007, 405) and
`RequestCancelled` (1008, 499) to `ApiErrorName` and `API_ERROR_NAMES`. `RequestInvalid` (1000) was already there.
`ApiError` messages come from the server body (`error.message`), so no frontend message table needed a line;
`importErrorCopy` keeps its generic fallback for names it doesn't special-case.

## Issues found → what changed, file by file
- `hooks/usePlayback.ts`
  - Effect gated on `canPlay` (follow-up 1, above).
  - §11 dead code: `pause` was exposed in the `Playback` interface but no consumer used it (only `togglePlay` inside
    the hook). Removed from the interface and the return; the internal `pause` stays.
  - §1 layering: `PlaybackStatus` now comes from `utils/playerStatus.ts` (see below) instead of being defined here.
- `utils/playerStatus.ts` (§1 dependency direction, §11 duplication). It imported `PlaybackStatus` from `hooks/` and
  declared its own `EngineStateLike`, a copy of `useSpeechEngine`'s `EngineState`. Both status types now live in
  this util (lowest layer) and the hooks import them: `usePlayback` → `PlaybackStatus`, `useSpeechEngine` → `EngineState`.
  `EngineStateLike` is gone. Public functions and the tests are unchanged.
- `hooks/useSpeechEngine.ts`: imports `EngineState` from the util; comment on `markUnavailable` now names both callers.
- `hooks/useAudioExport.ts`: `onEngineUnavailable` argument and the single `fail` path (follow-up 2, above).
- `pages/ReaderPage/ReaderScreen.tsx`
  - Passes `engine.markUnavailable` to `useAudioExport`.
  - §2 DRY: "do the jump, then resume follow-scroll" was written out five times (next, previous, seek, sentence click,
    block click). One `andFollow(jump)` wrapper now produces all five handlers; the three inline arrow props in the JSX
    became named handlers (`seek`, `selectSentence`, `selectBlock`).
  - §8 correctness: the finished export's "Ready · Heart voice" line used the *currently selected* voice, so switching
    the voice after an export mislabelled the file (the download name was already right). New `fileVoiceName`
    (the job's voice, falling back to the selected one), used for both the label and the download name.
- `components/ExportPanel/ExportPanel.tsx`: new `fileVoiceName` prop, used in the `done` phase; `voiceName` keeps
  describing the *next* export in the help line (both documented in the props).
- `pages/LibraryPage/LibraryPage.tsx` (§2 no logic in JSX): the four drag-and-drop handlers (counter, preventDefault,
  file pick) moved out of the JSX into named functions above the `return`. Same behaviour.
- `components/RecentDocumentList/RecentDocumentList.tsx` (§2): the "Start" / "Continue · 34%" choice is computed as
  `action` next to `percent`, not inside the markup.
- `types/document.ts` (§11): removed `BlockKind` and `SkippedKind`, exported but never used.
- `styles/tokens.css` (§11): removed `--warn-bg` / `--warn-fg` in both schemes; nothing referenced them since the
  banner's warning tone was dropped in F.
- `types/errors.ts`, `api/errors.ts`: the three new names (above).

## Checked and left as is (with why)
- `components/`: no API calls, no `dangerouslySetInnerHTML`, no `any`, imports on one line, `Record` maps for every
  fixed-set choice (`PLAY_BUTTON`, `STATUS_CLASS`, `PHASE_VIEW`, `FORMAT_LOCKED`, `HEADING_TAG`, `BOX_CLASS`, …),
  no mode flags; `MiniPlayer` takes the popover as a slot. Every CSS class in every module is referenced and every
  `styles.*` resolves; I checked each file pair.
- Errors (§5): no empty `catch`. Every request either sets user-visible state or logs; aborts are filtered on purpose.
- `global.css` `[hidden]` rule and the dotted sentence `:hover` underline: both are specified in the brief (§8.2, §8.4),
  so they stay.
- `SkippedBlockState` has the same four members as `SentenceState`. They describe different things (a whole box vs one
  span) and may diverge; not merged.
- `api/index.ts` bundles the mock API into the production build (~sample text). Code-splitting it would need `api` to
  become async for every hook; not worth it for a local app. Noted, not changed.
- `useAudioExport`'s poll effect depends on `job` (each poll result schedules the next poll); `jobId`/`inProgress`
  are derived from it but the lint's exhaustive-deps wants them listed. Harmless.
- Pill "p. 15 of 150" vs popover "PDF page … of 164" (W.md note 3): by design from F, unchanged.
- The play button is enabled for the short `checking` moment before `/health` answers; clicking does nothing until the
  engine is `ready`. Pre-existing, brief-conformant, and now no audio is requested in that window either.

## Not verifiable in mock mode
The engine-down measurement counts request *attempts*; the mock never reaches the network. Against the real server
the same code path produces zero `GET …/audio` calls (the attempt is what creates the request).
