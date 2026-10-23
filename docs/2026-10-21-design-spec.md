# Lectrice: design spec v1 (2026-10-21)

Status: **draft for the user's review.** Nothing is built yet. Items marked **(proposed)** were not
asked for explicitly. Keep them or drop them during review.

## 1. What v1 does

1. The user opens a PDF in the browser (drag-and-drop or the file picker).
2. The backend fingerprints the file. If it is new, it extracts and cleans the text **once** and saves
   the result in a local library. If it was opened before, the saved version loads instantly.
3. The reader shows the cleaned text full-screen (design **option D**: large text plus a small floating
   player). Page markers ("p. 37") show where each part sits in the original PDF.
4. Press play: the app reads sentence by sentence in the `af_heart` voice and highlights the sentence
   being read. Click any sentence to jump there. Speed and voice can be changed.
5. The Open-PDF screen lists **recent documents** with reading progress, and "continue" picks up
   at the saved position.
6. Audio is generated **ahead of the listener** (target 10 minutes) and about **an hour** of it is
   kept per book (§4).
7. **Export the whole book as an audio file** in the background, to use anywhere (§4a).

**Not in v1:** mobile app, GPU, voice cloning, a "show what was removed" toggle,
deleting documents from the library, OCR for scanned PDFs, languages other than English.

## 2. Architecture

Two separate projects that only talk over HTTP:

```
lectrice/
  backend/            Rust Cargo workspace (core + HTTP server)
  frontend/           TypeScript + React + Vite (its own npm project)
  models/             Kokoro bundle (already there)
  rules/              coding rules
```

### Backend crates (dependencies point inward only)

```
lectrice-domain  ←  lectrice-infra
       ↑                 ↑
lectrice-app  ←──────────┘ (wired together only in lectrice-api)
       ↑
lectrice-api   (axum binary: routes, controllers, DTOs, presenters, composition root)
lectrice-ffi   (later: mobile bindings over lectrice-app, same as the api crate's role)
```

| Crate | Rules layer | Contains | Depends on |
|---|---|---|---|
| `lectrice-domain` | Domain + interfaces | Entities (`Document`, `Block`, `Sentence`, `PageRef`, `TextRun`, `Voice`, `Audio`), traits (§3), the error registry, **pure cleaning rules** (header/footer, page numbers, de-hyphenation, column order) | `thiserror` only |
| `lectrice-infra` | Infrastructure | Adapters: `PdfiumTextSource`, `SentencexSplitter`, `KokoroSpeechEngine`, `WavEncoder`, `FsDocumentRepository`, `FsAudioCache` | domain + `pdfium-render`, `sentencex`, `sherpa-onnx`, `hound`, `serde_json` |
| `lectrice-app` | Services | Use cases (§4), the speech worker queue | domain only (adapters arrive as `Arc<dyn Trait>`) |
| `lectrice-api` | Controllers + routes | Thin controllers, input DTOs + validation, output presenters, `AppError → HTTP` mapping, config loading, logging setup, wiring | all of the above + `axum`, `tower-http`, `tokio`, `tracing-subscriber` |

The mobile app later swaps `lectrice-api` for `lectrice-ffi`. Domain, app and infra stay as they are.

## 3. Domain interfaces (traits)

```rust
pub trait PdfTextSource      { fn read_runs(&self, pdf: &[u8]) -> Result<PdfContent, AppError>; }      // runs + page labels + outline
pub trait DocumentCleaner    { fn clean(&self, content: PdfContent) -> Result<CleanedDocument, AppError>; }
pub trait SentenceSplitter   { fn split(&self, text: &str) -> Vec<String>; }
pub trait SpeechEngine       { fn id(&self) -> &str; fn voices(&self) -> Vec<Voice>; fn synthesize(&self, request: SpeechRequest) -> Result<Audio, AppError>; }
pub trait AudioEncoder       { fn mime_type(&self) -> &str; fn encode(&self, audio: &Audio) -> Result<Vec<u8>, AppError>; }
pub trait DocumentReader     { fn find(&self, id: &DocumentId) -> Result<Option<Document>, AppError>; fn list_recent(&self, options: ListOptions) -> Result<Page<DocumentSummary>, AppError>; }
pub trait DocumentWriter     { fn save(&self, document: &Document, original_pdf: &[u8]) -> Result<(), AppError>; fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> Result<(), AppError>; }
pub trait AudioCache         { fn get(&self, key: &AudioKey) -> Result<Option<Vec<u8>>, AppError>; fn put(&self, key: &AudioKey, bytes: &[u8]) -> Result<(), AppError>; }
```

- Read and write repositories are split (ISP, as in the rules).
- **`SpeechEngine` is the plug-in point.** Nothing outside `KokoroSpeechEngine` mentions Kokoro,
  sherpa-onnx, speaker ids, lexicons or model paths. A factory in `lectrice-api` picks the adapter
  from `speech.engine` in the config.
- `DocumentCleaner` is pure domain logic over `TextRun { pdf_page, text, font_name, font_size, bbox }`,
  so it can be unit-tested without a PDF library. The exact rule list comes from the PDF survey
  (`research/2026-10-21-pdf-survey.md`, in progress).

## 4. Use cases (services)

All follow the rules: `run(ctx, args)`, context first. Inputs are normalized before use. Errors are
raised by registry name. Every call boundary is logged.

| Service | Does |
|---|---|
| `ImportPdf` | Validate it's a PDF (magic bytes, size) → SHA-256 → `find`. **If found:** touch `last_opened_at` and return it. **If not:** `read_runs` → `clean` → split sentences → assign ids → `save`. Returns `{ summary, already_in_library }` |
| `GetDocument` | Load the full cleaned document, touch `last_opened_at` |
| `ListRecentDocuments` | Paginated, newest `last_opened_at` first |
| `SaveReadingPosition` | Store the last sentence id, so recent documents can show progress and "continue" (approved 2026-10-21) |
| `ListVoices` | Engine voices + the default voice id |
| `SpeakSentence` | Validate the voice and sentence → cache `get` → on a miss, queue synthesis on the speech worker → encode → cache `put` → return bytes |

**Speech worker:** one dedicated thread owns the engine and takes jobs from a **priority queue**:
(1) the sentence the listener needs now, (2) read-ahead, (3) export. Requests for the same
`AudioKey` that are already in flight are merged. Audio is always made at **1× speed**; the browser
applies the chosen speed (`playbackRate`, pitch preserved), so the cache stays valid at any speed.

**Measured speed (2026-10-21, this laptop, CPU only, Ryzen 9 6900HX, `af_heart`, a Kubernetes Book
page):** 8 threads → 41 s of speech in 9.3 s, i.e. **≈ 4.4× faster than real time** (4 threads:
3.7×). Model load 1.2 s. Speaking rate ≈ 189 words/min (18 chars/s). So:

| Job | CPU time |
|---|---|
| One typical sentence | ≈ 1 s |
| 10 minutes of speech | ≈ 2.3 min |
| 1 hour of speech | ≈ 14 min |
| A whole ~8-hour book (export) | ≈ 1 h 50 min |

GPU should be much faster (not measured yet).

**Read-ahead (user requirement):** each audio request also queues the following spoken sentences at
priority (2) until about `audio.read_ahead_minutes` (default **10**) of **listening time** is ready or
queued, estimated from text length (18 chars/s at 1×).

**It scales with playback speed (user requirement).** The audio request carries the listener's
current speed (`rate`, 0.75–2). Audio is made at 1×, so 10 minutes of listening at 2× needs
**10 × 2 = 20 minutes** of 1× audio ahead. In general: `ahead_1x_minutes = read_ahead_minutes × rate`.
The generator runs at 4.4×, so even at 2× it stays ahead with a 2.2× margin (20 minutes ahead is
reached after about 8 minutes of listening at 2×). A speed change or a jump re-plans the queue, and
old queued read-ahead jobs are cancelled.

**Keep about an hour per book:** the cache keeps up to `audio.keep_minutes_per_document`
(default **60**, also counted in listening time, so × `rate`) of audio per document plus voice,
deleting the sentences **farthest behind** the reading position first. A global disk cap
(`audio.cache_max_mb`) is the safety net.

### 4a. Export the whole book (user requirement)

| Service | Does |
|---|---|
| `StartAudioExport` | Create an export job (document, voice, format) and queue every spoken sentence at priority (3), reusing cached sentences |
| `GetAudioExport` | Status, progress (sentences done / total), estimated time left |
| `CancelAudioExport` | Stop a running export |

- The output is **one audio file per book**, in **MP3** (plays everywhere) or **M4B** (an audiobook
  file with chapter marks taken from the headings; user decision 2026-10-21). It is written to
  `exports/<docId>/<voiceId>.<ext>` and downloaded from the browser.
- Encoding goes through an `AudioExporter` trait. The v1 adapter `FfmpegExporter` pipes PCM into the
  system `ffmpeg` (installed: `ffmpeg-free` 8.0.1 with `libmp3lame` and `aac`), plus a chapters file
  for M4B. As a separate program, it also avoids linking the LGPL LAME library into our app. A mobile
  build would swap in a different adapter.
- Exporting runs in the background at the lowest priority, so playback never waits for it.
- Export audio skips the cache's 1-hour limit: it streams each sentence straight into the encoder,
  so a whole book never sits in the cache.

## 5. The saved document (JSON, the app's own format)

```json
{
  "id": "3f2a…(sha256 of the PDF bytes)",
  "title": "Clean Architecture",
  "sourceFileName": "Clean_Architecture_….pdf",
  "pageCount": 429,
  "cleanerVersion": 1,
  "pages": [ { "index": 1, "label": "i" }, { "index": 25, "label": "1" } ],
  "toc": [
    { "id": 0, "title": "PART I Introduction", "level": 1, "sentenceId": 234, "pageIndex": 25 },
    { "id": 1, "title": "Chapter 1 What Is Design and Architecture?", "level": 2, "sentenceId": 271, "pageIndex": 27 },
    { "id": 2, "title": "The Goal?", "level": 3, "sentenceId": 298, "pageIndex": 28 }
  ],
  "blocks": [
    { "id": 0, "kind": "heading", "level": 1, "pageIndex": 25, "sentences": [ { "id": 0, "text": "What Is Design and Architecture?", "pageIndex": 25 } ] },
    { "id": 1, "kind": "paragraph", "pageIndex": 25, "sentences": [ { "id": 1, "text": "This chapter opens with a short story…", "pageIndex": 25 } ] },
    { "id": 2, "kind": "code", "pageIndex": 26, "spoken": false, "text": "public class Foo { … }" }
  ]
}
```

- **Location aids (user requirement):** every block and sentence has the `pageIndex` (PDF page,
  1-based) it starts on. `pages` maps each PDF page to its **printed label**. The reading view shows a
  small page marker wherever the page changes ("p. 37", with "PDF page 61" on hover). Markers are
  **never spoken**. Headings are kept, shown and spoken.
- Sentence ids are global and sequential. They are what the audio route, the highlight and the reading
  position use.
- Block kinds v1: `heading`, `paragraph`, `listItem`, `note` (spoken), and `code`, `caption`,
  `table` (**skipped by default**, user decisions 2026-10-21). They are shown on screen. Instead of
  reading them, playback says a short cue once per block: **"Code example."** for code and
  **"Table."** for tables; captions are skipped silently. Bibliography and copyright pages are shown
  but silent. The TOC and the index are dropped.
- Skipped blocks (`code`, `caption`, `table`) have **both** `text` (shown as-is, e.g. code keeps its
  line breaks and indentation) **and** `sentences` (used only when "read aloud" is on). `listItem`
  blocks have no grouping in v1; each one renders as a bullet paragraph.
- **How the cue works:** a skipped code or table block carries a `cue` sentence with its own id
  (`{ "id": 812, "text": "Code example.", "pageIndex": 26 }`). It is not displayed, and it is played
  through the normal sentence audio route, so the frontend needs no special case. When "read aloud"
  is on, the block's own sentences play instead of the cue.
- **"Read code & tables aloud" option (user request).** Skipped blocks still get split into sentences
  and stored (marked `spoken: false`), so a switch in the player's "more" menu can read them anyway
  with no re-processing. Detection is required in v1 anyway, to skip them: monospace font for code
  (CA, DDS, K8s), smaller monospace for GoF, the caption regex, and table column shapes (from the
  survey's rules 9, 12 and 13). Better detection is phase 2.
- Default text fixes from the survey: GoF's missing spaces ("Forexample") are left as they are in v1.
- `cleanerVersion` is stored. Raising it makes the app re-clean old documents from their saved
  original PDFs.
- **Table of contents (`toc`, added 2026-10-23, cleaner version 3).** A flat list in reading order:
  `{ id, title, level, sentenceId, pageIndex }`. `level` 1 is the book's top level (a part or a
  chapter), 2 a sub-chapter, 3 below that; deeper entries are left out. A level is the entry's depth
  under the entries before it with a smaller source level, so it is never more than one deeper than
  the entry before and entries of the same source level stay siblings. `sentenceId` is the first sentence of that part (jump
  there to start it) and strictly increases; `pageIndex` is that sentence's PDF page (show it with
  `pages`). Built by `lectrice-domain/src/cleaning/toc.rs` once sentence ids exist: the PDF outline
  (bookmarks) when at least half of its entries are found as heading blocks (same words, ±1 page),
  else the heading blocks. Front matter (cover, title page, copyright, contents, "Tweet This
  Book!"), empty titles, entries whose page has no text (dropped TOC/index pages) and extra
  entries that share a page with the entry before (LinkedIn's outline holds post sentences) are
  skipped; a title stops at a zero-width space and is cut at 120 chars. It is `[]` when a PDF has
  neither an outline nor headings. Documents saved before version 3 load with `toc: []` until
  they are re-cleaned.

## 6. Storage on disk

```
~/.local/share/lectrice/                      (configurable: storage.data_dir)
  library/<docId>/original.pdf
  library/<docId>/document.json               (§5)
  library/<docId>/meta.json                   { addedAt, lastOpenedAt, lastSentenceId }
  audio/<docId>/c<cleanerVersion>/<engineId>/<voiceId>/<sentenceId>.wav
  exports/<docId>/<voiceId>.<ext>
```

- Writes go to a temp file and are then renamed, so a crash never leaves half a document.
- **Sizes:** WAV is about 48 KB per second, so 1 hour ≈ 170 MB. With 60 minutes kept per book and a
  global cap of `audio.cache_max_mb` (default 2048), the cache stays small. An exported 8-hour book
  as 64 kbps mono MP3 is about 230 MB.

## 7. HTTP API

JSON uses camelCase. Errors always look like
`{ "error": { "name": "PdfNoText", "code": 1005, "message": "This PDF has no selectable text." } }`.
Dates are ISO 8601 strings in UTC (`"2026-10-21T18:04:00Z"`).

| Method + path | Request | Success |
|---|---|---|
| `GET /api/health` | | `200 { status: "ok", speechEngine: "ready" | "failed" }` |
| `GET /api/voices` | | `200 { voices: [{ id: "af_heart", name: "Heart", language: "en-US" | "en-GB", gender: "female" | "male" }], defaultVoiceId: "af_heart" }` |
| `POST /api/documents` | multipart, field `file` (PDF, max `upload.max_mb`) | `201 { document: DocumentSummary, alreadyInLibrary: false }`, or `200 { …, alreadyInLibrary: true }` |
| `GET /api/documents?limit=20&offset=0` | | `200 { items: [DocumentSummary], total, limit, offset }` |
| `GET /api/documents/{id}` | | `200 Document` (§5, including `toc`) plus `lastSentenceId: number | null`, so "Continue" can resume |
| `PUT /api/documents/{id}/position` | `{ sentenceId }` | `204` |
| `GET /api/documents/{id}/sentences/{sentenceId}/audio?voice=af_heart&rate=1.5&readSkipped=false` | | `200 audio/wav` (also triggers read-ahead, scaled by `rate`; `readSkipped` includes code/captions/tables in the read-ahead) |
| `POST /api/documents/{id}/exports` | `{ voiceId, format: "mp3" | "m4b", readSkipped }` | `202 { export: AudioExport }` |
| `GET /api/documents/{id}/exports` | | `200 { items: [AudioExport] }`, newest first, so the reader can show a running or finished export after it's reopened |
| `GET /api/exports/{exportId}` | | `200 { export: AudioExport }` |
| `DELETE /api/exports/{exportId}` | | `204` (cancels) |
| `GET /api/exports/{exportId}/file` | | `200` the audio file (download) |

`AudioExport = { id, documentId, voiceId, format, status: "queued" | "running" | "done" | "failed" | "cancelled", sentencesDone, sentencesTotal, estimatedSecondsLeft | null }`

`DocumentSummary = { id, title, sourceFileName, pageCount, sentenceCount, addedAt, lastOpenedAt, lastSentenceId | null }`

- In development the Vite dev server proxies `/api` to the backend, so no CORS setup is needed.
  CORS for `server.allowed_origin` stays available for other setups.
- v1 imports synchronously: the upload request returns once cleaning is done. A 400-page book may
  take several seconds, so the frontend shows "Reading… cleaning text". If that turns out too slow, the
  next step is an import job with status polling.

## 8. Error registry (one place, `lectrice-domain/src/error.rs`)

| Name | Code | HTTP | Operational | User-facing message |
|---|---|---|---|---|
| `RequestInvalid` | 1000 | 400 | yes | The request isn't valid. |
| `UploadMissingFile` | 1001 | 400 | yes | No file was uploaded. |
| `RouteNotFound` | 1006 | 404 | yes | Not found. |
| `MethodNotAllowed` | 1007 | 405 | yes | Method not allowed. |
| `RequestCancelled` | 1008 | 499 | yes | (only for clients that already disconnected) |
| `UploadTooLarge` | 1002 | 413 | yes | This file is too large. |
| `PdfInvalid` | 1003 | 422 | yes | This file isn't a readable PDF. |
| `PdfEncrypted` | 1004 | 422 | yes | This PDF is password-protected. |
| `PdfNoText` | 1005 | 422 | yes | This PDF has no selectable text. |
| `DocumentNotFound` | 1101 | 404 | yes | Document not found. |
| `SentenceNotFound` | 1102 | 404 | yes | Sentence not found. |
| `VoiceUnknown` | 2001 | 400 | yes | Unknown voice. |
| `SpeechEngineUnavailable` | 2002 | 503 | yes | The speech engine isn't available. |
| `SpeechSynthesisFailed` | 2003 | 500 | no | Speech generation failed. |
| `ExportNotFound` | 3001 | 404 | yes | Export not found. |
| `ExportNotReady` | 3002 | 409 | yes | The audio file isn't ready yet. |
| `ExportFormatUnsupported` | 3003 | 400 | yes | That audio format isn't supported. |
| `ExporterUnavailable` | 3004 | 503 | yes | Audio export isn't available (ffmpeg is missing). |
| `StorageFailed` | 9001 | 500 | no | Something went wrong saving data. |
| `Internal` | 9999 | 500 | no | Something went wrong. |

Services raise by name (`ErrorName::PdfNoText`). Only `lectrice-api` turns a name into HTTP. Messages
for non-operational errors are generic, and the full cause is logged.

## 9. Logging

`tracing` with one JSON object per line. The fields follow the rules: `service`, `message`, `context`.
HTTP requests are logged by `TraceLayer`.

**Log only the important parts (user decision 2026-10-21).** Book content is **never** logged: no
PDF bytes, text runs, sentence or block text, full documents, or audio. Instead, log:

- service entry with its small args (document id, sentence id, voice id, file name, file size);
- outcomes of each step as counts and ids, e.g. `{ docId, pages: 429, blocks: 3120, sentences: 8012, durationMs: 6400 }`,
  `{ docId, sentenceId, voiceId, cacheHit: false, audioSeconds: 4.1, synthMs: 930 }`;
- every write (document saved, position saved, cache evictions, export progress every 5 %);
- every error with its name, code and full cause chain.

This replaces the rules' "log the whole returned value" for this project.

## 10. Speech engine: Kokoro adapter

- `sherpa-onnx` 1.13.8 (static). Model bundle at `models/kokoro-multi-lang-v1_0/`. Lexicon
  `lexicon-us-en.txt`, `data_dir` = `espeak-ng-data`, `num_threads` from config.
- Voices v1: the English ones (`af_*`, `am_*`, `bf_*`, `bm_*`, ids 0–27). Names are shown nicely
  ("Heart · US · female"). The name ↔ id table lives inside the adapter. British voices use the US
  lexicon in v1; check how they sound in the first voice test.
- Safety, because sherpa-onnx can kill the process on bad input: check that the model paths exist at
  startup (a failure marks the engine `failed`, and the server still runs and reports it through
  `/api/health`); strip `\0` from text; reject unknown voice ids before calling.
- Output: 24 000 Hz mono f32 → `WavEncoder` (16-bit PCM).

## 11. Configuration (`backend/config.toml`, every key overridable by `LECTRICE_*` env vars)

```toml
[server]   port = 7878            allowed_origin = "http://localhost:5173"
[storage]  data_dir = "~/.local/share/lectrice"
[audio]    read_ahead_minutes = 10   keep_minutes_per_document = 60   cache_max_mb = 2048
[upload]   max_mb = 200
[pdf]      pdfium_library = "vendor/pdfium/lib/libpdfium.so"
[speech]   engine = "kokoro"   model_dir = "../models/kokoro-multi-lang-v1_0"   default_voice = "af_heart"   num_threads = 4
```

## 12. Frontend

Follows `rules/CLAUDE_FRONTEND_RULES.md`: data flows `api → hooks → page → components`, and events
flow back up.

```
frontend/src/
  api/          client.ts (fetch + error mapping), documents.ts, voices.ts, audio.ts  ← the only HTTP code
                mock/ (same function signatures, fake data). VITE_API_MODE=mock|http picks one.
  types/        Document, Block, Sentence, DocumentSummary, Voice, ApiError (mirror §5/§7)
  hooks/        useLibrary (recent docs, import), useDocument, useVoices, usePlayback (audio element,
                speed, prefetch next 2 sentences, skip non-spoken blocks, save position)
  pages/        LibraryPage (open PDF + recent documents), ReaderPage (option D)
  components/   ReadingView, Sentence, PageMarker, MiniPlayer, PlayerPopover, VoicePicker,
                SpeedPicker, DropZone, RecentDocumentList, StatusBanner
  utils/        pure helpers: timeLeft, formatPageLabel, groupVoices
  config/, styles/
```

- **Design: option D** from `design/2026-10-06-reader-options.html`: large text filling the screen; a
  centred floating pill (prev / play-pause / next + a short status); "more" opens a popover with
  progress, time left, speed (0.75–2×), voice, open, and **Export audio** (starts an export, shows
  its progress, then offers the download), with an MP3/M4B choice, and a **"Read code & tables
  aloud"** switch (off by default).
  Phone width: the pill spans the width. Light and dark follow the system setting.
- **Location aids in the view:** a small page marker in the margin wherever the page changes
  ("p. 37"), plus the current page in the player status ("p. 37 of 412"). Not read aloud.
- Keyboard: Space = play/pause, ← → = previous/next sentence.
- **Build order:** Fable builds the UI against the `mock` API now. Wiring it to the real backend later
  only means switching to `VITE_API_MODE=http`, because both implement the same `api/` functions.

## 13. Testing

- **domain:** unit tests for every cleaning rule using `TextRun` fixtures taken from the user's books
  (page list in the PDF survey), plus sentence-splitting cases ("e.g.", "Dr.", "Fig. 3", "U.S.", "3.14").
- **app:** services tested with in-memory fakes (fake engine, in-memory repositories).
- **infra:** a small integration test per adapter (a real Kokoro WAV, a real PDF page through pdfium).
- **api:** route tests with `tower::ServiceExt::oneshot`.
- **frontend:** `tsc --noEmit` + `vite build`, assertions for pure `utils/` modules, then your
  browser pass.

## 14. Risks

1. **The PDF cleanup is our own code** and won't be perfect on every book. Mitigation: rules based on the
   survey of your books, fixture tests, and `cleanerVersion` for re-processing.
2. **sherpa-onnx can crash the process** on bad config or input. Mitigation: §10 checks.
3. **Big books:** a synchronous import takes a while, and the full document JSON for 400 pages is a few
   MB. That should be fine for v1. Measure it with your books.
4. **Export takes time on CPU:** about 1 h 50 min for an 8-hour book. It runs in the background. GPU
   later would cut this a lot.
5. **Licenses before sharing the app:** espeak-ng-data may be GPL-3.0 (UNVERIFIED). MP3/M4B are made
   by the separate `ffmpeg` program, so we don't link LAME.
6. **Export needs `ffmpeg` installed.** It is on this laptop. On machines without it, export fails with
   `ExporterUnavailable` and the rest of the app still works.

## 15. Build milestones (the detailed plan comes after you approve this)

0. Workspace skeleton, config, logging, error registry.
1. **Voice test:** Kokoro adapter → a WAV of `af_heart` reading a paragraph from one of your books.
2. PDF: pdfium adapter + cleaner rules + fixture tests on your books.
3. Library: import, saved JSON, recent list.
4. API + speech worker + audio cache.
5. Frontend (**Fable starts now on mocks**) → switch to the real API.
6. Fixes from using it on your books.
