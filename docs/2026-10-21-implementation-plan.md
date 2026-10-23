# Lectrice: implementation plan (2026-10-21)

Built from `docs/2026-10-21-design-spec.md` (decisions approved by the user on 2026-10-21) and
`research/2026-10-21-pdf-survey.md`. Each step lists its files, tests, and "done when". Rules:
`rules/BACKEND_RULES.md` and `rules/CLAUDE_FRONTEND_RULES.md` (minus the FSG parts, see CLAUDE.md).
No git commits until the user asks.

**Who does what:** Opus agents build the backend steps, and the main agent reviews every diff against
the rules before the next step starts. The frontend is an Opus brief, then Fable builds it. Steps on
the same line can run in parallel.

```
B0 ─┬─ B1 (voice) ─────────────┐
    ├─ B2 (PDF → clean doc) ───┼─ B4 (API) ─ B5 (read-ahead/cache) ─ B6 (export) ─ W (wire) ─ T (try on books)
    └─ B3 (library storage) ───┘
F (frontend on mocks) runs alongside the B steps.
```

## B0. Workspace skeleton
- `backend/Cargo.toml` workspace (resolver 3, pinned `[workspace.dependencies]`), crates
  `lectrice-domain`, `lectrice-infra`, `lectrice-app`, `lectrice-api`.
- domain: `ErrorName` + `ErrorSpec` registry (spec §8), `AppError`, entity structs (spec §5), traits (spec §3).
- api: config loading (`config.toml` + `LECTRICE_*` env), JSON `tracing` setup, `GET /api/health`,
  `AppError → JSON` mapping, graceful shutdown.
- `vendor/pdfium/` with pdfium-binaries `chromium/7881` `libpdfium.so` + its LICENSE.
- **Done when:** `cargo build` and `cargo test` pass, `curl :7878/api/health` returns JSON, and a
  test proves every `ErrorName` has a unique code.

## B1. Voice: Kokoro adapter
- infra: `KokoroSpeechEngine` (sherpa-onnx 1.13.8): startup path checks, the name ↔ id table
  (ids 0–27 exposed), `\0` stripping, voice validation, `synthesize` → `Audio` (24 kHz f32);
  `WavEncoder` (hound, 16-bit).
- A small dev binary `lectrice-api --speak "text" --voice af_heart --out x.wav`, for listening.
- **Tests:** unknown voice → `VoiceUnknown`; missing model dir → engine reports failed (the process
  must not exit); a real generation gives a non-empty 24 kHz WAV (integration test, `#[ignore]` by default).
- **Done when:** the user listens to a WAV made by our Rust code and confirms it matches the sample.

## B2. PDF → cleaned document
- infra: `PdfiumTextSource`: per-char data → `TextRun { pdf_page, text, font_name, font_size, bbox }`,
  page labels, outline. Check survey UNVERIFIED items 9 and 10 first (scaled font size, hyphen marker)
  with a tiny test on K8s p9.
- domain (pure): the survey's 20-rule pipeline (§4) as small separate functions: profile, lines,
  header/footer bands, page labels, TOC/index/bibliography/copyright pages, footnotes and markers,
  code, captions, tables, notes, lists, headings (outline-matched), paragraphs (including across
  pages), de-hyphenation with a book vocabulary, NFKC, speech-only filters (citations, URLs, emoji).
  The output is blocks with `pageIndex`, cue sentences and `spoken` flags.
- infra: `SentencexSplitter` + our extra abbreviations (e.g., i.e., Dr., Fig., et al., Eq., U.S., decimals).
- **Tests:** each rule unit-tested on `TextRun` fixtures extracted from the survey's 35 (book, page)
  cases. Snapshot test: each of the 5 books cleans without error, and the counts (blocks, sentences,
  code blocks) are printed for review.
- **Done when:** the fixtures pass and the user reads a cleaned-text dump of ~3 chapters (one per book
  type) and says it's right.

## B3. Library storage
- infra: `FsDocumentRepository` (read + write traits): `library/<sha>/original.pdf`, `document.json`,
  `meta.json`, written as temp file + rename; `list_recent` paginated by `lastOpenedAt`.
- app: `ImportPdf`, `GetDocument`, `ListRecentDocuments`, `SaveReadingPosition`, with in-memory fakes
  for tests.
- **Tests:** importing twice → the second is `alreadyInLibrary` and doesn't re-run the cleaner (a fake
  counts calls); renamed file → same id; `cleanerVersion` bump → re-cleaned from the saved PDF.

## B4. HTTP API
- api: routes, DTOs and presenters for every endpoint in spec §7 except exports; multipart upload with
  a `upload.max_mb` limit; magic-byte check; `spawn_blocking` for import.
- app: `ListVoices`, `SpeakSentence` (cache get → worker → encode → put).
- The speech worker: a dedicated thread with a priority queue (now > read-ahead > export) and merging
  of identical keys.
- **Tests:** `oneshot` route tests for each endpoint, including the error shapes (`PdfNoText`,
  `DocumentNotFound`, `VoiceUnknown`, `UploadTooLarge`).

## B5. Read-ahead and the audio cache
- app: read-ahead planner (pure): from (sentence, rate, readSkipped) → the list of sentence ids
  covering `read_ahead_minutes × rate` of 1× audio (18 chars/s estimate); re-planning on a jump or a
  speed change cancels stale jobs.
- infra: `FsAudioCache` with eviction: keep `keep_minutes_per_document × rate` around the position
  (deleting from farthest behind first), plus the global `cache_max_mb` cap.
- **Tests:** planner at 1× and 2× (2× plans twice the audio); eviction order; a jump cancels queued jobs.

## B6. Whole-book export
- domain: `AudioExporter` trait. infra: `FfmpegExporter` (PCM → ffmpeg stdin; MP3 via libmp3lame
  64 kbps mono; M4B via aac + chapters metadata from headings). Missing ffmpeg → `ExporterUnavailable`.
- app: `StartAudioExport`, `GetAudioExport`, `CancelAudioExport`; export jobs at the lowest priority,
  reusing cached sentences.
- api: the export endpoints (spec §7).
- **Tests:** the export job state machine with a fake exporter; a real 1-minute MP3 and M4B (`#[ignore]`),
  with M4B chapters checked via `ffprobe`.

## F. Frontend (parallel, on mocks)
- Opus writes `docs/2026-10-21-frontend-brief.md`, then Fable builds `frontend/` (Vite + React + TS,
  option D, recent documents, page markers, player popover with speed, voice, the read-aloud switch and
  export) against `VITE_API_MODE=mock`.
- **Done when:** `npm run build` passes, the brief's acceptance checklist is ticked, and the user
  clicks through it in mock mode.

## W. Wire up
- Switch to `VITE_API_MODE=http`; the Vite proxy sends `/api` to `:7878`. Fix any contract gaps (a
  shape change is a full loop: Rust DTO + presenter + TS type + api function together).

## T. Try it on the user's books
- Open all 5 books, read a chapter of each, export one chapter-sized test as MP3 and M4B. Collect
  cleaner issues, fix them, and raise `cleanerVersion`.

## Later (not v1)
GPU (CUDA archive), better code/table detection, OCR for image-only pages, mobile (`lectrice-ffi`),
splitting GoF's missing spaces, two-column papers in the fixtures.
