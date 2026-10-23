# Lectrice frontend: build brief (2026-10-21)

**For:** the Fable agent that builds the frontend. **Written by:** Opus, after research.
**Sources:** `docs/2026-10-21-design-spec.md` (§1, §4, §4a, §5, §7, §8, §12), `rules/CLAUDE_FRONTEND_RULES.md`,
`design/2026-10-06-reader-options.html` (option D), `research/2026-10-06-rust-stack.md` §1 (voice table).

This brief is meant to be complete. Do **not** research, and do not add anything it doesn't ask for. If something
here truly can't work, stop and report it rather than inventing a different design.

Code blocks marked **COPY VERBATIM** were type-checked (`tsc -b`), linted (`oxlint`) and, for `utils/`, tested
(`node --test`) in a scratch project made with the exact scaffold below. Copy them as they are. Everything else is a
precise spec you write yourself.

---

## 0. What you are building, in one paragraph

A Vite + React + TypeScript web page with two screens. **Library** (`#/`): a drop zone / "Open PDF…" button that
uploads a PDF, plus a list of recent documents with progress and "Continue". **Reader** (`#/read/<id>`): option D,
the cleaned text in large type filling the window, page markers ("p. 37") in the margin, and a small floating pill
player (previous / play-pause / next + short status + "more"). "More" opens a popover with position, time left, speed,
voice, a **"Read code & tables aloud"** switch, **Export audio** (whole book, MP3 or M4B, with progress, cancel and
download) and "Open another PDF…". Playback goes sentence by sentence and highlights the current sentence. Code,
table and caption blocks are shown but skipped: a code or table block plays its short spoken **cue** ("Code
example.") instead, unless the switch is on, in which case its own sentences play. Playback prefetches the next 2
sentences and saves the reading position. All backend calls go through `src/api/`, which has two
implementations with the same signatures: `mock` (default now) and `http`.

---

## 1. Toolchain and versions (checked 2026-10-21 with `npm view`)

| Package | Version | Notes |
|---|---|---|
| `create-vite` | 9.3.1 | scaffold tool |
| `vite` | 8.3.5 (template asks `^8.3.4`) | needs Node `^20.19.0 \|\| >=22.12.0` |
| `@vitejs/plugin-react` | 6.1.2 | peer `vite ^8` |
| `react`, `react-dom` | 19.3.1 | |
| `@types/react`, `@types/react-dom` | 19.3.0 | |
| `typescript` | **6.0.3** (template pins `~6.0.3`) | 7.0.2 (the Go port) is `latest` on npm, but keep the template's 6.0.x pin. TS 6 has `strict` on by default (checked: an implicit `any` fails the build). |
| `oxlint` | 1.89.0 | the template's linter (the template no longer uses ESLint by default) |
| `@types/node` | 24.19.2 | template dev dependency (used by `vite.config.ts`) |

This machine has Node v22.23.1 and npm 10.9.8. Both meet Vite 8's `>=22.12.0`. The scaffold was run here,
installed and built with no warnings.

**No other dependencies.** No router, no state library, no UI kit, no test runner, no CSS tooling. The reasons are in §3.

### Scaffold (run once, from `/home/kia/Desktop/learning/lectrice`)

```bash
npm create vite@9.3.1 frontend -- --template react-ts --no-interactive --no-immediate
cd frontend
npm install
```

`--no-interactive` stops the prompts. `--no-immediate` stops it from starting the dev server. It picks Oxlint, the
template's default linter.

Then:
- Delete `src/App.css`, `src/index.css`, `src/assets/` and `public/icons.svg`.
- Replace `public/favicon.svg` with:
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="16" cy="16" r="15" fill="#1d5fae"/><path d="M12 9v14l11-7z" fill="#fff"/></svg>`
- `index.html`: `<title>Lectrice</title>`, keep `lang="en"`, and add `<meta name="color-scheme" content="light dark" />`.
- Create `.env` with one line: `VITE_API_MODE=mock`.
- In `package.json` "scripts", add `"test": "node --test \"tests/**/*.test.ts\""`. Leave `dev`, `build` (`tsc -b && vite build`),
  `lint` (`oxlint`) and `preview` as they are.
- Keep `tsconfig.app.json` and `tsconfig.node.json` as generated. Note that `erasableSyntaxOnly` is on, so:
  **no `enum`, no `namespace`, no constructor parameter properties** (`constructor(private x: T)`). Use unions,
  `as const` arrays and plain fields.

### `vite.config.ts` (COPY VERBATIM)

```ts
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: { '/api': 'http://localhost:7878' },
  },
})
```

The backend serves its routes under `/api` (spec §7), so the proxy does not rewrite the path. Port 5173 matches the
backend's `allowed_origin`.

### `.oxlintrc.json` (COPY VERBATIM, replaces the generated one)

```json
{
  "$schema": "./node_modules/oxlint/configuration_schema.json",
  "plugins": ["react", "typescript", "oxc"],
  "rules": {
    "react/rules-of-hooks": "error",
    "react/exhaustive-deps": "warn",
    "react/only-export-components": ["warn", { "allowConstantExport": true }],
    "react/no-danger": "error",
    "typescript/no-explicit-any": "error"
  }
}
```

Each of these rule names was checked: they fire on a test file (`any`, `dangerouslySetInnerHTML`, a missing effect
dependency). The `react` plugin also turns on `react(set-state-in-effect)` as a warning. Treat every warning as
something to fix.

---

## 2. Commands Fable must run before reporting

```bash
cd /home/kia/Desktop/learning/lectrice/frontend
npm run build      # tsc -b && vite build; must pass with zero errors
npm run lint       # oxlint; zero warnings, zero errors
npm test           # node --test on tests/; all pass
npm run dev        # then open http://localhost:5173/  (mock mode)
```

`npm test` uses Node 22's built-in test runner and built-in TypeScript type stripping. No packages and no flags are
needed (checked on this machine). It works because `src/utils/` files import **only types** from other files.
**Rule:** a `src/utils/` file never imports a runtime value from another file. If it ever has to, the import must
use the `.ts` extension.

**Do not run git** (no `git init`, no commit). Don't install anything globally.

---

## 3. Decisions (with reasons)

1. **CSS Modules, no UI kit.** Vite supports `*.module.css` out of the box (scoped class names, no config). Global
   tokens go in plain `styles/tokens.css` and `styles/global.css`, imported once in `main.tsx`. Shared control
   styles (buttons, selects, icon buttons) live in `styles/controls.module.css` and are imported where needed.
   Note that `vite/client` types every CSS Module as `Record<string, string>`, so a misspelled `styles.foo` is
   *not* a compile error. Check by hand that every `styles.x` exists (rules §11).
2. **Routing: a tiny hash router (`useHashRoute`), no react-router.** There are only two screens and one route
   parameter. Hash URLs (`#/`, `#/read/<id>`) give a working Back button and reload, need no server fallback and
   no dependency. A plain state switch would break Back and reload.
3. **No state library.** State is per screen. The hooks own it (rules §1). Nothing is shared across screens
   except through the API.
4. **Large documents: `content-visibility: auto` per block plus `React.memo` per block, no virtualization
   library.** A real book has about 8,000 sentences (about 2,000 blocks). `content-visibility: auto` (Baseline 2024)
   lets the browser skip layout and paint for off-screen blocks, while keeping them in the DOM. So Ctrl+F,
   text selection, screen readers and `scrollIntoView` keep working. A virtual list would break all four and
   add a dependency. On the React side, each block is a memoized component that only re-renders when the active
   sentence enters or leaves it, so moving to the next sentence re-renders at most 2 blocks. Clicks use one
   delegated handler on the container instead of 8,000 handlers.
5. **One `HTMLAudioElement` for the whole reader**, created in `usePlayback` (`new Audio()`, never in the DOM).
   One element keeps a single set of listeners and the playback rate. The next 2 sentences are fetched ahead as
   Blobs and turned into object URLs. URLs that leave the window are revoked, and their requests are aborted
   (`AbortController`). Code: `sentenceAudioBuffer.ts` (COPY VERBATIM).
6. **Speed** = `audio.playbackRate`. Loading a new `src` resets `playbackRate` to `defaultPlaybackRate`, so the
   code sets both. `preservesPitch` defaults to `true` and is Baseline (widely available since Dec 2023), so
   don't set it. (TypeScript 6's DOM types don't even declare it.)
7. **Playback order is one pure function**, `buildPlaybackOrder(document, readSkipped)` in `utils/playbackOrder.ts`,
   following spec §5. Spoken blocks (`heading`, `paragraph`, `listItem`, `note`) play their sentences. A skipped block
   (`code`, `table`, `caption`; `spoken: false`) plays its `cue` if it has one and the switch is off, plays nothing
   if it has no cue (captions), and plays its own sentences (never the cue) when the switch is on. The cue is
   **never displayed**. The hook keeps the user's *requested* sentence id and derives the playable one with
   `resolvePlayable`. So flipping the switch mid-book needs no extra state: a cue maps to the block's first line
   and back, and a caption maps to the next playable block. The mock export counts sentences with the same function.
8. **The audio URL carries `rate` (current speed) and `readSkipped`** so the server can size and shape its
   read-ahead (spec §7). They don't change the audio itself, so changing speed or the switch keeps the prefetched
   audio; the buffer just sends the new values on later requests.
9. **Mock audio** is a real WAV built in JS: 16 kHz, 16-bit mono, `max(0.8 s, 0.06 s × characters)` long, with a
   soft 120 ms 440 Hz tick at the start so you can hear sentence changes, then silence. `play`, `ended` and
   `playbackRate` all really work. (The checked output for 100 characters: a 6.00 s PCM WAV.)
10. **Upload progress: none.** `fetch` can't report upload progress. The upload is to localhost, so it is near
   instant anyway. The slow part is the server cleaning the PDF, which no client can measure. Show an
   indeterminate `<progress>` with "Reading <file>…" (as in D).
11. **Auto-scroll follows the reader until the user scrolls by hand.** Then a "Scroll to current sentence" button
   appears in the pill. D has no such button; it is the smallest affordance that solves the problem.
12. **Window scrolling** (not an inner scroll box). The pill is `position: fixed`. This is simpler, works better
    on phones and makes `scrollIntoView` straightforward.
13. **Theme follows the system** (`prefers-color-scheme`), with no toggle (spec §12).
14. **Status text with page:** "p. 37 of 412", where 412 is the *printed label of the last PDF page*, so both
    numbers come from the same system. The popover shows the exact PDF page too: "Page 37 · PDF page 61 of 429".
15. **Export:** formats come from the constant `EXPORT_FORMATS = ['mp3', 'm4b']` (a `<select>` in the export panel;
    MP3 is the default). The export uses the current voice and the current "Read code & tables aloud" setting
    (`readSkipped` in the POST body). The download is a plain
    `<a href download>` to `GET /api/exports/{id}/file`, so a large file streams to disk instead of being loaded
    into memory. The export job lives on the server. On open, the reader loads `GET /api/documents/{id}/exports`
    (newest first) and shows the newest job, so a running or finished export survives closing the reader.
16. **The document type is named `CleanDocument`,** not `Document`, so it never shadows the DOM's global `Document`
    type.

### Contract details confirmed in the spec (§5, §7)

- `GET /api/documents/{id}` returns the §5 document **plus `lastSentenceId: number | null`** (type `OpenedDocument`).
  The reader starts there ("Continue").
- `GET /api/documents/{id}/exports` returns `{ items: AudioExport[] }`, newest first.
- `Voice.language` is `"en-US" | "en-GB"` (typed as a union, so the label maps are exhaustive). `Voice.name` is
  the short name (`"Heart"`). `Voice.gender` is `"female" | "male"`.
- Timestamps (`addedAt`, `lastOpenedAt`) are ISO 8601 strings in UTC (`"2026-10-21T18:04:00Z"`).
- `pages` has one entry per PDF page. A page with no entry shows its PDF number.
- `listItem` blocks are shown as bullet paragraphs.
- **Every block has `sentences`**, skipped ones included (spec §5: they "still get split into sentences and stored").
  Skipped blocks (`code`, `table`, `caption`) also have `text` (for display; it keeps line breaks) and an optional
  `cue`. A `note` block is spoken and has no `text`.
- The cue's id is a normal sentence id from the same sequence, and it is valid in the audio route and as a saved
  position.
- `PdfNoText` is code 1005 (the §8 registry).

---

## 4. File tree (every file you create; one job each)

```
frontend/
  .env                          VITE_API_MODE=mock
  .oxlintrc.json                lint rules (§1)
  index.html                    title, color-scheme meta
  vite.config.ts                React plugin, port 5173, /api proxy
  public/favicon.svg            icon (§1)
  tests/
    utils.test.ts               node:test assertions for src/utils (COPY VERBATIM, then extend: §10)
  src/
    main.tsx                    createRoot + StrictMode, imports styles/tokens.css + styles/global.css
    App.tsx                     reads the hash route, renders LibraryPage or ReaderPage; blocks stray file drops from navigating away
    env.d.ts                    types VITE_API_MODE (COPY VERBATIM)
    config/
      env.ts                    API_MODE (validated), API_BASE_PATH (COPY VERBATIM)
      constants.ts              upload limit, speeds, prefetch size, timings, export formats (COPY VERBATIM)
      routes.ts                 Route type, parseRoute(hash), routeHref(route)
    types/
      document.ts               CleanDocument, Block union, Sentence, PageLabel, DocumentSummary, OpenedDocument (COPY VERBATIM)
      voice.ts                  Voice (COPY VERBATIM)
      api.ts                    ListResponse, PageRequest, VoiceList, ImportResult, Health, AudioExport, ApiErrorBody (COPY VERBATIM)
      errors.ts                 ApiErrorName, ClientErrorName, AppErrorName (COPY VERBATIM)
    api/
      contract.ts               LectriceApi interface: the one signature both implementations share (COPY VERBATIM)
      client.ts                 the only fetch; error mapping (COPY VERBATIM)
      errors.ts                 ApiError, ClientError, helpers (COPY VERBATIM)
      health.ts                 GET /health (COPY VERBATIM)
      voices.ts                 GET /voices (COPY VERBATIM)
      documents.ts              list, import, get, save position (COPY VERBATIM)
      audio.ts                  GET sentence audio (COPY VERBATIM)
      exports.ts                start, list for a document, get, cancel, file URL (COPY VERBATIM)
      index.ts                  picks http or mock from VITE_API_MODE; exports `api` (COPY VERBATIM)
      mock/
        index.ts                mockApi: implements LectriceApi in memory with latency (COPY VERBATIM)
        scenario.ts             ?mock=… toggles (COPY VERBATIM)
        latency.ts              abortable delay + jitter (COPY VERBATIM)
        wav.ts                  playable fake WAV (COPY VERBATIM)
        voices.ts               28 English Kokoro voices (COPY VERBATIM)
        sampleText.ts           the sample book's text, compact form (COPY VERBATIM)
        buildDocument.ts        compact source → spec §5 shape; page labels; chapter repeat (COPY VERBATIM)
        library.ts              in-memory documents + summaries; import (COPY VERBATIM)
        exports.ts              clock-driven fake export jobs, kept in module state (COPY VERBATIM)
    utils/
      playbackOrder.ts          what plays and in what order (cue / skip / read-aloud rule), stepping, resolving, prefetch window, seconds left (COPY VERBATIM)
      pages.ts                  page label lookup, marker plan, "p. 37" formatting (COPY VERBATIM)
      time.ts                   "12 min left", "2 hours ago" (COPY VERBATIM)
      voices.ts                 group by language, "Heart (US, female)" (COPY VERBATIM)
      pdfFile.ts                client-side PDF checks (COPY VERBATIM)
      progress.ts               percent read (COPY VERBATIM)
      playerStatus.ts           pill status text + play-button state from playback/engine state (you write; §7)
      exportPhase.ts            one phase from AudioExport + isStarting (you write; §7)
      fileName.ts               safe download file name (you write; §7)
    hooks/
      useHashRoute.ts           current route + navigate (§6)
      useLibrary.ts             recent documents, pagination, import (§6)
      useDocument.ts            load one OpenedDocument (§6)
      useVoices.ts              voice list + selected voice (§6)
      useSpeechEngine.ts        health check, retry, mark failed (§6)
      usePlayback.ts            the player use case (COPY VERBATIM)
      sentenceAudioBuffer.ts    prefetch + object URL lifetime, used only by usePlayback (COPY VERBATIM)
      useAudioExport.ts         start / poll / cancel a whole-book export (COPY VERBATIM)
      useFollowScroll.ts        auto-scroll to the active sentence; pause on manual scroll (§6)
      useReaderShortcuts.ts     Space, ←, → (§6)
    pages/
      LibraryPage/LibraryPage.tsx + LibraryPage.module.css    header, DropZone, RecentDocumentList; page-wide drag and drop
      ReaderPage/ReaderPage.tsx + ReaderPage.module.css       loads document, voices and engine in parallel; loading/error states; document.title
      ReaderPage/ReaderScreen.tsx                              a loaded document: wires usePlayback/useAudioExport/useFollowScroll to the components
    components/
      ReadingView/ReadingView.tsx + .module.css    the article; one delegated click handler; renders BlockView per block
      ReadingView/BlockView.tsx                    memoized: one block (heading / paragraph / listItem / code)
      Sentence/Sentence.tsx + .module.css          one sentence span with its state class
      PageMarker/PageMarker.tsx + .module.css      "p. 37", title "PDF page 61"
      SkippedBlockView/SkippedBlockView.tsx + .module.css  code / table box (monospace, label) and caption (small text); shows reading state
      MiniPlayer/MiniPlayer.tsx + .module.css      the pill; owns popover open/close (view state)
      PlayerPopover/PlayerPopover.tsx + .module.css  position, time left, speed, voice, export slot, open another
      ProgressSlider/ProgressSlider.tsx + .module.css  native range; commits on release
      SpeedPicker/SpeedPicker.tsx                  select of PLAYBACK_SPEEDS
      VoicePicker/VoicePicker.tsx                  select with optgroups
      ReadSkippedSwitch/ReadSkippedSwitch.tsx      the "Read code & tables aloud" checkbox switch
      ExportPanel/ExportPanel.tsx + .module.css    format select, export states and actions
      DropZone/DropZone.tsx + .module.css          idle / importing / failed (D's empty state)
      DropZone/importErrorCopy.ts                  Record of error titles and bodies
      RecentDocumentList/RecentDocumentList.tsx + .module.css
      StatusBanner/StatusBanner.tsx + .module.css  error/warning strip with action buttons
      icons/icons.tsx                              inline SVG icons (paths from D)
    styles/
      tokens.css                colour + font tokens, light and dark (§8.1)
      global.css                reset, body, focus ring (§8.2)
      controls.module.css       button, primary button, icon button, play button, select (§8.3)
```

If a file isn't in this list, don't create it. You may drop a planned file if it turns out not to be needed;
say so in your report.

---

## 5. Types, API and mock (COPY VERBATIM)

### 5.1 `src/env.d.ts`

```ts
interface ImportMetaEnv {
  readonly VITE_API_MODE: string | undefined;
}
```

### 5.2 `src/config/env.ts`

```ts
export type ApiMode = 'mock' | 'http';

const API_MODES: readonly ApiMode[] = ['mock', 'http'];

const parseApiMode = (value: string | undefined): ApiMode => {
  const mode = (value ?? '').trim().toLowerCase();
  const match = API_MODES.find((candidate) => candidate === mode);
  if (!match) throw new Error(`VITE_API_MODE must be "mock" or "http", got "${value ?? ''}"`);
  return match;
};

export const API_MODE: ApiMode = parseApiMode(import.meta.env.VITE_API_MODE);

/** Same origin; the Vite dev server proxies /api to the backend. */
export const API_BASE_PATH = '/api';
```

### 5.3 `src/config/constants.ts`

```ts
import type { ExportFormat } from '../types/api';

/** Mirrors the backend's upload.max_mb. The server re-checks. */
export const MAX_UPLOAD_MB = 200;
export const MAX_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024;

export const PLAYBACK_SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2] as const;
export type PlaybackSpeed = (typeof PLAYBACK_SPEEDS)[number];
export const DEFAULT_SPEED: PlaybackSpeed = 1;

/** Sentences fetched ahead of the one playing. */
export const PREFETCH_AHEAD = 2;

/** Measured Kokoro rate at 1× (spec §4: ≈ 18 characters per second). Used only for "time left". */
export const SECONDS_PER_CHAR = 1 / 18;

/** Used until GET /api/voices answers, or if it fails. The server's defaultVoiceId wins once known. */
export const FALLBACK_VOICE_ID = 'af_heart';

export const POSITION_SAVE_DELAY_MS = 1500;

export const RECENT_PAGE_SIZE = 20;

/** Formats offered for whole-book export, in menu order. The first is the default. */
export const EXPORT_FORMATS: readonly ExportFormat[] = ['mp3', 'm4b'];

export const EXPORT_FORMAT_LABEL: Record<ExportFormat, string> = { mp3: 'MP3', m4b: 'M4B (audiobook)' };

/** File extension per format, for the download name. */
export const EXPORT_FORMAT_EXTENSION: Record<ExportFormat, string> = { mp3: 'mp3', m4b: 'm4b' };

export const EXPORT_POLL_INTERVAL_MS = 1000;
```

### 5.4 `src/types/document.ts`

```ts
/** SHA-256 of the PDF bytes, hex. */
export type DocumentId = string;

/** Maps one PDF page (1-based) to the label printed on it ("xii", "37"). */
export interface PageLabel { index: number; label: string; }

export interface Sentence { id: number; text: string; pageIndex: number; }

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

interface BlockBase { id: number; pageIndex: number; sentences: Sentence[]; }

export interface HeadingBlock extends BlockBase { kind: 'heading'; level: HeadingLevel; }
export interface ParagraphBlock extends BlockBase { kind: 'paragraph'; }
export interface ListItemBlock extends BlockBase { kind: 'listItem'; }
export interface NoteBlock extends BlockBase { kind: 'note'; }

/**
 * Shown, but skipped by playback unless "Read code & tables aloud" is on.
 * `text` keeps the original layout (line breaks) for display; `sentences` are what gets read when the switch is on.
 * `cue` (code and tables only) is a short spoken stand-in ("Code example."), never displayed.
 */
export interface SkippedBlock extends BlockBase { kind: 'code' | 'table' | 'caption'; spoken: false; text: string; cue?: Sentence; }

export type SpokenBlock = HeadingBlock | ParagraphBlock | ListItemBlock | NoteBlock;
export type Block = SpokenBlock | SkippedBlock;
export type BlockKind = Block['kind'];
export type SkippedKind = SkippedBlock['kind'];

/** The saved, cleaned document (spec §5). Named CleanDocument so it never shadows the DOM's global `Document` type. */
export interface CleanDocument {
  id: DocumentId;
  title: string;
  sourceFileName: string;
  pageCount: number;
  cleanerVersion: number;
  pages: PageLabel[];
  blocks: Block[];
}

/** GET /api/documents/{id}: the §5 document plus where the reader stopped (spec §7), used for "Continue". */
export interface OpenedDocument extends CleanDocument { lastSentenceId: number | null; }

export interface DocumentSummary {
  id: DocumentId;
  title: string;
  sourceFileName: string;
  pageCount: number;
  sentenceCount: number;
  /** ISO 8601 timestamps in UTC: "2026-10-21T18:04:00Z". */
  addedAt: string;
  lastOpenedAt: string;
  lastSentenceId: number | null;
}
```

### 5.5 `src/types/voice.ts`

```ts
export type VoiceGender = 'female' | 'male';

export type VoiceLanguage = 'en-US' | 'en-GB';

export interface Voice {
  id: string;
  /** Short display name: "Heart". */
  name: string;
  language: VoiceLanguage;
  gender: VoiceGender;
}
```

### 5.6 `src/types/api.ts`

```ts
import type { DocumentSummary } from './document';
import type { Voice } from './voice';

export interface ListResponse<T> { items: T[]; total: number; limit: number; offset: number; }

export interface PageRequest { limit: number; offset: number; }

export interface VoiceList { voices: Voice[]; defaultVoiceId: string; }

export interface ImportResult { document: DocumentSummary; alreadyInLibrary: boolean; }

export type SpeechEngineState = 'ready' | 'failed';

export interface Health { status: 'ok'; speechEngine: SpeechEngineState; }

export interface ApiErrorBody { error: { name: string; code: number; message: string; }; }

export type ExportStatus = 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

export interface AudioExport {
  id: string;
  documentId: string;
  voiceId: string;
  format: ExportFormat;
  status: ExportStatus;
  sentencesDone: number;
  sentencesTotal: number;
  estimatedSecondsLeft: number | null;
}

export interface AudioExportResponse { export: AudioExport; }

/** GET /api/documents/{id}/exports, newest first. */
export interface AudioExportList { items: AudioExport[]; }

export type ExportFormat = 'mp3' | 'm4b';

export interface StartExportRequest { voiceId: string; format: ExportFormat; readSkipped: boolean; }
```

### 5.7 `src/types/errors.ts`

```ts
/** Server error names (spec §8). */
export type ApiErrorName =
  | 'UploadMissingFile' | 'UploadTooLarge' | 'PdfInvalid' | 'PdfEncrypted' | 'PdfNoText'
  | 'DocumentNotFound' | 'SentenceNotFound'
  | 'VoiceUnknown' | 'SpeechEngineUnavailable' | 'SpeechSynthesisFailed'
  | 'ExportNotFound' | 'ExportNotReady' | 'ExportFormatUnsupported' | 'ExporterUnavailable'
  | 'StorageFailed' | 'Internal';

/** Errors raised in the browser, before or instead of a server answer. */
export type ClientErrorName = 'NetworkError' | 'UnexpectedResponse' | 'FileNotPdf' | 'FileEmpty' | 'FileTooLarge' | 'AudioPlaybackFailed';

export type AppErrorName = ApiErrorName | ClientErrorName;
```

### 5.8 `src/api/errors.ts`

**Error-mapping rule:** a non-2xx response becomes `ApiError(name, code, message, status)` from the body
`{ error: { name, code, message } }`. If the body is missing or unknown, it becomes `ApiError('Internal', 9999, …)`.
A fetch that never reaches the server becomes `ClientError('NetworkError')`. An `AbortError` is re-thrown untouched
so callers can ignore it. Show `error.message` to the user: the server's messages are already user-safe (spec §8),
and the client ones are written for users.

```ts
import type { ApiErrorName, AppErrorName, ClientErrorName } from '../types/errors';

const API_ERROR_NAMES: readonly ApiErrorName[] = ['UploadMissingFile', 'UploadTooLarge', 'PdfInvalid', 'PdfEncrypted', 'PdfNoText', 'DocumentNotFound', 'SentenceNotFound', 'VoiceUnknown', 'SpeechEngineUnavailable', 'SpeechSynthesisFailed', 'ExportNotFound', 'ExportNotReady', 'ExportFormatUnsupported', 'ExporterUnavailable', 'StorageFailed', 'Internal'];

const CLIENT_ERROR_MESSAGES: Record<ClientErrorName, string> = {
  NetworkError: "Can't reach the Lectrice server. Is it running?",
  UnexpectedResponse: 'The server sent an answer Lectrice did not understand.',
  FileNotPdf: 'This file is not a PDF.',
  FileEmpty: 'This file is empty.',
  FileTooLarge: 'This file is too large.',
  AudioPlaybackFailed: "This sentence's audio could not be played.",
};

/** A non-2xx answer from the backend, carrying the registry name, code and user-facing message. */
export class ApiError extends Error {
  override readonly name: ApiErrorName;
  readonly code: number;
  readonly status: number;

  constructor(name: ApiErrorName, code: number, message: string, status: number) {
    super(message);
    this.name = name;
    this.code = code;
    this.status = status;
  }
}

/** A failure detected in the browser (no server answer, bad file, playback failure). */
export class ClientError extends Error {
  override readonly name: ClientErrorName;

  constructor(name: ClientErrorName, options?: { cause?: unknown }) {
    super(CLIENT_ERROR_MESSAGES[name], options);
    this.name = name;
  }
}

export type AppError = ApiError | ClientError;

export const isApiErrorName = (value: string): value is ApiErrorName => (API_ERROR_NAMES as readonly string[]).includes(value);

export const isAbortError = (error: unknown): boolean => error instanceof DOMException && error.name === 'AbortError';

export const hasErrorName = (error: unknown, name: AppErrorName): boolean => (error instanceof ApiError || error instanceof ClientError) && error.name === name;

/** Anything thrown that is not already an AppError becomes UnexpectedResponse, keeping the original as `cause`. */
export const toAppError = (error: unknown): AppError => (error instanceof ApiError || error instanceof ClientError) ? error : new ClientError('UnexpectedResponse', { cause: error });
```

### 5.9 `src/api/client.ts`

```ts
import { API_BASE_PATH } from '../config/env';
import type { ApiErrorBody } from '../types/api';
import { ApiError, ClientError, isAbortError, isApiErrorName } from './errors';

const isApiErrorBody = (value: unknown): value is ApiErrorBody => {
  if (typeof value !== 'object' || value === null || !('error' in value)) return false;
  const inner: unknown = value.error;
  return typeof inner === 'object' && inner !== null && 'name' in inner && 'code' in inner && 'message' in inner
    && typeof inner.name === 'string' && typeof inner.code === 'number' && typeof inner.message === 'string';
};

const toApiError = async (response: Response): Promise<ApiError> => {
  const body: unknown = await response.json().catch(() => null);
  if (isApiErrorBody(body) && isApiErrorName(body.error.name)) {
    return new ApiError(body.error.name, body.error.code, body.error.message, response.status);
  }
  return new ApiError('Internal', 9999, 'Something went wrong.', response.status);
};

/** The only place that calls fetch. Throws ApiError (non-2xx), ClientError('NetworkError'), or re-throws AbortError untouched. */
const send = async (path: string, init: RequestInit = {}): Promise<Response> => {
  let response: Response;
  try {
    response = await fetch(`${API_BASE_PATH}${path}`, init);
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw new ClientError('NetworkError', { cause: error });
  }
  if (!response.ok) throw await toApiError(response);
  return response;
};

export const requestJson = async <T>(path: string, init?: RequestInit): Promise<T> => {
  const response = await send(path, init);
  try {
    return (await response.json()) as T;
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw new ClientError('UnexpectedResponse', { cause: error });
  }
};

export const requestBlob = async (path: string, init?: RequestInit): Promise<Blob> => (await send(path, init)).blob();

export const requestVoid = async (path: string, init?: RequestInit): Promise<void> => {
  await send(path, init);
};
```

### 5.10 `src/api/contract.ts`

```ts
import type { AudioExport, Health, ImportResult, ListResponse, PageRequest, StartExportRequest, VoiceList } from '../types/api';
import type { DocumentId, DocumentSummary, OpenedDocument } from '../types/document';

export interface SentenceAudioRequest {
  documentId: DocumentId;
  sentenceId: number;
  voiceId: string;
  /** The listener's speed. Audio is always made at 1×; the server only uses this to size its read-ahead. */
  rate: number;
  /** Whether read-ahead should include code, table and caption sentences. */
  readSkipped: boolean;
}

/** Both implementations (http and mock) satisfy this. Hooks only see this interface. */
export interface LectriceApi {
  getHealth(signal?: AbortSignal): Promise<Health>;
  listVoices(signal?: AbortSignal): Promise<VoiceList>;
  listDocuments(page: PageRequest, signal?: AbortSignal): Promise<ListResponse<DocumentSummary>>;
  importPdf(file: File, signal?: AbortSignal): Promise<ImportResult>;
  getDocument(id: DocumentId, signal?: AbortSignal): Promise<OpenedDocument>;
  saveReadingPosition(id: DocumentId, sentenceId: number): Promise<void>;
  /** Resolves to audio/wav bytes made at 1× speed. */
  getSentenceAudio(request: SentenceAudioRequest, signal?: AbortSignal): Promise<Blob>;
  startExport(documentId: DocumentId, request: StartExportRequest): Promise<AudioExport>;
  /** This document's exports, newest first, so a running or finished export survives closing the reader. */
  listExports(documentId: DocumentId, signal?: AbortSignal): Promise<AudioExport[]>;
  getExport(exportId: string, signal?: AbortSignal): Promise<AudioExport>;
  cancelExport(exportId: string): Promise<void>;
  /** A plain URL for an <a download> link, so a large file streams to disk instead of into memory. */
  exportFileUrl(exportId: string): string;
}
```

### 5.11 HTTP modules: `health.ts`, `voices.ts`, `documents.ts`, `audio.ts`, `exports.ts`

```ts
// src/api/health.ts
import type { Health } from '../types/api';
import { requestJson } from './client';

export const getHealth = (signal?: AbortSignal): Promise<Health> => requestJson<Health>('/health', { signal });
```

```ts
// src/api/voices.ts
import type { VoiceList } from '../types/api';
import { requestJson } from './client';

export const listVoices = (signal?: AbortSignal): Promise<VoiceList> => requestJson<VoiceList>('/voices', { signal });
```

```ts
// src/api/documents.ts
import type { ImportResult, ListResponse, PageRequest } from '../types/api';
import type { DocumentId, DocumentSummary, OpenedDocument } from '../types/document';
import { requestJson, requestVoid } from './client';

export const listDocuments = (page: PageRequest, signal?: AbortSignal): Promise<ListResponse<DocumentSummary>> => {
  const query = new URLSearchParams({ limit: String(page.limit), offset: String(page.offset) });
  return requestJson<ListResponse<DocumentSummary>>(`/documents?${query}`, { signal });
};

export const importPdf = (file: File, signal?: AbortSignal): Promise<ImportResult> => {
  const body = new FormData();
  body.append('file', file);
  return requestJson<ImportResult>('/documents', { method: 'POST', body, signal });
};

export const getDocument = (id: DocumentId, signal?: AbortSignal): Promise<OpenedDocument> => requestJson<OpenedDocument>(`/documents/${encodeURIComponent(id)}`, { signal });

export const saveReadingPosition = (id: DocumentId, sentenceId: number): Promise<void> => requestVoid(`/documents/${encodeURIComponent(id)}/position`, {
  method: 'PUT',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ sentenceId }),
});
```

```ts
// src/api/audio.ts
import type { SentenceAudioRequest } from './contract';
import { requestBlob } from './client';

export const getSentenceAudio = ({ documentId, sentenceId, voiceId, rate, readSkipped }: SentenceAudioRequest, signal?: AbortSignal): Promise<Blob> => {
  const query = new URLSearchParams({ voice: voiceId, rate: String(rate), readSkipped: String(readSkipped) });
  return requestBlob(`/documents/${encodeURIComponent(documentId)}/sentences/${sentenceId}/audio?${query}`, { signal });
};
```

```ts
// src/api/exports.ts
import { API_BASE_PATH } from '../config/env';
import type { AudioExport, AudioExportList, AudioExportResponse, StartExportRequest } from '../types/api';
import type { DocumentId } from '../types/document';
import { requestJson, requestVoid } from './client';

export const startExport = async (documentId: DocumentId, request: StartExportRequest): Promise<AudioExport> => {
  const response = await requestJson<AudioExportResponse>(`/documents/${encodeURIComponent(documentId)}/exports`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  return response.export;
};

export const listExports = async (documentId: DocumentId, signal?: AbortSignal): Promise<AudioExport[]> => (await requestJson<AudioExportList>(`/documents/${encodeURIComponent(documentId)}/exports`, { signal })).items;

export const getExport = async (exportId: string, signal?: AbortSignal): Promise<AudioExport> => (await requestJson<AudioExportResponse>(`/exports/${encodeURIComponent(exportId)}`, { signal })).export;

export const cancelExport = (exportId: string): Promise<void> => requestVoid(`/exports/${encodeURIComponent(exportId)}`, { method: 'DELETE' });

export const exportFileUrl = (exportId: string): string => `${API_BASE_PATH}/exports/${encodeURIComponent(exportId)}/file`;
```

### 5.12 `src/api/index.ts`

```ts
import { API_MODE } from '../config/env';
import type { ApiMode } from '../config/env';
import { getSentenceAudio } from './audio';
import type { LectriceApi } from './contract';
import { getDocument, importPdf, listDocuments, saveReadingPosition } from './documents';
import { cancelExport, exportFileUrl, getExport, listExports, startExport } from './exports';
import { getHealth } from './health';
import { mockApi } from './mock';
import { listVoices } from './voices';

const httpApi: LectriceApi = { getHealth, listVoices, listDocuments, importPdf, getDocument, saveReadingPosition, getSentenceAudio, startExport, listExports, getExport, cancelExport, exportFileUrl };

const API_BY_MODE: Record<ApiMode, LectriceApi> = { http: httpApi, mock: mockApi };

export const api: LectriceApi = API_BY_MODE[API_MODE];
```

Hooks import `api` from `../api`, and the error classes and helpers from `../api/errors`. Nothing outside
`src/api/` calls `fetch`.

### 5.13 Mock: what it contains

- **"Designing Calm Software"** (`mock-calm`): 18 PDF pages. Text sits on PDF pages 12–17, labelled
  **xii, xiii, 1, 2, 3, 4** (front matter is roman, the body restarts at 1). It has 70 sentences in all (ids
  0–69), including:
  - 6 headings (levels 1, 2, 3), three `listItem` blocks and one `note`;
  - one **`code` block with the cue "Code example."** (id 41, PDF page 16), whose 6 lines are its own sentences
    (ids 42–47);
  - one **`caption` with no cue** (id 48);
  - **a page change mid-paragraph** (PDF page 15 → 16 after the 3rd sentence of that paragraph);
  - one long sentence (more than 200 characters) and one very short one.

  With the switch off, 63 sentences play (the cue instead of the code, no caption). With it on, 69 play (the code
  lines and the caption, no cue). Saved position: sentence 23 (chapter 1, printed page 1), so the list shows 34 %.
- **"The Long Book (about 8,000 sentences)"** (`mock-big`): the same preface, then the chapter repeated 180 times.
  That gives 7,939 playable sentences (switch off) on 734 PDF pages. Use it for the performance checks.
- **"Quarterly Notes"** (`mock-missing`): listed in recent documents but not stored, so opening it gives
  `DocumentNotFound`.
- **Import:** any PDF you drop becomes a new document with the sample content, titled from the file name.
  Dropping the same file name and size again returns `alreadyInLibrary: true`.
- **Voices:** the 28 English Kokoro voices, ids 0–27 from the research table (af_* and am_* are `en-US`; bf_* and
  bm_* are `en-GB`). The default is `af_heart`.
- **Audio:** the first request for a sentence and voice takes 0.5–1.2 s ("preparing audio" is visible); repeats
  take about 20–50 ms (like the server cache).
- **Reading position** saves into the in-memory summary, so the Library shows the new progress when you go back.
- **Export:** accepts `mp3` and `m4b`. Jobs live in module state, so they survive leaving and reopening the
  reader (not a page reload), and `listExports` returns them newest first. A fake job is `queued` for 1 s, then `running`, and `done` after 20 s. Progress is computed from
  the clock. Cancel works. The "file" is a 3-second WAV object URL. It plays when downloaded, even though the name
  ends in `.mp3`. That is fine for a mock.
- **Scenarios** in the page URL, comma-separated, *before* the hash: `http://localhost:5173/?mock=noText,engineFailed#/`

| `?mock=` | Effect |
|---|---|
| `noText` | every import fails with `PdfNoText` (422) |
| `engineFailed` | `/health` says `speechEngine: "failed"`; every audio request fails with `SpeechEngineUnavailable` (503) |
| `offline` | every call fails with `ClientError('NetworkError')` |
| `empty` | recent documents list is empty |
| `audioError` | audio for sentence id 5 fails with `SpeechSynthesisFailed` (500) |
| `exportFailed` | an export fails at 40 % |
| `exporterMissing` | starting an export fails with `ExporterUnavailable` (3004, 503) |

State is in memory and resets on reload. That's intended.

### 5.14 Mock files

```ts
// src/api/mock/scenario.ts
/** Toggles read once from the page URL, e.g. http://localhost:5173/?mock=noText,engineFailed#/ */
export type MockScenario = 'noText' | 'engineFailed' | 'offline' | 'empty' | 'audioError' | 'exportFailed' | 'exporterMissing';

const SCENARIOS: readonly MockScenario[] = ['noText', 'engineFailed', 'offline', 'empty', 'audioError', 'exportFailed', 'exporterMissing'];

const parseScenarios = (search: string): ReadonlySet<MockScenario> => {
  const requested = (new URLSearchParams(search).get('mock') ?? '').split(',').map((part) => part.trim());
  return new Set(SCENARIOS.filter((scenario) => requested.includes(scenario)));
};

const active = parseScenarios(window.location.search);

export const isScenarioActive = (scenario: MockScenario): boolean => active.has(scenario);
```

```ts
// src/api/mock/latency.ts
/** Waits like a real request would, and rejects with AbortError if the signal fires first. */
export const delay = (ms: number, signal?: AbortSignal): Promise<void> => new Promise((resolve, reject) => {
  const abort = () => {
    clearTimeout(timer);
    reject(new DOMException('Aborted', 'AbortError'));
  };
  if (signal?.aborted) {
    abort();
    return;
  }
  const timer = setTimeout(() => {
    signal?.removeEventListener('abort', abort);
    resolve();
  }, ms);
  signal?.addEventListener('abort', abort, { once: true });
});

export const jitter = (baseMs: number, spreadMs: number): number => baseMs + Math.round(Math.random() * spreadMs);
```

```ts
// src/api/mock/wav.ts
const SAMPLE_RATE = 16_000;
const SECONDS_PER_CHAR = 0.06;
const MIN_SECONDS = 0.8;
const TICK_SECONDS = 0.12;
const TICK_HZ = 440;
const TICK_AMPLITUDE = 0.08;
const HEADER_BYTES = 44;

/** Length of the fake audio for a sentence, at 1× speed. */
export const mockSpeechSeconds = (text: string): number => Math.max(MIN_SECONDS, text.length * SECONDS_PER_CHAR);

/** A playable 16-bit mono PCM WAV: a short soft tick, then silence, lasting `seconds`. */
export const makeMockWav = (seconds: number): Blob => {
  const sampleCount = Math.round(seconds * SAMPLE_RATE);
  const dataBytes = sampleCount * 2;
  const view = new DataView(new ArrayBuffer(HEADER_BYTES + dataBytes));
  const writeAscii = (offset: number, text: string) => {
    for (let i = 0; i < text.length; i += 1) view.setUint8(offset + i, text.charCodeAt(i));
  };
  writeAscii(0, 'RIFF');
  view.setUint32(4, 36 + dataBytes, true);
  writeAscii(8, 'WAVE');
  writeAscii(12, 'fmt ');
  view.setUint32(16, 16, true);              // fmt chunk size
  view.setUint16(20, 1, true);               // PCM
  view.setUint16(22, 1, true);               // mono
  view.setUint32(24, SAMPLE_RATE, true);
  view.setUint32(28, SAMPLE_RATE * 2, true); // byte rate
  view.setUint16(32, 2, true);               // block align
  view.setUint16(34, 16, true);              // bits per sample
  writeAscii(36, 'data');
  view.setUint32(40, dataBytes, true);
  const tickSamples = Math.min(sampleCount, Math.round(TICK_SECONDS * SAMPLE_RATE));
  for (let i = 0; i < tickSamples; i += 1) {
    const fade = 1 - i / tickSamples;
    const value = Math.sin((2 * Math.PI * TICK_HZ * i) / SAMPLE_RATE) * TICK_AMPLITUDE * fade;
    view.setInt16(HEADER_BYTES + i * 2, Math.round(value * 0x7fff), true);
  }
  return new Blob([view.buffer], { type: 'audio/wav' });
};
```

```ts
// src/api/mock/voices.ts
import type { VoiceList } from '../../types/api';
import type { Voice } from '../../types/voice';

const voice = (id: string, name: string): Voice => ({
  id,
  name,
  language: id.startsWith('b') ? 'en-GB' : 'en-US',
  gender: id[1] === 'f' ? 'female' : 'male',
});

/** Kokoro English voices, speaker ids 0–27, in id order. */
export const MOCK_VOICES: VoiceList = {
  defaultVoiceId: 'af_heart',
  voices: [
    voice('af_alloy', 'Alloy'), voice('af_aoede', 'Aoede'), voice('af_bella', 'Bella'), voice('af_heart', 'Heart'),
    voice('af_jessica', 'Jessica'), voice('af_kore', 'Kore'), voice('af_nicole', 'Nicole'), voice('af_nova', 'Nova'),
    voice('af_river', 'River'), voice('af_sarah', 'Sarah'), voice('af_sky', 'Sky'),
    voice('am_adam', 'Adam'), voice('am_echo', 'Echo'), voice('am_eric', 'Eric'), voice('am_fenrir', 'Fenrir'),
    voice('am_liam', 'Liam'), voice('am_michael', 'Michael'), voice('am_onyx', 'Onyx'), voice('am_puck', 'Puck'), voice('am_santa', 'Santa'),
    voice('bf_alice', 'Alice'), voice('bf_emma', 'Emma'), voice('bf_isabella', 'Isabella'), voice('bf_lily', 'Lily'),
    voice('bm_daniel', 'Daniel'), voice('bm_fable', 'Fable'), voice('bm_george', 'George'), voice('bm_lewis', 'Lewis'),
  ],
};
```

```ts
// src/api/mock/sampleText.ts
import type { HeadingLevel } from '../../types/document';

/**
 * Compact source for mock documents. `page` is the PDF page (1-based). `nextPageFrom: k` puts sentences k.. on page + 1.
 * A code block's sentences are its non-empty lines; `cue` is what playback says instead when skipping it.
 */
export type SourceBlock =
  | { kind: 'heading'; level: HeadingLevel; page: number; text: string }
  | { kind: 'paragraph' | 'listItem' | 'note'; page: number; sentences: string[]; nextPageFrom?: number }
  | { kind: 'code'; page: number; text: string; cue?: string }
  | { kind: 'caption'; page: number; sentences: string[] };

/** PDF pages 12–13 carry roman labels (xii, xiii); page 14 is printed "1". */
export const FIRST_ARABIC_PAGE = 14;

export const PREFACE: SourceBlock[] = [
  { kind: 'heading', level: 1, page: 12, text: 'Preface' },
  { kind: 'paragraph', page: 12, sentences: [
    'This book is about software that stays out of the way.',
    'Most of us have used tools that shout: banners that never close, badges that never clear, and settings hidden three menus deep.',
    'Calm software does the opposite.',
    'It shows what matters now, keeps everything else one step away, and never makes the reader wonder what just happened.',
    'That sounds simple, and in a sense it is.',
  ] },
  { kind: 'paragraph', page: 12, sentences: [
    'The chapters that follow are short on purpose.',
    'Each one takes a single idea, shows where it came from, and ends with something you can try on your own project the same afternoon.',
    'You do not need to read them in order.',
    'If a chapter does not fit the work in front of you, skip it and come back later.',
  ] },
  { kind: 'paragraph', page: 13, sentences: [
    'A few words about the examples.',
    'They are written in TypeScript because it is widely read, but nothing in them depends on a particular language or framework.',
    'Where an example needs a server, it assumes a small local one, the kind you might run on a laptop while you work.',
    'Three habits come up again and again:',
  ] },
  { kind: 'listItem', page: 13, sentences: ['Show one thing at a time, and make it obvious which one it is.'] },
  { kind: 'listItem', page: 13, sentences: ['Say what a control will do before it is pressed.'] },
  { kind: 'listItem', page: 13, sentences: ['When something goes wrong, say what the person can do next.'] },
  { kind: 'paragraph', page: 13, sentences: [
    'None of these are new.',
    'What is new, perhaps, is treating them as engineering rules rather than matters of taste.',
  ] },
];

export const CHAPTER: SourceBlock[] = [
  { kind: 'heading', level: 1, page: 14, text: '1. Quiet Interfaces' },
  { kind: 'paragraph', page: 14, sentences: [
    'An interface is quiet when nothing on the screen competes for attention without a reason.',
    'That does not mean empty.',
    'A quiet screen can be dense with information, as long as the information is arranged so that the eye knows where to start.',
    'The difference between a busy screen and a rich one is mostly a matter of hierarchy: one clear primary element, a few secondary ones, and everything else visibly less important.',
    'Hierarchy is cheap to add and expensive to remove, so it pays to decide it early.',
  ] },
  { kind: 'heading', level: 2, page: 14, text: '1.1 Fewer, clearer signals' },
  { kind: 'paragraph', page: 14, sentences: [
    'Every colour, icon and badge is a signal, and signals wear out.',
    'A red dot that appears for every minor update soon means nothing, and then it fails on the day it carries real news.',
    'The fix is to spend signals deliberately.',
    'Give each one a single meaning, write that meaning down, and remove any signal that has drifted from it.',
    'Teams that do this often find they can delete half of their notifications without anyone noticing, except to say that the product feels easier.',
  ] },
  { kind: 'paragraph', page: 15, nextPageFrom: 3, sentences: [
    'Signals also need to be readable.',
    'Grey text on a grey background may look elegant in a mock-up, but it fails the moment the screen is in sunlight or the reader is tired.',
    'A useful rule is to measure contrast rather than judge it by eye, and to treat any text below a ratio of four and a half to one as a bug.',
    'The same applies to size.',
    'Text that must be read should be large enough to read at arm’s length on the smallest device you support, which for most products today is a phone held in one hand on a moving train.',
    'Labels that are only decoration can be smaller, but there are fewer of those than designers tend to think.',
  ] },
  { kind: 'heading', level: 2, page: 16, text: '1.2 A small example' },
  { kind: 'paragraph', page: 16, sentences: [
    'Consider a status line that reports the state of a background job.',
    'A first version often maps each state to a colour with a chain of conditions.',
    'A calmer version keeps the mapping in one table, so that adding a state is one line and nothing can be forgotten:',
  ] },
  { kind: 'code', page: 16, text: "const STATUS_LABEL: Record<JobStatus, string> = {\n  queued: 'Waiting to start',\n  running: 'Working…',\n  failed: 'Stopped. Try again?',\n  done: 'Finished',\n};", cue: 'Code example.' },
  { kind: 'caption', page: 16, sentences: ['Listing 1.1. A status table that keeps every message in one place.'] },
  { kind: 'paragraph', page: 16, sentences: [
    'The table does more than tidy the code.',
    'It puts every message the user can see in one place, where a reviewer can read them side by side and notice that one of them is rude, vague or wrong.',
    'It also makes the compiler check that no state is missing.',
    'That is the whole idea.',
  ] },
  { kind: 'heading', level: 2, page: 17, text: '1.3 Measuring calm' },
  { kind: 'paragraph', page: 17, sentences: [
    'Calm is hard to measure directly, but its absence leaves traces.',
    'Support requests that begin with “I can’t find” or “what does this mean” point at hierarchy and wording.',
    'Repeated clicks on the same control point at missing feedback.',
    'Long pauses before a destructive action point at a label that does not say what will happen.',
    'None of these need special tools; a week of reading support mail is usually enough to find the worst of them.',
  ] },
  { kind: 'note', page: 17, sentences: ['Note: none of these measures replace talking to the people who use the product.'] },
  { kind: 'paragraph', page: 17, sentences: [
    'Once you have a list, fix the cheapest items first.',
    'A clearer label costs minutes, and a removed notification costs nothing at all.',
    'Larger changes, such as reorganising a screen, are easier to argue for once the small fixes have shown that calm is something users notice.',
    'Keep the list somewhere the whole team can see it.',
  ] },
  { kind: 'heading', level: 3, page: 17, text: 'What to try next' },
  { kind: 'paragraph', page: 17, sentences: [
    'Pick one screen you know well.',
    'Write down every signal on it, and next to each one, the single thing it is supposed to mean.',
    'Cross out the ones you cannot explain in a sentence.',
    'Then show the crossed-out list to a colleague and ask which of them they would miss.',
    'You may be surprised by how short their answer is.',
  ] },
];
```

```ts
// src/api/mock/buildDocument.ts
import type { Block, OpenedDocument, PageLabel, Sentence } from '../../types/document';
import type { SourceBlock } from './sampleText';

interface DocumentMeta { id: string; title: string; sourceFileName: string; pageCount: number; firstArabicPage: number; lastSentenceId: number | null; }

const ROMAN: ReadonlyArray<[number, string]> = [[1000, 'm'], [900, 'cm'], [500, 'd'], [400, 'cd'], [100, 'c'], [90, 'xc'], [50, 'l'], [40, 'xl'], [10, 'x'], [9, 'ix'], [5, 'v'], [4, 'iv'], [1, 'i']];

const toRoman = (value: number): string => {
  let rest = value;
  let result = '';
  for (const [amount, letters] of ROMAN) {
    while (rest >= amount) {
      result += letters;
      rest -= amount;
    }
  }
  return result;
};

/** Front matter gets roman labels, the body restarts at "1": PDF page 14 → "1". */
const buildPageLabels = (pageCount: number, firstArabicPage: number): PageLabel[] => Array.from({ length: pageCount }, (_, i) => {
  const index = i + 1;
  return { index, label: index < firstArabicPage ? toRoman(index) : String(index - firstArabicPage + 1) };
});

/** Turns compact source blocks into the spec §5 shape with global, sequential block and sentence ids. */
export const buildDocument = (meta: DocumentMeta, source: readonly SourceBlock[]): OpenedDocument => {
  let nextSentenceId = 0;
  const sentence = (text: string, pageIndex: number): Sentence => ({ id: nextSentenceId++, text, pageIndex });
  const blocks: Block[] = source.map((item, blockId): Block => {
    if (item.kind === 'code') {
      const cue = item.cue === undefined ? undefined : sentence(item.cue, item.page);
      const sentences = item.text.split('\n').map((line) => line.trim()).filter(Boolean).map((line) => sentence(line, item.page));
      return { id: blockId, kind: 'code', spoken: false, pageIndex: item.page, text: item.text, sentences, ...(cue ? { cue } : {}) };
    }
    if (item.kind === 'caption') {
      const sentences = item.sentences.map((text) => sentence(text, item.page));
      return { id: blockId, kind: 'caption', spoken: false, pageIndex: item.page, text: item.sentences.join(' '), sentences };
    }
    if (item.kind === 'heading') return { id: blockId, kind: 'heading', level: item.level, pageIndex: item.page, sentences: [sentence(item.text, item.page)] };
    const sentences = item.sentences.map((text, i) => sentence(text, item.nextPageFrom !== undefined && i >= item.nextPageFrom ? item.page + 1 : item.page));
    return { id: blockId, kind: item.kind, pageIndex: item.page, sentences };
  });
  return {
    id: meta.id,
    title: meta.title,
    sourceFileName: meta.sourceFileName,
    pageCount: meta.pageCount,
    cleanerVersion: 1,
    pages: buildPageLabels(meta.pageCount, meta.firstArabicPage),
    blocks,
    lastSentenceId: meta.lastSentenceId,
  };
};

/** Shifts a chapter by whole pages and renames its first heading, to build a long document out of one chapter. */
export const repeatChapter = (chapter: readonly SourceBlock[], times: number, pagesPerChapter: number): SourceBlock[] => Array.from({ length: times }, (_, round) => chapter.map((item, i): SourceBlock => {
  const page = item.page + round * pagesPerChapter;
  if (i === 0 && item.kind === 'heading') return { ...item, page, text: `${round + 1}. ${item.text.replace(/^\d+\.\s*/, '')}` };
  return { ...item, page };
})).flat();
```

```ts
// src/api/mock/library.ts
import type { DocumentSummary, OpenedDocument, Sentence } from '../../types/document';
import { buildDocument, repeatChapter } from './buildDocument';
import { CHAPTER, FIRST_ARABIC_PAGE, PREFACE } from './sampleText';

const CHAPTER_PAGES = 4;
const BIG_REPEATS = 180;
const HOUR_MS = 3_600_000;
const DAY_MS = 24 * HOUR_MS;

const hoursAgo = (hours: number): string => new Date(Date.now() - hours * HOUR_MS).toISOString();

/** Every stored sentence, including skipped blocks' sentences and cues (ids are sequential over all of them). */
const allSentences = (document: OpenedDocument): Sentence[] => document.blocks.flatMap((block) => ('cue' in block && block.cue ? [block.cue, ...block.sentences] : block.sentences));

const countSentences = (document: OpenedDocument): number => allSentences(document).length;

const summarize = (document: OpenedDocument, addedAt: string, lastOpenedAt: string): DocumentSummary => ({
  id: document.id,
  title: document.title,
  sourceFileName: document.sourceFileName,
  pageCount: document.pageCount,
  sentenceCount: countSentences(document),
  addedAt,
  lastOpenedAt,
  lastSentenceId: document.lastSentenceId,
});

const calm = buildDocument({ id: 'mock-calm', title: 'Designing Calm Software', sourceFileName: 'designing-calm-software.pdf', pageCount: 18, firstArabicPage: FIRST_ARABIC_PAGE, lastSentenceId: 23 }, [...PREFACE, ...CHAPTER]);

const big = buildDocument({ id: 'mock-big', title: 'The Long Book (about 8,000 sentences)', sourceFileName: 'the-long-book.pdf', pageCount: FIRST_ARABIC_PAGE + BIG_REPEATS * CHAPTER_PAGES, firstArabicPage: FIRST_ARABIC_PAGE, lastSentenceId: null }, [...PREFACE, ...repeatChapter(CHAPTER, BIG_REPEATS, CHAPTER_PAGES)]);

/** In-memory library. Lost on reload, which is fine for a mock. */
export const documents = new Map<string, OpenedDocument>([[calm.id, calm], [big.id, big]]);

export const summaries: DocumentSummary[] = [
  summarize(calm, new Date(Date.now() - 12 * DAY_MS).toISOString(), hoursAgo(2)),
  summarize(big, new Date(Date.now() - 3 * DAY_MS).toISOString(), hoursAgo(26)),
  // Listed but not stored: opening it exercises the DocumentNotFound path.
  { id: 'mock-missing', title: 'Quarterly Notes', sourceFileName: 'quarterly-notes.pdf', pageCount: 42, sentenceCount: 900, addedAt: new Date(Date.now() - 40 * DAY_MS).toISOString(), lastOpenedAt: new Date(Date.now() - 9 * DAY_MS).toISOString(), lastSentenceId: 310 },
];

/** A newly "imported" file gets the sample content under its own id and title. */
export const addImported = (file: File): { summary: DocumentSummary; alreadyInLibrary: boolean } => {
  const id = `mock-import-${file.name}-${file.size}`;
  const now = new Date().toISOString();
  const existing = summaries.find((summary) => summary.id === id);
  if (existing) {
    existing.lastOpenedAt = now;
    return { summary: existing, alreadyInLibrary: true };
  }
  const title = file.name.replace(/\.pdf$/i, '').replace(/[-_]+/g, ' ').trim() || 'Untitled';
  const document: OpenedDocument = { ...calm, id, title, sourceFileName: file.name, lastSentenceId: null };
  documents.set(id, document);
  const summary = summarize(document, now, now);
  summaries.unshift(summary);
  return { summary, alreadyInLibrary: false };
};

export const findSentenceText = (documentId: string, sentenceId: number): string | null => {
  const document = documents.get(documentId);
  if (!document) return null;
  return allSentences(document).find((sentence) => sentence.id === sentenceId)?.text ?? null;
};
```

```ts
// src/api/mock/exports.ts
import type { AudioExport, ExportFormat, StartExportRequest } from '../../types/api';
import type { CleanDocument } from '../../types/document';
import { buildPlaybackOrder } from '../../utils/playbackOrder';
import { ApiError } from '../errors';
import { documents } from './library';
import { isScenarioActive } from './scenario';
import { makeMockWav } from './wav';

const EXPORT_DURATION_MS = 20_000;
const QUEUED_MS = 1_000;
const FAIL_AT_SHARE = 0.4;
const MOCK_FILE_SECONDS = 3;
const SUPPORTED_FORMATS: readonly ExportFormat[] = ['mp3', 'm4b'];

interface MockJob { info: AudioExport; startedAt: number; }

const jobs = new Map<string, MockJob>();
let nextJobNumber = 1;
let fileUrl: string | null = null;

/** Same rule as playback, from its single owner. */
const countExportSentences = (document: CleanDocument, readSkipped: boolean): number => buildPlaybackOrder(document, readSkipped).sentenceIds.length;

const notFound = () => new ApiError('ExportNotFound', 3001, 'Export not found.', 404);

/** Module state: kept across reader visits until the page reloads, like the server keeps its jobs. */

/** Progress is computed from the clock, so polling sees it advance over ~20 s. */
const snapshot = (job: MockJob): AudioExport => {
  if (job.info.status === 'cancelled' || job.info.status === 'failed' || job.info.status === 'done') return { ...job.info };
  const elapsed = Date.now() - job.startedAt;
  if (elapsed < QUEUED_MS) return { ...job.info, status: 'queued', estimatedSecondsLeft: Math.round(EXPORT_DURATION_MS / 1000) };
  const share = Math.min(1, (elapsed - QUEUED_MS) / (EXPORT_DURATION_MS - QUEUED_MS));
  const sentencesDone = Math.floor(share * job.info.sentencesTotal);
  if (isScenarioActive('exportFailed') && share >= FAIL_AT_SHARE) job.info = { ...job.info, status: 'failed', sentencesDone, estimatedSecondsLeft: null };
  else if (share >= 1) job.info = { ...job.info, status: 'done', sentencesDone: job.info.sentencesTotal, estimatedSecondsLeft: 0 };
  else job.info = { ...job.info, status: 'running', sentencesDone, estimatedSecondsLeft: Math.ceil((1 - share) * (EXPORT_DURATION_MS - QUEUED_MS) / 1000) };
  return { ...job.info };
};

export const startMockExport = (documentId: string, request: StartExportRequest): AudioExport => {
  const document = documents.get(documentId);
  if (!document) throw new ApiError('DocumentNotFound', 1101, 'Document not found.', 404);
  if (isScenarioActive('exporterMissing')) throw new ApiError('ExporterUnavailable', 3004, "Audio export isn't available (ffmpeg is missing).", 503);
  if (!SUPPORTED_FORMATS.includes(request.format)) throw new ApiError('ExportFormatUnsupported', 3003, "That audio format isn't supported.", 400);
  const sentencesTotal = countExportSentences(document, request.readSkipped);
  const id = `mock-export-${nextJobNumber++}`;
  const job: MockJob = { startedAt: Date.now(), info: { id, documentId, voiceId: request.voiceId, format: request.format, status: 'queued', sentencesDone: 0, sentencesTotal, estimatedSecondsLeft: null } };
  jobs.set(id, job);
  return snapshot(job);
};

/** Newest first. Job numbers grow, so creation order is enough. */
export const listMockExports = (documentId: string): AudioExport[] => [...jobs.values()].filter((job) => job.info.documentId === documentId).reverse().map(snapshot);

export const getMockExport = (exportId: string): AudioExport => {
  const job = jobs.get(exportId);
  if (!job) throw notFound();
  return snapshot(job);
};

export const cancelMockExport = (exportId: string): void => {
  const job = jobs.get(exportId);
  if (!job) throw notFound();
  const current = snapshot(job);
  if (current.status === 'queued' || current.status === 'running') job.info = { ...current, status: 'cancelled', estimatedSecondsLeft: null };
};

/** The mock "file" is a short WAV, created once. Fine for testing the download link. */
export const mockExportFileUrl = (): string => {
  fileUrl ??= URL.createObjectURL(makeMockWav(MOCK_FILE_SECONDS));
  return fileUrl;
};
```

```ts
// src/api/mock/index.ts
import type { ImportResult, ListResponse, PageRequest } from '../../types/api';
import type { DocumentSummary, OpenedDocument } from '../../types/document';
import type { LectriceApi, SentenceAudioRequest } from '../contract';
import { ApiError, ClientError } from '../errors';
import { cancelMockExport, getMockExport, listMockExports, mockExportFileUrl, startMockExport } from './exports';
import { delay, jitter } from './latency';
import { addImported, documents, findSentenceText, summaries } from './library';
import { isScenarioActive } from './scenario';
import { MOCK_VOICES } from './voices';
import { makeMockWav, mockSpeechSeconds } from './wav';

const AUDIO_ERROR_SENTENCE_ID = 5;

/** Same message, code and HTTP status the backend registry uses (spec §8). */
const ERRORS = {
  PdfNoText: () => new ApiError('PdfNoText', 1005, 'This PDF has no selectable text.', 422),
  DocumentNotFound: () => new ApiError('DocumentNotFound', 1101, 'Document not found.', 404),
  SentenceNotFound: () => new ApiError('SentenceNotFound', 1102, 'Sentence not found.', 404),
  VoiceUnknown: () => new ApiError('VoiceUnknown', 2001, 'Unknown voice.', 400),
  SpeechEngineUnavailable: () => new ApiError('SpeechEngineUnavailable', 2002, "The speech engine isn't available.", 503),
  SpeechSynthesisFailed: () => new ApiError('SpeechSynthesisFailed', 2003, 'Speech generation failed.', 500),
};

/** Audio already "generated" once comes back fast, like the backend's cache. */
const generatedAudio = new Set<string>();

const respond = async (ms: number, signal?: AbortSignal): Promise<void> => {
  await delay(ms, signal);
  if (isScenarioActive('offline')) throw new ClientError('NetworkError');
};

export const mockApi: LectriceApi = {
  async getHealth(signal) {
    await respond(jitter(80, 60), signal);
    return { status: 'ok', speechEngine: isScenarioActive('engineFailed') ? 'failed' : 'ready' };
  },

  async listVoices(signal) {
    await respond(jitter(120, 80), signal);
    return MOCK_VOICES;
  },

  async listDocuments(page: PageRequest, signal): Promise<ListResponse<DocumentSummary>> {
    await respond(jitter(200, 150), signal);
    const all = isScenarioActive('empty') ? [] : [...summaries].sort((a, b) => b.lastOpenedAt.localeCompare(a.lastOpenedAt));
    return { items: all.slice(page.offset, page.offset + page.limit), total: all.length, limit: page.limit, offset: page.offset };
  },

  async importPdf(file: File, signal): Promise<ImportResult> {
    await respond(jitter(1600, 800), signal);
    if (isScenarioActive('noText')) throw ERRORS.PdfNoText();
    const { summary, alreadyInLibrary } = addImported(file);
    return { document: { ...summary }, alreadyInLibrary };
  },

  async getDocument(id, signal): Promise<OpenedDocument> {
    await respond(jitter(300, 200), signal);
    const document = documents.get(id);
    if (!document) throw ERRORS.DocumentNotFound();
    const summary = summaries.find((item) => item.id === id);
    if (summary) summary.lastOpenedAt = new Date().toISOString();
    return { ...document, lastSentenceId: summary?.lastSentenceId ?? document.lastSentenceId };
  },

  async saveReadingPosition(id, sentenceId) {
    await respond(jitter(60, 60));
    const summary = summaries.find((item) => item.id === id);
    if (!summary) throw ERRORS.DocumentNotFound();
    summary.lastSentenceId = sentenceId;
  },

  async getSentenceAudio({ documentId, sentenceId, voiceId }: SentenceAudioRequest, signal) {
    const key = `${documentId}/${voiceId}/${sentenceId}`;
    await respond(generatedAudio.has(key) ? jitter(20, 30) : jitter(500, 700), signal);
    if (isScenarioActive('engineFailed')) throw ERRORS.SpeechEngineUnavailable();
    if (isScenarioActive('audioError') && sentenceId === AUDIO_ERROR_SENTENCE_ID) throw ERRORS.SpeechSynthesisFailed();
    if (!MOCK_VOICES.voices.some((voice) => voice.id === voiceId)) throw ERRORS.VoiceUnknown();
    const text = findSentenceText(documentId, sentenceId);
    if (text === null) throw ERRORS.SentenceNotFound();
    generatedAudio.add(key);
    return makeMockWav(mockSpeechSeconds(text));
  },
  async startExport(documentId, request) {
    await respond(jitter(150, 100));
    return startMockExport(documentId, request);
  },

  async listExports(documentId, signal) {
    await respond(jitter(80, 60), signal);
    if (!documents.has(documentId)) throw ERRORS.DocumentNotFound();
    return listMockExports(documentId);
  },

  async getExport(exportId, signal) {
    await respond(jitter(80, 60), signal);
    return getMockExport(exportId);
  },

  async cancelExport(exportId) {
    await respond(jitter(80, 60));
    cancelMockExport(exportId);
  },

  exportFileUrl: () => mockExportFileUrl(),
};
```

### 5.15 Utils (COPY VERBATIM)

```ts
// src/utils/playbackOrder.ts
import type { Block, CleanDocument, Sentence } from '../types/document';

/**
 * What playback reads, in order, for one setting of "Read code & tables aloud". Built once per (document, readSkipped).
 * Rule for a skipped block (code, table, caption): readSkipped off → its cue if it has one, else nothing;
 * readSkipped on → its own sentences (never the cue). Every other block → its sentences.
 */
export interface PlaybackOrder {
  sentenceIds: number[];
  /** Sentence id → position in sentenceIds. */
  positionOf: Map<number, number>;
  /** Block index (position in document.blocks) of each position in sentenceIds. */
  blockIndexAt: number[];
  /** charsFrom[i] = characters from sentenceIds[i] to the end. Has one extra trailing 0. */
  charsFrom: number[];
  /** Sentence id → PDF page, for every sentence in the order. */
  pageIndexOf: Map<number, number>;
  /** Block index of every sentence in the document, cues included, so any id can be placed. */
  blockIndexOfAny: Map<number, number>;
}

const playableSentences = (block: Block, readSkipped: boolean): Sentence[] => {
  if (!('spoken' in block)) return block.sentences;
  if (readSkipped) return block.sentences;
  return block.cue ? [block.cue] : [];
};

export const buildPlaybackOrder = (document: CleanDocument, readSkipped: boolean): PlaybackOrder => {
  const sentenceIds: number[] = [];
  const blockIndexAt: number[] = [];
  const lengths: number[] = [];
  const pageIndexOf = new Map<number, number>();
  const blockIndexOfAny = new Map<number, number>();
  document.blocks.forEach((block, blockIndex) => {
    for (const sentence of block.sentences) blockIndexOfAny.set(sentence.id, blockIndex);
    if ('cue' in block && block.cue) blockIndexOfAny.set(block.cue.id, blockIndex);
    for (const sentence of playableSentences(block, readSkipped)) {
      sentenceIds.push(sentence.id);
      blockIndexAt.push(blockIndex);
      lengths.push(sentence.text.length);
      pageIndexOf.set(sentence.id, sentence.pageIndex);
    }
  });
  const charsFrom = new Array<number>(lengths.length + 1).fill(0);
  for (let i = lengths.length - 1; i >= 0; i -= 1) charsFrom[i] = charsFrom[i + 1] + lengths[i];
  const positionOf = new Map(sentenceIds.map((id, position) => [id, position]));
  return { sentenceIds, positionOf, blockIndexAt, charsFrom, pageIndexOf, blockIndexOfAny };
};

/** First playable sentence in this block or any later one, or null past the end. */
export const firstPlayableFromBlock = (order: PlaybackOrder, blockIndex: number): number | null => {
  const position = order.blockIndexAt.findIndex((at) => at >= blockIndex);
  return position === -1 ? null : order.sentenceIds[position];
};

/**
 * Maps any sentence id onto this order: itself if playable, otherwise the first playable sentence from its block on.
 * Used for saved positions, clicks on skipped text, and when the readSkipped switch changes mid-book.
 */
export const resolvePlayable = (order: PlaybackOrder, sentenceId: number): number | null => {
  if (order.positionOf.has(sentenceId)) return sentenceId;
  const blockIndex = order.blockIndexOfAny.get(sentenceId);
  return blockIndex === undefined ? null : firstPlayableFromBlock(order, blockIndex);
};

/** Where to start: the saved sentence (resolved), otherwise the first playable one. Null if nothing is playable. */
export const startingSentenceId = (order: PlaybackOrder, lastSentenceId: number | null): number | null => {
  const resolved = lastSentenceId === null ? null : resolvePlayable(order, lastSentenceId);
  return resolved ?? order.sentenceIds[0] ?? null;
};

/** The playable sentence `offset` steps away (negative = back), or null past either end. */
export const stepFrom = (order: PlaybackOrder, sentenceId: number, offset: number): number | null => {
  const position = order.positionOf.get(sentenceId);
  if (position === undefined) return null;
  return order.sentenceIds[position + offset] ?? null;
};

/** The current sentence and the next `ahead` playable ones. */
export const prefetchWindow = (order: PlaybackOrder, sentenceId: number, ahead: number): number[] => {
  const position = order.positionOf.get(sentenceId);
  if (position === undefined) return [];
  return order.sentenceIds.slice(position, position + ahead + 1);
};

export const secondsLeft = (order: PlaybackOrder, sentenceId: number, speed: number, secondsPerChar: number): number => {
  const position = order.positionOf.get(sentenceId) ?? 0;
  return (order.charsFrom[position] * secondsPerChar) / speed;
};
```

```ts
// src/utils/pages.ts
import type { CleanDocument, PageLabel } from '../types/document';

/** PDF page index → printed label. A page without a label falls back to its PDF number. */
export const buildPageLabelLookup = (pages: readonly PageLabel[]): Map<number, string> => new Map(pages.map((page) => [page.index, page.label]));

export const printedLabel = (labels: ReadonlyMap<number, string>, pageIndex: number): string => labels.get(pageIndex) ?? String(pageIndex);

/** "p. 37" */
export const formatPageMarker = (label: string): string => `p. ${label}`;

/** "PDF page 61" */
export const formatPdfPage = (pageIndex: number): string => `PDF page ${pageIndex}`;

/**
 * Where page markers go. Code and table blocks are drawn as one box, so their marker goes before the block;
 * everything else is drawn sentence by sentence, so the marker goes before the sentence where the page changes.
 * The first content always gets one.
 */
export interface PageMarkerPlan { beforeSentence: Set<number>; beforeBlock: Set<number>; }

export const planPageMarkers = (document: CleanDocument): PageMarkerPlan => {
  const plan: PageMarkerPlan = { beforeSentence: new Set(), beforeBlock: new Set() };
  let previousPage: number | null = null;
  for (const block of document.blocks) {
    if (block.kind === 'code' || block.kind === 'table') {
      if (block.pageIndex !== previousPage) plan.beforeBlock.add(block.id);
      previousPage = block.pageIndex;
      continue;
    }
    for (const sentence of block.sentences) {
      if (sentence.pageIndex !== previousPage) plan.beforeSentence.add(sentence.id);
      previousPage = sentence.pageIndex;
    }
  }
  return plan;
};
```

```ts
// src/utils/time.ts
const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;

/** "under a minute left", "12 min left", "3 h 05 min left" */
export const formatTimeLeft = (seconds: number): string => {
  const minutes = Math.ceil(seconds / SECONDS_PER_MINUTE);
  if (minutes <= 1) return 'under a minute left';
  if (minutes < MINUTES_PER_HOUR) return `${minutes} min left`;
  const hours = Math.floor(minutes / MINUTES_PER_HOUR);
  const rest = String(minutes % MINUTES_PER_HOUR).padStart(2, '0');
  return `${hours} h ${rest} min left`;
};

const UNITS: ReadonlyArray<[Intl.RelativeTimeFormatUnit, number]> = [['year', 31_536_000], ['month', 2_592_000], ['week', 604_800], ['day', 86_400], ['hour', 3_600], ['minute', 60]];

const relative = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

/** "just now", "2 hours ago", "yesterday", "last week". `now` is passed in so the function stays pure. */
export const formatRelativeTime = (isoDate: string, now: Date): string => {
  const elapsedSeconds = (now.getTime() - new Date(isoDate).getTime()) / 1000;
  for (const [unit, size] of UNITS) {
    if (elapsedSeconds >= size) return relative.format(-Math.floor(elapsedSeconds / size), unit);
  }
  return 'just now';
};
```

```ts
// src/utils/voices.ts
import type { Voice, VoiceGender, VoiceLanguage } from '../types/voice';

export interface VoiceGroup { label: string; voices: Voice[]; }

const LANGUAGE_LABEL: Record<VoiceLanguage, string> = { 'en-US': 'American English', 'en-GB': 'British English' };
const REGION_SHORT: Record<VoiceLanguage, string> = { 'en-US': 'US', 'en-GB': 'UK' };
const GENDER_LABEL: Record<VoiceGender, string> = { female: 'female', male: 'male' };

/** Groups voices by language, keeping the server's order inside each group and the order languages first appear. */
export const groupVoices = (voices: readonly Voice[]): VoiceGroup[] => {
  const groups = new Map<VoiceLanguage, Voice[]>();
  for (const voice of voices) groups.set(voice.language, [...(groups.get(voice.language) ?? []), voice]);
  return [...groups].map(([language, members]) => ({ label: LANGUAGE_LABEL[language], voices: members }));
};

/** "Heart (US, female)" */
export const formatVoiceOption = (voice: Voice): string => `${voice.name} (${REGION_SHORT[voice.language]}, ${GENDER_LABEL[voice.gender]})`;
```

```ts
// src/utils/pdfFile.ts
import type { ClientErrorName } from '../types/errors';

export interface FileFacts { name: string; type: string; size: number; }

const PDF_MIME = 'application/pdf';
const PDF_EXTENSION = /\.pdf$/i;

/** First checks that need no reading. Some systems report an empty MIME type, so the extension also counts. */
export const checkPdfFile = (file: FileFacts, maxBytes: number): ClientErrorName | null => {
  const looksLikePdf = file.type === PDF_MIME || (file.type === '' && PDF_EXTENSION.test(file.name.trim()));
  if (!looksLikePdf) return 'FileNotPdf';
  if (file.size === 0) return 'FileEmpty';
  if (file.size > maxBytes) return 'FileTooLarge';
  return null;
};

/** Every PDF starts with "%PDF-". */
export const PDF_MAGIC = '%PDF-';
```

```ts
// src/utils/progress.ts
/** Share of a document read, 0–100, from the saved sentence id (ids are sequential from 0). Null if never started. */
export const percentRead = (lastSentenceId: number | null, sentenceCount: number): number | null => {
  if (lastSentenceId === null || sentenceCount <= 0) return null;
  return Math.min(100, Math.round(((lastSentenceId + 1) / sentenceCount) * 100));
};
```

### 5.16 `tests/utils.test.ts` (COPY VERBATIM, then add the cases in §10)

```ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatTimeLeft, formatRelativeTime } from '../src/utils/time.ts';
import { buildPlaybackOrder, firstPlayableFromBlock, prefetchWindow, resolvePlayable, startingSentenceId, stepFrom } from '../src/utils/playbackOrder.ts';
import { planPageMarkers } from '../src/utils/pages.ts';
import { checkPdfFile } from '../src/utils/pdfFile.ts';
import type { CleanDocument } from '../src/types/document.ts';

// Ids: heading 0; code cue 1 + code lines 2, 3; paragraph 4, 5; caption 6 (no cue); paragraph 7.
const doc: CleanDocument = { id: 'd', title: 't', sourceFileName: 'f.pdf', pageCount: 3, cleanerVersion: 1, pages: [], blocks: [
  { id: 0, kind: 'heading', level: 1, pageIndex: 1, sentences: [{ id: 0, text: 'Hi', pageIndex: 1 }] },
  { id: 1, kind: 'code', spoken: false, pageIndex: 2, text: 'a\nb', cue: { id: 1, text: 'Code example.', pageIndex: 2 }, sentences: [{ id: 2, text: 'a', pageIndex: 2 }, { id: 3, text: 'b', pageIndex: 2 }] },
  { id: 2, kind: 'paragraph', pageIndex: 2, sentences: [{ id: 4, text: 'abc', pageIndex: 2 }, { id: 5, text: 'de', pageIndex: 3 }] },
  { id: 3, kind: 'caption', spoken: false, pageIndex: 3, text: 'Fig.', sentences: [{ id: 6, text: 'Fig.', pageIndex: 3 }] },
  { id: 4, kind: 'paragraph', pageIndex: 3, sentences: [{ id: 7, text: 'End.', pageIndex: 3 }] },
] };

test('time left', () => {
  assert.equal(formatTimeLeft(30), 'under a minute left');
  assert.equal(formatTimeLeft(12 * 60), '12 min left');
  assert.equal(formatTimeLeft(185 * 60), '3 h 05 min left');
  assert.equal(formatRelativeTime('2026-10-20T10:00:00Z', new Date('2026-10-21T11:00:00Z')), 'yesterday');
});

test('readSkipped off: cue instead of code, caption skipped silently', () => {
  const order = buildPlaybackOrder(doc, false);
  assert.deepEqual(order.sentenceIds, [0, 1, 4, 5, 7]);
  assert.deepEqual(order.blockIndexAt, [0, 1, 2, 2, 4]);
  assert.deepEqual(order.charsFrom, [2 + 13 + 3 + 2 + 4, 13 + 3 + 2 + 4, 3 + 2 + 4, 2 + 4, 4, 0]);
  assert.equal(stepFrom(order, 0, 1), 1);
  assert.equal(stepFrom(order, 7, 1), null);
  assert.deepEqual(prefetchWindow(order, 1, 2), [1, 4, 5]);
});

test('readSkipped on: own sentences, never the cue', () => {
  const order = buildPlaybackOrder(doc, true);
  assert.deepEqual(order.sentenceIds, [0, 2, 3, 4, 5, 6, 7]);
});

test('any id resolves onto the current order', () => {
  const off = buildPlaybackOrder(doc, false);
  const on = buildPlaybackOrder(doc, true);
  assert.equal(resolvePlayable(off, 3), 1);   // a code line → the code block's cue
  assert.equal(resolvePlayable(on, 1), 2);    // the cue → the code's first line
  assert.equal(resolvePlayable(off, 6), 7);   // a caption → the next playable block
  assert.equal(resolvePlayable(off, 99), null);
  assert.equal(startingSentenceId(off, null), 0);
  assert.equal(startingSentenceId(off, 99), 0);
  assert.equal(firstPlayableFromBlock(off, 3), 7);
});

test('page markers', () => {
  const plan = planPageMarkers(doc);
  assert.deepEqual([...plan.beforeSentence], [0, 5]);
  assert.deepEqual([...plan.beforeBlock], [1]);
});

test('pdf checks', () => {
  assert.equal(checkPdfFile({ name: 'a.pdf', type: '', size: 10 }, 100), null);
  assert.equal(checkPdfFile({ name: 'a.txt', type: 'text/plain', size: 10 }, 100), 'FileNotPdf');
  assert.equal(checkPdfFile({ name: 'a.pdf', type: 'application/pdf', size: 101 }, 100), 'FileTooLarge');
});
```

---

## 6. Hooks

Rules §1: a hook is the use case. It holds state, calls `api`, and returns data and actions, never markup.
**Use-case state** is what was loaded, chosen or will be saved. **View state** (popover open, slider draft, follow
mode) lives in the component, or in a view hook used by the page. Every hook aborts its in-flight requests on
unmount (`AbortController`) and ignores `AbortError`. No hook leaves an empty `catch`: log with `console.error` or
`console.warn` and put a user-facing error in state.

### 6.1 `usePlayback` and `sentenceAudioBuffer` (COPY VERBATIM)

How it works:
- **Order:** `buildPlaybackOrder(document, readSkipped)` is memoized. `currentSentenceId` is *derived* from the
  requested id (`startingSentenceId(order, requested)`), so it is always playable in the current order.
- **Buffer:** one `SentenceAudioBuffer` per (document, voice). Changing the voice makes a new buffer, clears the old
  one and reloads the current sentence in the new voice. Speed and `readSkipped` are passed to the buffer with
  `setContext` and go on every later request as `rate` and `readSkipped`.
- **Effect order:** create the audio element → load the current sentence (pause; `keepOnly(current + next 2)`;
  `get(current)` → set `src` and rate → mark loaded; prefetch the next 2) → play or pause to match intent once
  loaded.
- **Status:** `preparing` = wants to play, but the current sentence isn't loaded yet. `ended` = finished the last
  sentence in the order. Pressing play from `ended` restarts at the first sentence.
- **`ended` event** → the next sentence in the order (`stepFrom`). The skip and cue rule is already in the order.
- **Errors:** `SpeechEngineUnavailable` calls `onEngineUnavailable()`, which makes the engine banner appear. Any
  other audio error goes in `audioError`, and the page shows a banner with "Try again" (`retryAudio`) and "Skip
  sentence" (`skipFailedSentence`). An `AbortError` from `play()` (interrupted by a jump) is ignored.
  `NotAllowedError` (autoplay blocked) just pauses.
- **Position saving:** saves 1.5 s after the sentence stops changing, also on pause, on `pagehide` and on unmount.
  It never re-sends the same id.

```ts
// src/hooks/sentenceAudioBuffer.ts
import { api } from '../api';
import type { DocumentId } from '../types/document';

interface Entry { controller: AbortController; promise: Promise<string>; url: string | null; }

/** Fetches sentence audio as object URLs, and frees what is no longer wanted. Not React: usePlayback owns one per document + voice. */
export interface SentenceAudioBuffer {
  /** Object URL for the sentence's audio. Repeated calls share one request. */
  get(sentenceId: number): Promise<string>;
  /** Applies to requests made from now on. */
  setContext(next: AudioRequestContext): void;
  /** Aborts and revokes every entry not in `sentenceIds`. */
  keepOnly(sentenceIds: readonly number[]): void;
  clear(): void;
}

/** Sent with each request. Changing it needs no new buffer, because the audio itself doesn't change. */
export interface AudioRequestContext { rate: number; readSkipped: boolean; }

export const createSentenceAudioBuffer = (documentId: DocumentId, voiceId: string): SentenceAudioBuffer => {
  const entries = new Map<number, Entry>();
  let context: AudioRequestContext = { rate: 1, readSkipped: false };

  const release = (entry: Entry) => {
    entry.controller.abort();
    if (entry.url) URL.revokeObjectURL(entry.url);
  };

  const get = (sentenceId: number): Promise<string> => {
    const existing = entries.get(sentenceId);
    if (existing) return existing.promise;
    const controller = new AbortController();
    const entry: Entry = { controller, url: null, promise: Promise.resolve('') };
    entry.promise = api.getSentenceAudio({ documentId, sentenceId, voiceId, ...context }, controller.signal).then((blob) => {
      // Released while the body was arriving: never create a URL nobody will revoke.
      if (controller.signal.aborted) throw new DOMException('Aborted', 'AbortError');
      entry.url = URL.createObjectURL(blob);
      return entry.url;
    });
    // A failed request must not stay cached, so the next get() tries again.
    entry.promise.catch(() => {
      if (entries.get(sentenceId) === entry) entries.delete(sentenceId);
    });
    entries.set(sentenceId, entry);
    return entry.promise;
  };

  const keepOnly = (sentenceIds: readonly number[]) => {
    for (const [sentenceId, entry] of entries) {
      if (sentenceIds.includes(sentenceId)) continue;
      release(entry);
      entries.delete(sentenceId);
    }
  };

  const clear = () => keepOnly([]);

  const setContext = (next: AudioRequestContext) => {
    context = next;
  };

  return { get, setContext, keepOnly, clear };
};
```

```ts
// src/hooks/usePlayback.ts
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../api';
import { ClientError, hasErrorName, isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import { DEFAULT_SPEED, POSITION_SAVE_DELAY_MS, PREFETCH_AHEAD, SECONDS_PER_CHAR } from '../config/constants';
import type { PlaybackSpeed } from '../config/constants';
import type { OpenedDocument } from '../types/document';
import { buildPlaybackOrder, firstPlayableFromBlock, prefetchWindow, secondsLeft, startingSentenceId, stepFrom } from '../utils/playbackOrder';
import { createSentenceAudioBuffer } from './sentenceAudioBuffer';
import type { SentenceAudioBuffer } from './sentenceAudioBuffer';

/** Which sentence the audio element holds, for which voice buffer and retry round. */
interface LoadedAudio { buffer: SentenceAudioBuffer; sentenceId: number; reloadToken: number; }

export type PlaybackStatus = 'paused' | 'preparing' | 'playing' | 'ended';

interface UsePlaybackArgs {
  document: OpenedDocument;
  voiceId: string;
  /** False while the engine is checking or failed: play is refused. */
  canPlay: boolean;
  onEngineUnavailable: () => void;
}

export interface Playback {
  /** Always a playable sentence of the current order, or null if nothing is playable. */
  currentSentenceId: number | null;
  /** Index in document.blocks of the block being read (for highlighting code/table boxes and cues). */
  currentBlockIndex: number | null;
  currentPageIndex: number | null;
  /** 0-based position in the playback order, and its length. */
  position: number;
  playableCount: number;
  status: PlaybackStatus;
  speed: PlaybackSpeed;
  /** "Read code & tables aloud". */
  readSkipped: boolean;
  secondsLeft: number;
  /** Audio for the current sentence could not be made or played. Engine failures go to onEngineUnavailable instead. */
  audioError: AppError | null;
  togglePlay: () => void;
  pause: () => void;
  next: () => void;
  previous: () => void;
  /** Any sentence id, even a non-playable one (it resolves to the next playable sentence). */
  jumpTo: (sentenceId: number) => void;
  jumpToBlock: (blockIndex: number) => void;
  seekToPosition: (position: number) => void;
  setSpeed: (speed: PlaybackSpeed) => void;
  setReadSkipped: (readSkipped: boolean) => void;
  retryAudio: () => void;
  skipFailedSentence: () => void;
}

const playbackStatus = ({ ended, playing, isLoaded }: { ended: boolean; playing: boolean; isLoaded: boolean }): PlaybackStatus => {
  if (ended) return 'ended';
  if (!playing) return 'paused';
  return isLoaded ? 'playing' : 'preparing';
};

export const usePlayback = ({ document, voiceId, canPlay, onEngineUnavailable }: UsePlaybackArgs): Playback => {
  const [readSkipped, setReadSkipped] = useState(false);
  const order = useMemo(() => buildPlaybackOrder(document, readSkipped), [document, readSkipped]);
  // The user's last request, which may be any sentence id. The playable current sentence is derived from it,
  // so flipping readSkipped mid-book needs no extra state: a cue becomes the block's first sentence and back.
  const [requestedSentenceId, setRequestedSentenceId] = useState<number | null>(document.lastSentenceId);
  const currentSentenceId = startingSentenceId(order, requestedSentenceId);
  const [wantsToPlay, setWantsToPlay] = useState(false);
  const [ended, setEnded] = useState(false);
  const [loaded, setLoaded] = useState<LoadedAudio | null>(null);
  const [speed, setSpeed] = useState<PlaybackSpeed>(DEFAULT_SPEED);
  const [audioError, setAudioError] = useState<AppError | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const currentRef = useRef(currentSentenceId);
  const orderRef = useRef(order);
  const speedRef = useRef(speed);
  const engineUnavailableRef = useRef(onEngineUnavailable);

  useEffect(() => {
    currentRef.current = currentSentenceId;
    orderRef.current = order;
    speedRef.current = speed;
    engineUnavailableRef.current = onEngineUnavailable;
  });

  const buffer = useMemo(() => createSentenceAudioBuffer(document.id, voiceId), [document.id, voiceId]);
  useEffect(() => () => buffer.clear(), [buffer]);
  // Declared before the load effect so the first request already carries the right rate and switch.
  useEffect(() => buffer.setContext({ rate: speed, readSkipped }), [buffer, speed, readSkipped]);

  // One audio element for the whole session: it keeps the playback rate and a single set of listeners.
  useEffect(() => {
    const audio = new Audio();
    audio.preload = 'auto';
    audioRef.current = audio;
    const handleEnded = () => {
      const current = currentRef.current;
      const following = current === null ? null : stepFrom(orderRef.current, current, 1);
      if (following === null) {
        setWantsToPlay(false);
        setEnded(true);
        return;
      }
      setRequestedSentenceId(following);
    };
    const handleError = () => {
      if (!audio.getAttribute('src')) return;
      setWantsToPlay(false);
      setAudioError(new ClientError('AudioPlaybackFailed'));
    };
    audio.addEventListener('ended', handleEnded);
    audio.addEventListener('error', handleError);
    return () => {
      audio.removeEventListener('ended', handleEnded);
      audio.removeEventListener('error', handleError);
      audio.pause();
      audio.removeAttribute('src');
      audio.load();
      audioRef.current = null;
    };
  }, []);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.defaultPlaybackRate = speed;
    audio.playbackRate = speed;
  }, [speed]);

  // Load the current sentence into the element and prefetch the next ones.
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || currentSentenceId === null) return;
    let cancelled = false;
    audio.pause();
    const wanted = prefetchWindow(order, currentSentenceId, PREFETCH_AHEAD);
    buffer.keepOnly(wanted);
    buffer.get(currentSentenceId).then((url) => {
      if (cancelled) return;
      audio.src = url;
      // Loading a new src resets playbackRate to defaultPlaybackRate, so set both.
      audio.defaultPlaybackRate = speedRef.current;
      audio.playbackRate = speedRef.current;
      setLoaded({ buffer, sentenceId: currentSentenceId, reloadToken });
    }).catch((error: unknown) => {
      if (cancelled || isAbortError(error)) return;
      setWantsToPlay(false);
      if (hasErrorName(error, 'SpeechEngineUnavailable')) {
        engineUnavailableRef.current();
        return;
      }
      console.error('Sentence audio failed', error);
      setAudioError(toAppError(error));
    });
    for (const sentenceId of wanted.slice(1)) {
      buffer.get(sentenceId).catch((error: unknown) => {
        if (!isAbortError(error)) console.warn('Prefetch failed; will retry when needed', error);
      });
    }
    return () => {
      cancelled = true;
    };
  }, [buffer, order, currentSentenceId, reloadToken]);

  // Play or pause the element to match intent, once the current sentence is loaded.
  const isLoaded = loaded !== null && loaded.buffer === buffer && loaded.sentenceId === currentSentenceId && loaded.reloadToken === reloadToken;
  // The engine failing (canPlay false) stops playback without extra state.
  const isPlaying = wantsToPlay && canPlay;
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (!isPlaying || !isLoaded) {
      audio.pause();
      return;
    }
    audio.play().catch((error: unknown) => {
      // AbortError: play() was interrupted by pause() or a new src, which is expected when jumping.
      if (isAbortError(error)) return;
      console.error('Audio play() failed', error);
      setWantsToPlay(false);
      if (!(error instanceof DOMException && error.name === 'NotAllowedError')) setAudioError(new ClientError('AudioPlaybackFailed', { cause: error }));
    });
  }, [isPlaying, isLoaded]);

  // Save the position a moment after it stops changing, on pause, and when leaving.
  const lastSavedRef = useRef(document.lastSentenceId);
  const savePosition = useCallback((sentenceId: number) => {
    if (sentenceId === lastSavedRef.current) return;
    lastSavedRef.current = sentenceId;
    api.saveReadingPosition(document.id, sentenceId).catch((error: unknown) => console.warn('Could not save reading position', error));
  }, [document.id]);

  useEffect(() => {
    if (currentSentenceId === null) return;
    const timer = setTimeout(() => savePosition(currentSentenceId), POSITION_SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [currentSentenceId, savePosition]);

  useEffect(() => {
    const flush = () => {
      if (currentRef.current !== null) savePosition(currentRef.current);
    };
    window.addEventListener('pagehide', flush);
    return () => {
      window.removeEventListener('pagehide', flush);
      flush();
    };
  }, [savePosition]);

  const jumpTo = useCallback((sentenceId: number) => {
    setEnded(false);
    setAudioError(null);
    setRequestedSentenceId(sentenceId);
  }, []);

  const jumpToBlock = useCallback((blockIndex: number) => {
    const target = firstPlayableFromBlock(order, blockIndex);
    if (target !== null) jumpTo(target);
  }, [order, jumpTo]);

  const pause = useCallback(() => {
    setWantsToPlay(false);
    if (currentRef.current !== null) savePosition(currentRef.current);
  }, [savePosition]);

  const togglePlay = useCallback(() => {
    if (isPlaying) {
      pause();
      return;
    }
    if (!canPlay || currentSentenceId === null) return;
    setAudioError(null);
    if (ended) {
      setEnded(false);
      setRequestedSentenceId(order.sentenceIds[0] ?? null);
    }
    setWantsToPlay(true);
  }, [isPlaying, canPlay, currentSentenceId, ended, order, pause]);

  const step = useCallback((offset: number) => {
    if (currentSentenceId === null) return;
    const target = stepFrom(order, currentSentenceId, offset);
    if (target !== null) jumpTo(target);
  }, [currentSentenceId, order, jumpTo]);

  const next = useCallback(() => step(1), [step]);
  const previous = useCallback(() => step(-1), [step]);

  const seekToPosition = useCallback((position: number) => {
    const target = order.sentenceIds[position];
    if (target !== undefined) jumpTo(target);
  }, [order, jumpTo]);

  const retryAudio = useCallback(() => {
    setAudioError(null);
    setReloadToken((token) => token + 1);
    setWantsToPlay(canPlay);
  }, [canPlay]);

  const skipFailedSentence = useCallback(() => {
    next();
    setWantsToPlay(canPlay);
  }, [next, canPlay]);

  const position = currentSentenceId === null ? 0 : order.positionOf.get(currentSentenceId) ?? 0;

  return {
    currentSentenceId,
    currentBlockIndex: currentSentenceId === null ? null : order.blockIndexAt[position] ?? null,
    currentPageIndex: currentSentenceId === null ? null : order.pageIndexOf.get(currentSentenceId) ?? null,
    position,
    playableCount: order.sentenceIds.length,
    status: playbackStatus({ ended, playing: isPlaying, isLoaded }),
    speed,
    readSkipped,
    secondsLeft: currentSentenceId === null ? 0 : secondsLeft(order, currentSentenceId, speed, SECONDS_PER_CHAR),
    audioError,
    togglePlay,
    pause,
    next,
    previous,
    jumpTo,
    jumpToBlock,
    seekToPosition,
    setSpeed,
    setReadSkipped,
    retryAudio,
    skipFailedSentence,
  };
};
```

### 6.2 `useAudioExport` (COPY VERBATIM)

On mount it loads `listExports(documentId)` and takes the newest job (unless one was already started), so
reopening the reader shows a running export's progress or a finished export's download link.
It holds the chosen `format` (use-case state: it is what gets submitted). `start(voiceId, readSkipped)` posts
`{ voiceId, format, readSkipped }`. It polls `getExport` every second while the job is `queued` or `running`, and
stops on `done`, `failed` or `cancelled`. Cancel sends DELETE, then fetches the final state. `ExporterUnavailable`
and other request errors land in `error`.

```ts
// src/hooks/useAudioExport.ts
import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import { isAbortError, toAppError } from '../api/errors';
import type { AppError } from '../api/errors';
import { EXPORT_FORMATS, EXPORT_POLL_INTERVAL_MS } from '../config/constants';
import type { AudioExport, ExportFormat, ExportStatus } from '../types/api';
import type { DocumentId } from '../types/document';

const IN_PROGRESS: Record<ExportStatus, boolean> = { queued: true, running: true, done: false, failed: false, cancelled: false };

export interface AudioExportState {
  /** The latest known job, or null before the first export in this session. */
  job: AudioExport | null;
  isStarting: boolean;
  /** A request failed (start, poll or cancel). The job's own `failed` status is separate. */
  error: AppError | null;
  /** Set only when the job is done. */
  fileUrl: string | null;
  /** The format the next export will use (what will be submitted, so it lives here, not in the view). */
  format: ExportFormat;
  setFormat: (format: ExportFormat) => void;
  start: (voiceId: string, readSkipped: boolean) => void;
  cancel: () => void;
}

/** The document's latest whole-book export. Loaded on open (GET …/exports, newest first), so a running or finished export survives closing the reader. */
export const useAudioExport = (documentId: DocumentId): AudioExportState => {
  const [job, setJob] = useState<AudioExport | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [format, setFormat] = useState<ExportFormat>(EXPORT_FORMATS[0]);

  // Pick up the newest existing export, unless one was started in the meantime.
  useEffect(() => {
    const controller = new AbortController();
    api.listExports(documentId, controller.signal).then((items) => {
      const latest = items[0];
      if (latest) setJob((current) => current ?? latest);
    }).catch((cause: unknown) => {
      if (!isAbortError(cause)) setError(toAppError(cause));
    });
    return () => controller.abort();
  }, [documentId]);

  const start = useCallback((voiceId: string, readSkipped: boolean) => {
    setIsStarting(true);
    setError(null);
    api.startExport(documentId, { voiceId, format, readSkipped })
      .then(setJob)
      .catch((cause: unknown) => setError(toAppError(cause)))
      .finally(() => setIsStarting(false));
  }, [documentId, format]);

  const jobId = job?.id ?? null;
  const inProgress = job !== null && IN_PROGRESS[job.status];

  useEffect(() => {
    if (jobId === null || !inProgress) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      api.getExport(jobId, controller.signal).then(setJob).catch((cause: unknown) => {
        if (!isAbortError(cause)) setError(toAppError(cause));
      });
    }, EXPORT_POLL_INTERVAL_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [jobId, inProgress, job]);

  const cancel = useCallback(() => {
    if (jobId === null) return;
    api.cancelExport(jobId)
      .then(() => api.getExport(jobId))
      .then(setJob)
      .catch((cause: unknown) => setError(toAppError(cause)));
  }, [jobId]);

  const fileUrl = job?.status === 'done' ? api.exportFileUrl(job.id) : null;

  return { job, isStarting, error, fileUrl, format, setFormat, start, cancel };
};
```

### 6.3 Hooks you write

```ts
// src/config/routes.ts
export type Route = { name: 'library' } | { name: 'reader'; documentId: DocumentId };
export const parseRoute = (hash: string): Route => …;      // '#/read/<id>' (decodeURIComponent) → reader; anything else → library
export const routeHref = (route: Route): string => …;      // '#/' | `#/read/${encodeURIComponent(id)}`

// src/hooks/useHashRoute.ts: listens to 'hashchange'
export const useHashRoute = (): { route: Route; navigate: (route: Route) => void } => …;   // navigate sets window.location.hash
```

```ts
// src/hooks/useLibrary.ts
export type ImportState =
  | { phase: 'idle' }
  | { phase: 'importing'; fileName: string }
  | { phase: 'failed'; fileName: string; error: AppError };

export interface UseLibraryArgs { onDocumentReady: (documentId: DocumentId) => void; }

export interface Library {
  recent: DocumentSummary[];
  total: number;
  recentStatus: 'loading' | 'ready' | 'failed';
  recentError: AppError | null;
  isLoadingMore: boolean;
  reloadRecent: () => void;
  loadMore: () => void;            // next RECENT_PAGE_SIZE using offset = recent.length; appends
  importState: ImportState;
  importFile: (file: File) => void;
  resetImport: () => void;         // back to idle ("Choose another PDF")
}
export const useLibrary = ({ onDocumentReady }: UseLibraryArgs): Library => …;
```
`importFile` does these steps in order. It ignores the call if an import is already running.
1. `checkPdfFile(file, MAX_UPLOAD_BYTES)`: on a failure name, go to `failed` with `new ClientError(name)`.
2. Read the magic bytes: `(await file.slice(0, PDF_MAGIC.length).text()) === PDF_MAGIC`, otherwise `FileNotPdf`.
3. Go to `importing`, then `api.importPdf(file, signal)`. On success call `onDocumentReady(result.document.id)`
   (the page navigates to the reader). On an error go to `failed` with `toAppError(error)`.

The client checks are for UX only; the server re-checks everything (rules §3).

```ts
// src/hooks/useDocument.ts
export interface DocumentLoad {
  document: OpenedDocument | null;
  status: 'loading' | 'ready' | 'failed';
  error: AppError | null;
  retry: () => void;
}
export const useDocument = (documentId: DocumentId): DocumentLoad => …;
```

```ts
// src/hooks/useVoices.ts
export interface Voices {
  voices: Voice[];
  status: 'loading' | 'ready' | 'failed';
  /** Server default until the user picks one. Null until loaded; the page then uses FALLBACK_VOICE_ID. */
  selectedVoiceId: string | null;
  selectVoice: (voiceId: string) => void;
}
export const useVoices = (): Voices => …;
```

```ts
// src/hooks/useSpeechEngine.ts
export type EngineState = 'checking' | 'ready' | 'failed' | 'unreachable';   // unreachable = /health itself failed
export interface SpeechEngine {
  state: EngineState;
  recheck: () => void;          // the banner's Retry button
  markUnavailable: () => void;  // called by usePlayback on SpeechEngineUnavailable
}
export const useSpeechEngine = (): SpeechEngine => …;
```

```ts
// src/hooks/useFollowScroll.ts   (view state: losing it on remount loses nothing)
export interface FollowScroll { isFollowing: boolean; resumeFollowing: () => void; }
export const useFollowScroll = (activeSentenceId: number | null, activeBlockIndex: number | null): FollowScroll => …;
```
How `useFollowScroll` works:
- It starts with `isFollowing = true`.
- An effect on `[activeSentenceId, activeBlockIndex, isFollowing]`: when following, it finds
  `document.querySelector('[data-sentence-id="<id>"]')`, falling back to `[data-block-index="<index>"]`. The
  fallback is needed because cues and code/table lines have no element of their own.
  If the element's rect is above `0.15 × innerHeight`, or below `innerHeight − FOLLOW_BOTTOM_CLEARANCE_PX`
  (120 px, so it stays clear of the pill; make it a module constant), it calls
  `scrollIntoView({ block: 'center', behavior })`. `behavior` is `'auto'` on the first scroll after opening and
  whenever `matchMedia('(prefers-reduced-motion: reduce)')` matches; otherwise `'smooth'`. The effect never calls
  setState.
- Signs that the user is scrolling by hand set `isFollowing = false` (window listeners, passive where possible):
  `wheel`, `touchmove`, `keydown` of PageUp, PageDown, Home, End, ArrowUp or ArrowDown (unless the target is an
  input, select or textarea), and `pointerdown` whose target is `document.documentElement` (the scrollbar).
- `resumeFollowing()` sets it back to `true` (the effect then scrolls).
- ReaderScreen calls `resumeFollowing()` with every jump: clicking a sentence, prev/next (buttons and keys), the
  slider, and "Scroll to current sentence".

```ts
// src/hooks/useReaderShortcuts.ts
export interface ReaderShortcuts { onTogglePlay: () => void; onNext: () => void; onPrevious: () => void; }
export const useReaderShortcuts = (shortcuts: ReaderShortcuts): void => …;
```
How `useReaderShortcuts` works: one `keydown` listener on `window`. It ignores events with Ctrl, Alt or Meta held,
and events whose target is an `input`, `select` or `textarea`.
- **Space:** play/pause. Skip it when the target is a `button`, `a` or `summary` (Space already activates those),
  and when `event.repeat` is set. Call `preventDefault()` so the page doesn't scroll.
- **→ / ←:** next / previous sentence, with `preventDefault()`.

Keep the latest callbacks in a ref so the listener is added only once.

---

## 7. Pages and components

### 7.1 App and pages

- **`App.tsx`:** `const { route, navigate } = useHashRoute()`. It renders `<ReaderPage documentId=… onBack=…/>` for
  `reader`, else `<LibraryPage onOpenDocument=…/>`. It also adds `dragover`/`drop` listeners on `window` that call
  `preventDefault()`, so a file dropped on the reader doesn't make the browser navigate to the PDF.
- **`LibraryPage`** (`useLibrary({ onDocumentReady: (id) => navigate reader })`):
  - The header is D's `.empty-top`: `<h1>` "Lectrice" (styled 1rem, weight 600) + "Reads PDFs aloud, locally" in `--fg2`.
  - Then `<DropZone>`, then the "Recent documents" section.
  - The whole page (min-height 100dvh) is the drop target. `dragenter`/`dragover` set `isDragging` (view state;
    use a counter, because `dragleave` fires on child elements). On `drop`, take the first file and call
    `importFile`. Only one file is used; ignore the others.
  - Recent section: "Loading recent documents…" while loading. On failure, a `StatusBanner` (error) with a "Try
    again" button. If empty: "No documents yet. Open a PDF to start." Otherwise `RecentDocumentList`.
- **`ReaderPage`** (`documentId`, `onBack`): calls `useDocument`, `useVoices` and `useSpeechEngine` **in
  parallel** (no waterfall; rules §4).
  - While loading: a centred "Opening document…".
  - On `DocumentNotFound`: heading "Document not found", the text "It may have been removed from the library.",
    and a "Back to library" button. Other errors: `error.message` + "Try again" (`retry`) + "Back to library".
  - When ready: `<ReaderScreen …/>`.
  - It sets `document.title` to `"<title> · Lectrice"` and restores `"Lectrice"` on unmount.
- **`ReaderScreen`** receives a loaded `OpenedDocument` plus the voices and engine objects and `onBack`.
  - It calls `usePlayback({ document, voiceId: voices.selectedVoiceId ?? FALLBACK_VOICE_ID, canPlay: engine.state === 'ready', onEngineUnavailable: engine.markUnavailable })`,
    `useAudioExport(document.id)`, `useFollowScroll(playback.currentSentenceId, playback.currentBlockIndex)` and
    `useReaderShortcuts`. The export start is wired as `() => exportState.start(voiceId, playback.readSkipped)`.
  - It memoizes `buildPageLabelLookup(document.pages)` and `planPageMarkers(document)` (both stable for the
    document's lifetime).
  - It renders, top to bottom: banners (sticky, top), `ReadingView`, `MiniPlayer` (fixed, bottom).
  - Banners:
    - `engine.state === 'failed'`: "Speech engine failed to load. Playback is unavailable until it is restarted." + **Retry** (`recheck`)
    - `engine.state === 'unreachable'`: "Can't reach the Lectrice server. Is it running?" + **Retry**
    - `playback.audioError`: "Couldn't make audio for this sentence. <message>" + **Try again** + **Skip sentence**

### 7.2 Pure helpers you write (in `utils/`, add tests in §10)

```ts
// src/utils/playerStatus.ts
export type PlayerState = 'engineFailed' | 'preparing' | 'playing' | 'paused' | 'ended';
export const playerState = (engine: EngineStateLike, status: PlaybackStatus): PlayerState => …;
//   engine 'failed' | 'unreachable' → 'engineFailed'; otherwise the playback status.
//   Declare `type EngineStateLike = 'checking' | 'ready' | 'failed' | 'unreachable'` locally, or import it as a type (types only!).
export const playerStatusText = (state: PlayerState, pageText: string, speed: number): string => …;
//   Use a Record<PlayerState, …> (no chained ternaries):
//   engineFailed → 'Speech engine unavailable'
//   preparing    → 'Preparing audio…'
//   playing      → pageText, plus ` · ${speed}×` only when speed !== 1      e.g. 'p. 37 of 412 · 1.25×'
//   paused       → `Paused · ${pageText}`
//   ended        → 'Finished'
//   pageText = `${formatPageMarker(label)} of ${lastLabel}`, where label = printedLabel(labels, playback.currentPageIndex)
//   and lastLabel = printedLabel(labels, document.pageCount)
```

```ts
// src/utils/exportPhase.ts
export type ExportPhase = 'idle' | 'starting' | 'queued' | 'running' | 'done' | 'failed' | 'cancelled';
export const exportPhase = (job: AudioExport | null, isStarting: boolean): ExportPhase => …;   // isStarting wins; null job → idle; else job.status
```

```ts
// src/utils/fileName.ts
export const safeFileName = (name: string): string => …;   // replace / \ : * ? " < > | and control chars with '-', collapse spaces, trim, max 120 chars
//   download name = safeFileName(`${document.title} (${voiceName}).${format}`)
```

### 7.3 Component props (presentational: props in, events out, no `api` imports)

```ts
// ReadingView
interface ReadingViewProps {
  document: CleanDocument;
  activeSentenceId: number | null;
  activeBlockIndex: number | null;            // playback.currentBlockIndex
  isPreparing: boolean;                       // status === 'preparing'
  readSkipped: boolean;                       // only changes the skipped blocks' label
  markers: PageMarkerPlan;
  pageLabels: ReadonlyMap<number, string>;
  onSentenceSelect: (sentenceId: number) => void;   // → playback.jumpTo (any id; it resolves)
  onBlockSelect: (blockIndex: number) => void;      // a click on a code/table box → playback.jumpToBlock
}
// BlockView (React.memo). The parent passes activeSentenceId only to the active block (index === activeBlockIndex), null to the others.
interface BlockViewProps {
  block: Block;
  blockIndex: number;
  isActive: boolean;                          // blockIndex === activeBlockIndex
  activeSentenceId: number | null;
  isRead: boolean;                            // blockIndex < activeBlockIndex
  isPreparing: boolean;                       // false unless isActive
  readSkipped: boolean;
  markers: PageMarkerPlan;
  pageLabels: ReadonlyMap<number, string>;
}
// Sentence
export type SentenceState = 'unread' | 'read' | 'active' | 'activePreparing';
interface SentenceProps { sentence: Sentence; state: SentenceState; }
// PageMarker
interface PageMarkerProps { label: string; pageIndex: number; }
// SkippedBlockView: code / table / caption (named ...View so it never clashes with the SkippedBlock type)
export type SkippedBlockState = 'unread' | 'read' | 'active' | 'activePreparing';
interface SkippedBlockViewProps {
  block: SkippedBlock;
  blockIndex: number;
  state: SkippedBlockState;                   // for code/table, the whole box is the "sentence"
  activeSentenceId: number | null;            // captions highlight their own sentence spans
  isPreparing: boolean;
  readSkipped: boolean;
  marker: React.ReactNode;                    // the PageMarker to place first, or null
}
// ReadSkippedSwitch: <label><input type="checkbox" role="switch" checked onChange/> Read code &amp; tables aloud</label>
interface ReadSkippedSwitchProps { checked: boolean; onChange: (checked: boolean) => void; }
// MiniPlayer
interface MiniPlayerProps {
  playerState: PlayerState;
  statusText: string;
  canGoBack: boolean;
  canGoForward: boolean;
  showBackToCurrent: boolean;                 // !isFollowing
  onTogglePlay: () => void;
  onPrevious: () => void;
  onNext: () => void;
  onBackToCurrent: () => void;
  popover: React.ReactNode;                   // <PlayerPopover/>; MiniPlayer only decides whether it is open
}
// PlayerPopover
interface PlayerPopoverProps {
  id: string;                                 // for aria-controls on the "more" button
  position: number;
  playableCount: number;
  positionText: string;                       // aria-valuetext: 'Sentence 120 of 7,939, page 37'
  locationText: string;                       // 'Page 37 · PDF page 61 of 429'
  timeLeftText: string;                       // formatTimeLeft(playback.secondsLeft)
  onSeek: (position: number) => void;
  speed: PlaybackSpeed;
  onSpeedChange: (speed: PlaybackSpeed) => void;
  voiceGroups: VoiceGroup[];
  voiceId: string;
  voicesAvailable: boolean;
  onVoiceChange: (voiceId: string) => void;
  readSkipped: boolean;
  onReadSkippedChange: (readSkipped: boolean) => void;
  exportPanel: React.ReactNode;
  onOpenAnother: () => void;
}
// ProgressSlider: native <input type="range" min=0 max=playableCount-1>.
// It keeps a draft value while dragging (view state) and calls onSeek on pointerup, keyup and blur (if changed),
// so dragging doesn't fire a jump per pixel.
interface ProgressSliderProps { position: number; max: number; valueText: string; onSeek: (position: number) => void; }
// SpeedPicker: <label>Speed <select> with options from PLAYBACK_SPEEDS ('0.75×' … '2×')
interface SpeedPickerProps { value: PlaybackSpeed; onChange: (speed: PlaybackSpeed) => void; }
// VoicePicker: <label>Voice <select> with an <optgroup> per group and option text formatVoiceOption(voice); disabled when !available
interface VoicePickerProps { groups: VoiceGroup[]; value: string; available: boolean; onChange: (voiceId: string) => void; }
// ExportPanel
interface ExportPanelProps {
  phase: ExportPhase;
  format: ExportFormat;
  onFormatChange: (format: ExportFormat) => void;   // select options from EXPORT_FORMATS, labels from EXPORT_FORMAT_LABEL; disabled while starting/queued/running
  sentencesDone: number;
  sentencesTotal: number;
  timeLeftText: string | null;                // from estimatedSecondsLeft via formatTimeLeft, null if unknown
  voiceName: string;                          // 'Heart'
  readSkipped: boolean;                       // shown in the help text
  fileUrl: string | null;
  downloadName: string;
  errorMessage: string | null;                // a request error (useAudioExport.error?.message)
  canStart: boolean;                          // engine ready
  onStart: () => void;
  onCancel: () => void;
}
// DropZone
interface DropZoneProps {
  importState: ImportState;
  isDragging: boolean;
  maxUploadMb: number;
  onFileChosen: (file: File) => void;
  onChooseAnother: () => void;
}
// RecentDocumentList
interface RecentDocumentListProps {
  documents: DocumentSummary[];
  now: Date;                                  // passed in so formatting stays pure
  hrefFor: (documentId: DocumentId) => string;
  hasMore: boolean;                           // recent.length < total
  isLoadingMore: boolean;
  onShowMore: () => void;
}
// StatusBanner
interface StatusBannerProps { tone: 'error' | 'warning'; message: string; actions?: React.ReactNode; }
```

### 7.4 What each component renders

- **ReadingView:** an `<article>` with one `onClick` handler. Find the target with
  `event.target instanceof Element ? event.target.closest('[data-sentence-id]') : null`. Do nothing if
  `window.getSelection()?.toString()` is not empty (the user is selecting text to copy). Otherwise call
  `onSentenceSelect(Number(el.dataset.sentenceId))`. If there is no sentence target, look for
  `closest('[data-jump-block-index]')` and call `onBlockSelect`.
  - It renders `blocks.map((block, blockIndex) => <BlockView key={block.id} …/>)`, comparing `blockIndex` with
    `activeBlockIndex` for `isActive` and `isRead`.
  - Every block's outer element gets `data-block-index={blockIndex}` and `content-visibility: auto; contain-intrinsic-size: auto 7rem;`.
- **BlockView:**
  - Headings: `Record<HeadingLevel, 'h1' | … | 'h6'>` picks the tag.
  - Paragraph: `<p>`. `listItem`: `<p>` with a hanging "•" drawn by `::before`. `note`: `<p>` with the note style (§8.4).
  - `code`, `table`, `caption`: `<SkippedBlockView>`.
  - Before a sentence whose id is in `markers.beforeSentence`, render a `<PageMarker>` inline (it can be
    mid-paragraph). A code or table block whose id is in `markers.beforeBlock` gets one before its box.
  - Sentences are separated by `{' '}`.
- **Sentence:** `<span data-sentence-id={id} className={STATE_CLASS[state]} aria-current={active ? 'true' : undefined}>{text}</span>`.
  Use a `Record<SentenceState, string>` for the classes. Text goes in as a React child (escaped): **never
  `dangerouslySetInnerHTML`**.
- **PageMarker:** `<span className={styles.marker} title={formatPdfPage(pageIndex)}>{formatPageMarker(label)}</span>`.
  - It has `user-select: none`, so copied text doesn't include it. TTS never sees it: it isn't a sentence.
  - Placement on wide screens (≥ 721 px): `position: absolute; left: 0; width: calc(var(--gutter) - .75rem); text-align: right`.
    `top` stays `auto`, so it sits on the line where it appears in the text flow. The containing block is the
    block element (`position: relative`, padding-inline `var(--gutter)`).
  - Placement on ≤ 720 px: `position: static; display: inline-block; margin-right: .35em` as a small inline chip.
- **SkippedBlockView:**
  - **code / table:** `<figure data-jump-block-index={blockIndex}>` with a `<figcaption>` and `<pre><code>{block.text}</code></pre>`
    (tables use `<pre>` too: the extracted text keeps its columns as spaces). `overflow-x: auto` sits on the
    `pre`, so the page itself never scrolls sideways. The whole box is clickable (it jumps to the cue, or to the
    first line when the switch is on).
  - The figcaption comes from `Record<'code' | 'table', string>` = `{ code: 'Code', table: 'Table' }`, plus
    " · read aloud" when `readSkipped` is on, " · says “Code example.”" (the cue text) when off with a cue, and
    " · not read aloud" when off without one.
  - The state class marks reading: `active` → `border-left: 4px solid var(--accent)` on the figure (the highlight
    yellow is too dark in dark mode to show on a box); `activePreparing` adds a dotted `--fg2` border on the
    `pre`; `read` → figcaption in `--fg2` (it already is). Individual code lines are not highlighted.
  - **caption:** `<p>` in the caption style (§8.4) whose sentences are normal `<Sentence>` spans, so they highlight
    when the switch is on. Clicking one with the switch off jumps to the next playable sentence (`jumpTo`
    resolves it).
  - The cue is never rendered.
- **MiniPlayer:**
  - `popoverOpen` is view state.
  - Layout, left to right: [Previous] [Play/Pause] [Next] [status] [Scroll to current (only if `showBackToCurrent`)] [More].
  - The play button's look comes from a `Record<PlayerState, { icon, label, busy, disabled }>`:
    - `playing` → pause icon, "Pause"
    - `preparing` → wait icon, "Preparing audio", `aria-busy="true"`, still clickable (it pauses)
    - `paused` / `ended` → play icon, "Play"
    - `engineFailed` → play icon, "Play", disabled
  - The status is `<span role="status">`, class `prep` when `preparing`.
  - "More": `aria-expanded`, `aria-controls={popover id}`, `aria-label="More controls"`.
  - While open, Escape closes it and returns focus to "More" (a keydown listener added only while open). A
    `pointerdown` outside the pill closes it too. It is a disclosure, not a modal: no focus trap, focus stays put
    when it opens.
- **PlayerPopover** (D's `.pop`), top to bottom:
  1. A row with the ProgressSlider (`aria-label="Position in book"`) and the time left (`--fg2`). Under it, the
     location text (`--fg2`, .8125rem).
  2. A settings grid with SpeedPicker, VoicePicker and ReadSkippedSwitch.
  3. The export panel slot.
  4. A row with the "Open another PDF…" button.
- **ExportPanel**: copy per phase (use a `Record<ExportPhase, …>`):

| phase | shows |
|---|---|
| idle | a "Format" select (MP3 / M4B (audiobook)) and the button **"Export whole book"**; help text "Heart voice · code and tables: cue only" (or "read aloud" when `readSkipped`) "· runs in the background while you listen." Disabled with "Needs the speech engine" when `!canStart`. |
| starting | the same button, disabled, label "Starting export…" |
| queued | "Waiting to start…", `<progress>` with no value (indeterminate), button **"Cancel export"** |
| running | `<progress value={done} max={total}>`, "1,204 of 7,939 sentences · 12 min left" (numbers via `toLocaleString('en')`; drop the time part when unknown), **"Cancel export"** |
| done | `<a className={primary button} href={fileUrl} download={downloadName}>Download MP3</a>` (or "Download M4B"; use `job.format`), "Ready · Heart voice", button "Export again" (= onStart) |
| failed | error text "The export stopped before it finished." (err colours), button "Try again" (= onStart) |
| cancelled | "Export cancelled.", then the idle button |

  If `errorMessage` is set, show it in an error-coloured line with `role="alert"` under whatever the phase shows.
  For `ExporterUnavailable` that is the server's message, "Audio export isn't available (ffmpeg is missing)."

  The download name is `safeFileName(`${title} (${voiceName}).${EXPORT_FORMAT_EXTENSION[job.format]}`)`.

- **DropZone:** D's `.drop`, three phases. Use a hidden `<input type="file" accept="application/pdf,.pdf">`
  triggered by the button through a ref. Reset its `value` after each pick, so choosing the same file again
  still fires.
  - **idle:**
    - `<h2>Open a PDF to start listening</h2>`
    - "Lectrice extracts the text, removes running headers, page numbers, footnotes, citation markers and reference lists, and reads what is left aloud with a local voice. Nothing leaves this computer."
    - primary button "Open PDF…"
    - muted small: "or drop a file anywhere in this window · PDFs with a text layer, up to 200 MB" (from `maxUploadMb`)
    - `isDragging` adds a class with `border-color: var(--accent)` and `border-style: solid`.
  - **importing** (`aria-live="polite"`):
    - `<h2>Reading {fileName}…</h2>`
    - "Extracting text and cleaning out headers, page numbers, footnotes and references."
    - `<progress aria-label="Preparing document">`
  - **failed** (`role="alert"`, D's `.drop.err`):
    - `<h2>{copy.title}</h2>`
    - `<p><b>{fileName}</b> {copy.body}</p>`
    - button "Choose another PDF" (→ `onChooseAnother`)
  - Error copy lives in `importErrorCopy.ts` as `Partial<Record<AppErrorName, { title: string; body: string }>>`,
    with a fallback of `{ title: "Couldn't open this PDF", body: error.message }`:

| name | title | body (follows the bold file name) |
|---|---|---|
| PdfNoText | This PDF has no selectable text | looks like a scanned document. There is no text layer for Lectrice to read, so it cannot be read aloud. |
| PdfEncrypted | This PDF is password-protected | is locked. Remove the password in a PDF viewer, then open it again. |
| PdfInvalid | This file isn't a readable PDF | could not be read. It may be damaged, or not really a PDF. |
| FileNotPdf | This file is not a PDF | is not a PDF. Lectrice can only read PDF files. |
| FileEmpty | This file is empty | has no content. |
| FileTooLarge, UploadTooLarge | This file is too large | is bigger than 200 MB, the largest file Lectrice accepts. (use `MAX_UPLOAD_MB`) |
| NetworkError | Can't reach Lectrice | could not be sent. Check that the Lectrice server is running, then try again. |

- **RecentDocumentList:** a `<section aria-labelledby>` with `<h2>Recent documents</h2>` and a `<ul>`. Each `<li>`
  holds one `<a href={hrefFor(id)}>` that is the whole row:
  - Left: the title (weight 600; fall back to `sourceFileName` if the title is blank) and, under it, "429 pages ·
    opened 2 hours ago" (`--fg2`, .875rem).
  - Right: "Continue · 34%" if `percentRead(...)` isn't null, otherwise "Start". Under it, a 4 px progress bar
    (`aria-hidden`, track `--line`, fill `--accent`), shown only when started.
  - Below the list: a "Show more" button when `hasMore`, disabled and labelled "Loading…" while `isLoadingMore`.
- **StatusBanner:** D's `.engine-banner` for `error` (`role="alert"`), and D's `.notice.warn` colours for
  `warning` (`role="status"`). Message on the left, actions on the right, wrapping on narrow screens. Buttons
  inside use `color: currentColor; border-color: currentColor; background: transparent`.
- **icons.tsx:** export small components (`PrevIcon`, `NextIcon`, `PlayIcon`, `PauseIcon`, `WaitIcon`,
  `MoreIcon`, `LocateIcon`). Each is `<svg viewBox="0 0 24 24" width height aria-hidden="true">` with these paths
  (from D, plus Locate):
  - prev `<path fill="currentColor" d="M6 5h2v14H6zM20 5v14L9 12z"/>` (20 px)
  - next `<path fill="currentColor" d="M16 5h2v14h-2zM4 5v14l11-7z"/>` (20 px)
  - play `<path fill="currentColor" d="M7 4v16l14-8z"/>` (22 px)
  - pause `<path fill="currentColor" d="M6 4h4v16H6zM14 4h4v16h-4z"/>` (22 px)
  - wait `<circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="2" stroke-dasharray="14 8"/>` (22 px, static, no spin)
  - more: three `<circle r="2" fill="currentColor">` at cx 5 / 12 / 19, cy 12 (20 px)
  - locate `<path fill="currentColor" d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm9 3h-2.07A7 7 0 0 0 13 5.07V3h-2v2.07A7 7 0 0 0 5.07 11H3v2h2.07A7 7 0 0 0 11 18.93V21h2v-2.07A7 7 0 0 0 18.93 13H21v-2zm-9 6a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/>` (20 px), aria-label "Scroll to the current sentence"

---

## 8. Styling (option D, faithfully)

### What comes from D vs what is shared in the mock-up file

- **Shared by all options (keep):** tokens, base reset, `:focus-visible`, `.btn`, `.icon-btn`, `.field select`,
  `.status`, `.engine-banner`, `.notice.warn`, the `.doc` text styles, the sentence states `.s`, `.s:hover`,
  `.s.read`, `.s.active` and `.s.active.prep`, and the empty / loading / error overlay (`.empty`, `.drop`).
- **D-specific (keep):** `.d-doc` sizes, `.pill`, `.pop` and their media queries.
- **Ignore:** the prototype chrome (`.top`, tabs, notes, theme select, engine checkbox), options A/B/C, the
  outline, and the removed-content styles (`.cut*`, `.pages`, `.page*`). Also D's `.prog` div slider (replaced
  by a native range) and the buffered bar.

### 8.1 `styles/tokens.css` (COPY VERBATIM)

```css
:root {
  --bg: #ffffff; --bg2: #f3f3ef; --fg: #1c1f24; --fg2: #5b636d; --line: #d3d3cd;
  --accent: #1d5fae; --on-accent: #ffffff;
  --hl: #ffe58f; --hl-fg: #1c1f24;
  --warn-bg: #fff4d1; --warn-fg: #5e4300;
  --err-bg: #fbe7e5; --err-fg: #8b1a12;
  --serif: Charter, "Iowan Old Style", Georgia, "Times New Roman", serif;
  --sans: system-ui, -apple-system, "Segoe UI", Roboto, Ubuntu, Cantarell, sans-serif;
  --mono: ui-monospace, "SF Mono", "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
  --gutter: 4.5rem;          /* page-marker margin; 0 on phones */
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #121417; --bg2: #1c1f24; --fg: #e8e6e1; --fg2: #a8adb5; --line: #363a41;
    --accent: #8ab8ff; --on-accent: #0b1a33;
    --hl: #4d4214; --hl-fg: #fff6d8;
    --warn-bg: #3a2f0c; --warn-fg: #f3d98a;
    --err-bg: #44181a; --err-fg: #f4b1ad;
    color-scheme: dark;
  }
}
@media (max-width: 720px) { :root { --gutter: 0rem; } }
```

### 8.2 `styles/global.css`

`*{box-sizing:border-box}`; `body{margin:0; background:var(--bg); color:var(--fg); font-family:var(--sans)}`;
`button,select,input{font:inherit; color:inherit}`; `:focus-visible{outline:2px solid var(--accent); outline-offset:2px}`;
`[hidden]{display:none !important}`. Nothing else. No `overflow: hidden` on body: the window scrolls.

### 8.3 `styles/controls.module.css` (D's values)

- `.button`: padding .4rem .75rem; border 1px solid var(--line); background var(--bg); border-radius 6px;
  cursor pointer; font-size .875rem; white-space nowrap; `text-decoration: none` (it is also used on the download
  `<a>`); display inline-block.
- `.primary`: background var(--accent); color var(--on-accent); border-color transparent.
- `.button:disabled`: opacity .45; cursor default.
- `.iconButton`: 40 px circle; display inline-grid; place-items center; padding 0; border 1px solid var(--line);
  border-radius 50%; background var(--bg); cursor pointer. `:disabled` → opacity .45, cursor default.
  `[aria-busy="true"]` → background var(--bg2); color var(--fg2); border-color var(--line).
- `.play`: 48 px; background var(--accent); color var(--on-accent); border-color transparent.
- `.select`: padding .3rem .4rem; border 1px solid var(--line); border-radius 6px; background var(--bg);
  max-width 100%; min-width 0.

### 8.4 Reader text (`ReadingView.module.css`, `Sentence.module.css`)

- Article: `position: relative; max-width: calc(40rem + 2 * var(--gutter)); margin: 0 auto; padding: 2rem 1rem 9rem;
  font-family: var(--serif); font-size: 1.1875rem; line-height: 1.7; overflow-wrap: anywhere`. The 9rem bottom
  padding keeps the last lines clear of the pill.
- Block wrapper: `position: relative; padding-inline: var(--gutter); content-visibility: auto; contain-intrinsic-size: auto 7rem`.
- h1 `1.65rem / 1.25`, margin `0 0 1rem`; h2 `1.25rem`, margin `1.6em 0 .5em`; h3–h6 `1.1rem`, margin `1.3em 0 .4em`;
  p margin `0 0 1em`. note: `font: .95em/1.6 var(--sans); padding: .5rem .75rem; border-left: 3px solid var(--line);
  background: var(--bg2)`. caption: `font: .875rem/1.5 var(--sans); color: var(--fg2); margin: -.5em 0 1em`
  (active sentences still use `--hl` / `--hl-fg`). listItem: `padding-left: 1.25em` with `::before { content: "•"; position: absolute; margin-left: -1.1em }`.
- Sentence: `cursor: pointer; border-radius: 2px; padding: .05em 0; -webkit-box-decoration-break: clone; box-decoration-break: clone`.
  - `:hover`: `text-decoration: underline dotted; text-underline-offset: .15em`.
  - `.read`: `color: var(--fg2)`.
  - `.active`: `background: var(--hl); color: var(--hl-fg)`. The active colour must always override `.read`.
    Dark `--fg2` on `--hl` is only 4.42:1.
  - `.activePreparing` = `.active` + `border-bottom: 2px dotted var(--fg2)`.
- Page marker: `font: .75rem/1.2 var(--sans); color: var(--fg2); user-select: none; white-space: nowrap`.
  - Wide: absolutely positioned in the gutter as in §7.4, with `padding-top: .45em` so it lines up with the
    text line.
  - Phone: inline chip, `border: 1px solid var(--line); border-radius: 4px; padding: 0 .3em`.
- Code/table figure: `margin: 0 0 1em; border-left: 4px solid transparent; padding-left: .5rem` (so the active
  border doesn't shift the layout). Figure `cursor: pointer`. figcaption: `font: .75rem var(--sans); color: var(--fg2); margin-bottom: .25rem`.
  pre: `margin: 0; padding: .75rem 1rem; background: var(--bg2); border: 1px solid var(--line); border-radius: 6px;
  font: .875rem/1.5 var(--mono); overflow-x: auto; white-space: pre; tab-size: 2`.

### 8.5 Pill and popover (`MiniPlayer.module.css`, `PlayerPopover.module.css`)

- Pill: `position: fixed; left: 50%; transform: translateX(-50%); bottom: max(1rem, env(safe-area-inset-bottom));
  display: flex; align-items: center; gap: .3rem; padding: .3rem .4rem; border: 1px solid var(--line);
  background: var(--bg2); border-radius: 999px; font-size: .85rem; max-width: calc(100% - 2rem); z-index: 10`.
  - Pill icon buttons are 36 px; the play button is 44 px.
  - Status: `color: var(--fg2); padding: 0 .4rem; max-width: 14rem; white-space: nowrap; overflow: hidden;
    text-overflow: ellipsis; min-width: 0`. `.prep` → `color: var(--fg); font-weight: 600`. (D used 11rem; the
    page text needs 14rem.)
- Popover: `position: absolute; bottom: calc(100% + .5rem); right: 0; width: min(21rem, calc(100vw - 2rem));
  max-height: calc(100dvh - 6rem); overflow-y: auto; background: var(--bg2); border: 1px solid var(--line);
  border-radius: 10px; padding: .85rem; display: grid; gap: .65rem; font-size: .875rem`.
  - `.row`: `display: flex; align-items: center; gap: .5rem; flex-wrap: wrap`.
  - `.settings`: `display: grid; gap: .6rem`. Each field is a `<label>` with `display: grid; grid-template-columns: 4rem 1fr; align-items: center`.
  - Range: `flex: 1 1 8rem; min-width: 6rem; accent-color: var(--accent)`.
  - Export `<progress>`: `width: 100%; accent-color: var(--accent)`.
- `@media (max-width: 720px)`: pill `left: 1rem; right: 1rem; transform: none; max-width: none; justify-content: center`;
  popover `left: 0; right: 0; width: auto`.
- `@media (max-width: 420px)`: hide the pill status (the page is still in the popover).
- Banners: `position: sticky; top: 0; z-index: 11`. D's `.engine-banner`: `display: flex; gap: .5rem 1rem;
  align-items: center; justify-content: space-between; flex-wrap: wrap; background: var(--err-bg);
  color: var(--err-fg); padding: .5rem 1rem; font-size: .875rem`. The warning tone uses `--warn-bg` / `--warn-fg`.

### 8.6 Library (`LibraryPage.module.css`, `DropZone.module.css`, `RecentDocumentList.module.css`)

- Header (D's `.empty-top`): `padding: .6rem 1rem; border-bottom: 1px solid var(--line); display: flex; gap: .75rem;
  align-items: baseline; font-size: .875rem`. h1: `margin: 0; font-size: 1rem; font-weight: 600`.
- Drop (D's `.drop`): `margin: 3rem auto 2rem; width: min(28rem, calc(100% - 2rem)); border: 2px dashed var(--line);
  border-radius: 12px; padding: 2rem 1.5rem; text-align: center; display: grid; gap: .75rem; justify-items: center;
  line-height: 1.5`. h2: `margin: 0; font-size: 1.25rem`. p: `margin: 0`. `.err`: `border-style: solid;
  border-color: var(--err-fg); background: var(--err-bg); color: var(--err-fg)`. progress: `width: 60%`.
  Muted small text: `color: var(--fg2); font-size: .85rem`.
- Recent: `max-width: 40rem; margin: 0 auto; padding: 0 1rem 3rem`. h2: `font-size: 1rem; margin: 0 0 .5rem`.
  `ul`: no bullets, padding 0, margin 0. Row link: `display: grid; grid-template-columns: 1fr auto; gap: .25rem 1rem;
  align-items: center; padding: .75rem .25rem; border-bottom: 1px solid var(--line); color: inherit;
  text-decoration: none`. `:hover` underlines the title only. The title gets `min-width: 0; overflow-wrap: anywhere`.

### 8.7 Contrast (measured with the WCAG formula; keep these pairs, don't add new ones without measuring)

| Pair | Light | Dark |
|---|---|---|
| `--fg` on `--bg` / `--bg2` | 16.52 / 14.85 | 14.79 / 13.25 |
| `--fg2` on `--bg` / `--bg2` (status, markers, read text, meta) | 6.09 / 5.47 | 8.18 / 7.33 |
| `--accent` on `--bg` / `--bg2` (links, focus ring) | 6.36 / 5.72 | 9.12 / 8.17 |
| `--on-accent` on `--accent` (play, primary) | 6.36 | 8.58 |
| `--hl-fg` on `--hl` (active sentence) | 13.26 | 9.23 |
| `--warn-fg` on `--warn-bg` | 8.39 | 9.49 |
| `--err-fg` on `--err-bg` | 7.84 | 8.44 |
| `--fg2` on `--hl` (**must not happen**) | 4.89 | **4.42 ✗** |

`--line` on `--bg` is 1.5:1. That is fine for decorative borders and tracks, but never use it for text.
Disabled controls (opacity .45) are exempt.

### 8.8 Phone width (≈ 375 px)

There must be no horizontal page scroll at 375 px. Check `document.documentElement.scrollWidth <= innerWidth`.
- The pill spans the width (1rem gutters), and the status is hidden at ≤ 420 px.
- The popover is full width above the pill and scrolls inside if it is tall.
- Page markers become inline chips. Code scrolls inside its own box.
- Recent rows wrap their titles.

### 8.9 No polish

No transitions, animations, shadows or hover effects beyond the ones listed (D's dotted sentence underline, the
link title underline). The native indeterminate `<progress>` animation is fine. Smooth scrolling is only used for
follow mode and is off under reduced motion.

---

## 9. Keyboard and accessibility

- **Reader keys:** Space = play/pause, → = next sentence, ← = previous sentence (§6.3 for when they are ignored).
  Escape closes the popover and puts focus back on "More". Tab reaches every control in the pill and popover in
  visual order.
- **Library:** "Open PDF…" works from the keyboard, and so do the recent-document links (they are real `<a href>`).
- Sentences are not tab stops (there are 8,000 of them). Keyboard users move with ←/→ and the slider. The active
  sentence has `aria-current="true"`.
- Every icon button has an `aria-label`, and every SVG has `aria-hidden="true"`. The play button's label follows
  its state. While preparing it has `aria-busy="true"`.
- "More": `aria-expanded` + `aria-controls`. The slider: `aria-label="Position in book"` + `aria-valuetext`.
- Live regions: the pill status is `role="status"`; the import progress is `aria-live="polite"`; errors are
  `role="alert"`.
- There is one `<h1>` per screen: "Lectrice" in the Library, and the document's own first heading in the reader.
- Focus is always visible: 2 px `--accent` outline, offset 2 px, on every interactive element, including links
  and the range.
- `lang="en"` is on `<html>`.

---

## 10. Tests to add to `tests/utils.test.ts`

Keep the given tests and add:
- `playerStatusText`: every `PlayerState`, and speed 1 vs 1.25 for `playing`.
- `exportPhase`: null job, starting, and each status.
- More `buildPlaybackOrder` cases on a fixture with a `table` that has a cue and a `note` block (the note always
  plays; the table plays its cue when off and its sentences when on).
- `safeFileName`: a title with `/`, `:` and `?`.
- `percentRead`: null, 0 of 70, the last sentence.
- `formatVoiceOption` and `groupVoices` on two US voices and one UK voice (the group order follows first appearance).

---

## 11. Acceptance checklist (tick each one in mock mode before reporting)

Open `http://localhost:5173/` (dark and light: switch the OS or the browser's emulation, and check both).

**Library**
- [ ] Recent documents show 3 entries, newest first: "Designing Calm Software" (Continue · 34%), "The Long Book"
      (Start), "Quarterly Notes" (Continue · 35%). Each has a page count and "opened … ago".
- [ ] "Open PDF…" opens the file picker. Choosing a `.txt` file shows "This file is not a PDF" without calling the API.
- [ ] Dropping a PDF anywhere on the page shows "Reading <name>…" with an indeterminate progress bar, then opens the reader.
- [ ] Going back, the imported file is at the top of recent documents. Importing it again opens it again.
- [ ] `?mock=noText#/`: importing shows the "no selectable text" error in red; "Choose another PDF" resets it.
- [ ] `?mock=empty#/` shows "No documents yet…". `?mock=offline#/` shows an error banner with "Try again".
- [ ] Opening "Quarterly Notes" shows "Document not found" and "Back to library" works.

**Reader (Designing Calm Software)**
- [ ] It opens scrolled to the saved sentence (id 23, page "1"), highlighted. The pill shows "Paused · p. 1 of 5".
- [ ] Page markers: "p. xii" before the Preface, "p. xiii", "p. 1"… and one **inside** the paragraph on PDF page
      15 → 16 ("p. 3"). Hovering one shows "PDF page 16". Markers sit in the left margin on desktop and inline on a phone.
- [ ] The code block is monospace, captioned "Code · says “Code example.”". Playing through it plays the cue once
      (the box gets the accent border), then moves on. The caption under it ("Listing 1.1…") is small text and is
      skipped silently.
- [ ] Turning on "Read code & tables aloud" (popover): the label changes to "Code · read aloud", the 6 code lines
      play (the box stays highlighted), and the caption's sentence highlights when it plays. If the switch is
      flipped while the cue is current, playback continues from the first code line. Flipping it back while on a
      code line goes back to the cue. Time left and the slider's length change (63 ↔ 69).
- [ ] The audio requests in the Network tab carry `rate=` (the current speed) and `readSkipped=`.
- [ ] Play: the button shows the wait icon and "Preparing audio…", then audio plays (a tick per sentence) and the
      highlight moves on every sentence. Read sentences turn grey.
- [ ] Space toggles play/pause; → and ← move one sentence; clicking a sentence jumps there (and keeps playing if it was playing).
- [ ] Selecting text with the mouse does not jump.
- [ ] Jumping far ahead shows "Preparing audio…" briefly. Jumping back and forth fast doesn't play stale audio
      (check the Network tab: aborted requests are expected).
- [ ] Speed 2× makes sentences finish twice as fast, and the pitch of the tick stays the same. The time left in the popover halves.
- [ ] Changing the voice while playing restarts the current sentence (a new audio request with the new `voice=` parameter).
- [ ] Scrolling away with the wheel stops auto-scroll and shows the locate button. Clicking it scrolls back and
      hides it. Prev/next and sentence clicks also resume following.
- [ ] "More" opens the popover: slider, time left, "Page … · PDF page … of 18", speed, voice (American / British
      English groups, "Heart (US, female)" selected), export, "Open another PDF…". Escape and clicking outside close it.
- [ ] Dragging the slider and releasing jumps once (one audio request, not dozens).
- [ ] Back in the Library after listening a while, "Designing Calm Software" shows a higher percentage (position saved).
- [ ] At the end of the book the status says "Finished". Play starts again from the top.

**Errors in the reader**
- [ ] `?mock=engineFailed#/read/mock-calm`: the red banner "Speech engine failed to load…" shows with Retry, play
      is disabled, the text is readable, and export is disabled.
- [ ] `?mock=audioError#/read/mock-calm`: click sentence id 5 ("That sounds simple, and in a sense it is.") and play. A banner appears with
      "Try again" and "Skip sentence". Skip moves on and keeps playing.

**Export**
- [ ] "Export whole book" (MP3) → "Waiting to start…" → a progress bar with "N of 63 sentences · … left" →
      after about 20 s, "Download MP3" downloads `Designing Calm Software (Heart).mp3`.
- [ ] Choosing M4B and exporting gives "Download M4B" and a `.m4b` name. With the switch on, the total is 69.
      The format select is disabled while an export runs.
- [ ] `?mock=exporterMissing`: starting an export shows "Audio export isn't available (ffmpeg is missing)." as an alert.
- [ ] "Cancel export" in the middle shows "Export cancelled." and the export button again.
- [ ] Start an export, go back to the Library, then reopen the book: the popover shows the same export still
      progressing (or "Download …" if it finished). A reload of the page forgets mock exports; that is expected.
- [ ] `?mock=exportFailed`: it stops at about 40 % with "The export stopped before it finished." and "Try again".

**Large document (The Long Book)**
- [ ] It opens in under about 2 s. Scrolling to the bottom is smooth. Ctrl+F finds a word near the end.
- [ ] While playing, the highlight moves with no visible lag. React DevTools "Highlight updates" (if you have it)
      shows only one or two blocks re-rendering per sentence.

**Layout and accessibility**
- [ ] At 375 px wide: no horizontal scroll on either screen; the pill spans the width; the status is hidden; the
      popover fits; the last line of text can scroll above the pill.
- [ ] Every control is reachable with Tab and has a visible focus ring. Icon buttons announce their labels.
- [ ] Light and dark both look like D. The active sentence is readable in both.

**Code review (rules §2, §11)**
- [ ] `npm run build`, `npm run lint` (zero warnings) and `npm test` pass.
- [ ] No `any`, no `as unknown as`, no `dangerouslySetInnerHTML`, no `enum`, no empty `catch`.
- [ ] No component imports `api`. Every import is on one line. Fixed sets use `Record` maps, not chained ternaries.
- [ ] Every CSS class is used and every `styles.x` exists. No leftover template files. No unused props or state.

---

## 12. Do not

- Don't add any dependency (no antd or other UI kit, router, state library, query library, icon pack, CSS
  framework, test runner or polyfill).
- Don't use `dangerouslySetInnerHTML`, `any`, double casts, `enum`, or constructor parameter properties.
- Don't call `fetch` outside `src/api/client.ts`, and don't call the API from components.
- Don't add features outside this brief: no delete, no settings page, no theme toggle, no "show removed text", no
  sign-in, no localStorage persistence, no export history, no keyboard shortcuts beyond §9.
- Don't display cues, and don't special-case block kinds in playback: the order comes only from `buildPlaybackOrder`.
- Don't add speculative animation, transitions or shadows (§8.9).
- Don't use web fonts or external requests: the app is fully local.
- Don't run git and don't install anything globally.
- Don't change the COPY VERBATIM code to suit taste. If it really doesn't work, fix the smallest thing and say
  exactly what you changed in your report.

When done, report: what you built, the output of the three commands, the ticked checklist (and anything you could
not tick, with the reason), and any place where you deviated from this brief.
