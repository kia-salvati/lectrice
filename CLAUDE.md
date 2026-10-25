# Lectrice

> **New session? Read `docs/HANDOFF.md` first** (current state, rules that are easy to break,
> next steps). This file is the full decision log.

A free, fully local Speechify-like app. You load a PDF, it cleans the text into a "reading view"
(no headers, footers, page numbers, footnotes, citation markers or references) and reads it aloud
in a **realistic woman's voice**. The voice must be downloadable and usable by anyone.

## Machine
This laptop does everything: code, model downloads and running the app.
RTX 3080 Ti Laptop (16 GB VRAM), ~100 GB free disk, Linux. Project started on another machine
on 2026-10-05 and moved here.

## Layout
- `research/`: findings. **Read `research/2026-10-05-findings.md` first.**
- `research/2026-10-06-rust-stack.md`: Rust crates, versions and risks.
- `design/`: UI mockups (local HTML).
- `models/`: Kokoro model bundle.
- `voices/`: empty; only for clips if a cloning model is added later.
- `rules/`: coding and workflow rules (see "Coding rules").
- `app/`: empty placeholder; code will go in separate `backend/` and `frontend/` projects.

## Coding rules
Follow `rules/` (copied from the user's shared rules on 2026-10-06):
- `rules/BACKEND_RULES.md` for the core and server, `rules/CLAUDE_FRONTEND_RULES.md` for the web
  page, `rules/HOW_WE_WORK.md` for the workflow.
- **Ignore the FSG-project parts:** BACKEND §9 and the DB/SQL/auth/password rules (no database
  here); FRONTEND's FSG names, antd specifics and §9 codebase-memory; HOW_WE_WORK's project
  boundaries, FSG repo names, codebase-memory MCP and the `fsg_internal_frontend` harness paths.
- Apply the universal parts in the core's language: layered clean architecture (dependencies
  point inward), SOLID, DI, one error registry, structured logging at call boundaries, no magic
  values, self-review of the diff.

## Decisions so far
- **Backend and frontend are separate projects** (`backend/`, `frontend/`), each built on its own.
- **Voice model: Kokoro 82M** (Apache 2.0, ONNX:
  https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX). The user tested it on Hugging Face
  and liked it (2026-10-06). It has preset voices (`af_heart`, `af_bella`…), no cloning. It runs on
  CPU and phones (sherpa-onnx has Android/iOS support).
- **The voice model is a swappable plug-in.** The app only knows a `SpeechEngine` interface
  (text + voice id + speed → PCM samples + sample rate; list voices). Kokoro is one adapter in
  `infrastructure/`; a factory picks the adapter from config. Nothing outside the adapter
  mentions Kokoro, sherpa-onnx, phonemes or model files. Audio encoding (WAV/MP3) is a separate
  concern, not part of the engine. A remote adapter (calls a TTS server over HTTP) is a possible
  later addition so models in any language can be plugged in.
- Non-commercial models (Breeze TTS 2, Fish S2 Pro, Voxtral, Higgs) are out: their licenses
  clash with "usable by anyone".
- **Process each PDF once (user request 2026-10-06).** On first open: extract + clean, then save
  the cleaned document (headings, paragraphs, sentences with ids) plus a copy of the original
  PDF in a library on disk, keyed by the SHA-256 of the PDF bytes (same file renamed = cache
  hit). Re-opening loads the saved version and doesn't re-process. Store a `cleaner_version`;
  bumping it lets old documents be re-processed when the rules improve. Behind a
  `DocumentRepository` trait. **Confirmed 2026-10-21:** the cleaned doc is saved as JSON in the
  app's own format, not a new PDF. Generated sentence audio is cached too, keyed by doc +
  sentence + voice + engine, made at 1× speed with the speed change applied in the player.
- **Keep location aids (user 2026-10-21):** page numbers and other things that help the reader
  find their place in the original PDF stay in the cleaned document as metadata (each block knows
  its PDF page and printed page label) and are shown in the reading view, but are **not spoken**.
  Chapter/section headings stay too.
- Default voice: **`af_heart`** (confirmed 2026-10-21).
- **Logging (user 2026-10-21): log only the important parts.** Never log book content (PDF
  bytes, text, sentences, documents, audio); log ids, counts, durations, writes and errors.
- **Reading position save: approved.** Recent documents show progress + "continue".
- **Audio (user 2026-10-21):** generate ahead of the listener (default 10 min), keep ~1 hour per
  book in the cache, and **export the whole book as one audio file** in the background. Measured:
  Kokoro on CPU (8 threads) ≈ 4.4× real time; 1 h of speech ≈ 14 min; an 8 h book ≈ 1 h 50 min.
  Sample: `research/2026-10-21-af_heart-sample.wav`.
  **Read-ahead and the cache window are in listening time, so they scale with playback speed**
  (10 min at 2× = 20 min of 1× audio). Export formats: **MP3 and M4B** (via system `ffmpeg`).
- **Skipped content (user 2026-10-21):** code is shown, not read; the cue "Code example." is
  spoken instead. Captions are shown, not read. Tables are shown with the cue "Table." (my
  default; user didn't object). GoF missing spaces left as is (my default). A "Read code & tables
  aloud" switch is in v1 since detection is needed anyway; better detection is phase 2.
- PDF survey done: `research/2026-10-21-pdf-survey.md` (20-rule pipeline, 35 fixtures, open
  questions in §6).
- Open-PDF screen shows **recent documents** (yes). **No "show what was removed" toggle** for now.
- Test PDFs: the user's real books in `~/Desktop/*.pdf` (Clean Architecture, Designing
  Distributed Systems, GoF Design Patterns, LinkedIn Posts 2024, The Kubernetes Book). All have a
  text layer. They are technical books: expect code listings, figures, tables, TOC and index.
- Cloning models (Qwen3-TTS Base, Chatterbox Turbo ONNX, both permissive) are a possible later
  add-on if the user wants custom voices. Don't build for them yet.
- **Architecture: two parts.** A UI-free **core** (PDF → clean text → sentences → TTS → audio)
  behind interfaces, and thin shells on top: a localhost HTTP server + web page now, mobile
  bindings later. The core must not depend on the web layer.
- **Languages (decided 2026-10-06):** backend/core in **Rust** (the official `sherpa-onnx`
  crate; `sherpa-rs` was archived 2026-06-06; `axum` for HTTP, later `uniffi` for mobile); frontend in **TypeScript + React + Vite**, no antd.
- Model files: `models/kokoro-multi-lang-v1_0/` (sherpa-onnx bundle: `model.onnx`, `voices.bin`
  with 54 voices, `tokens.txt`, `espeak-ng-data/`, lexicons). Default voice `af_heart`
  (confirmed).
- Machine state 2026-10-21: g++ 16.2, Node 22.23 and npm 10.9 installed; Rust 1.99.1 stable via
  rustup (`~/.cargo/env` is sourced from `~/.zshenv`). The NVIDIA driver now
  works (615.71, RTX 3080 Ti Laptop, 16 GB). Still build CPU first; GPU is an optional later step.
- Stack details and risks: `research/2026-10-06-rust-stack.md`.

## Agents
The user wants work dispatched to agents and orchestrated: **Opus for everything except design
code; Fable only writes design code (UI mockups, HTML/CSS/layout, UI components). No other models.**
**Design split (user 2026-10-23):** Opus does all the research, ideas, UI/UX thinking and the
design options, and writes exact instructions for what to build and how (a build brief). Fable
only turns that brief into code. Fable doesn't research, invent options or make UX decisions.
Design options are local `.html` files under `design/`, never published Artifacts.

## Where we are (2026-10-21)
Process: brainstorm, then a written design spec, then an implementation plan, then build.
**Backend code waits for the user's approval of the spec.** The user OK'd writing the spec and the
plan, and said **Fable may start designing and building the frontend now against mock data**
("we later wire things up").

2026-10-21:
- Design spec `docs/2026-10-21-design-spec.md`: **decisions approved by the user** ("i'm good
  with the decisions").
- Implementation plan: `docs/2026-10-21-implementation-plan.md` (steps B0–B6, F, W, T).
- Frontend build brief (Opus) done: `docs/2026-10-21-frontend-brief.md` (~37 verified files for
  Fable to copy; hash router, no extra deps, `content-visibility`, Oxlint, node --test).
- Fable is building `frontend/` from the brief against the mock API (started 2026-10-21);
  screenshots go to `design/screens/`.
- **2026-10-21: the user approved building the entire project** (B0 → T) and asked the main
  session to only orchestrate agents (no hands-on work, to keep the session context small), then
  report back when finished. Each step's agent writes `docs/progress/<step>.md`.
- **2026-10-23: v1 is built end to end** (B0–B6, F, review + fixes, W, Fable review, T). Reports:
  `docs/progress/*.md`; remaining problems in `T.md`. Cleaner version 2.
- **Git (2026-10-23):** repo at `lectrice/`, author `kia <limsclaude@gmail.com>` (repo-local
  config). v1 committed in 4 commits on `release/v1`, tagged `v1.0.0`. New work goes on
  `feature/<name>` branches merged into **`main`** (the development line, created 2026-10-23 from
  `release/v1`; `release/v1` stays as released). Chapters merged and pushed to `main` on 2026-10-23.
  **Public repo shows only `main` and version tags (user 2026-10-23):** GitHub's default branch is
  `main`; feature and release branches stay **local only** (never push them); merge into `main`
  locally, push `main`, and mark versions with tags (`git push origin main vX.Y.Z`) + a GitHub
  Release. Same rule for `lectrice-android`. **Remote:** `origin` =
  `personal:kia-salvati/lectrice.git` (public; SSH alias `personal` in `~/.ssh/config`, key
  `~/.ssh/github`). `release/v1` + tag `v1.0.0` pushed 2026-10-23 (history rebuilt without book
  text first). License: **MIT OR Apache-2.0**, with a README note that binaries built with
  sherpa-onnx 1.13.x include GPL-3 espeak-ng. **The repo will be PUBLIC (user 2026-10-23): no text from the user's
  copyrighted books may be tracked in git** (samples, fixtures, prose quotes in docs). Book-derived
  test data lives in a gitignored local folder and is regenerated from `~/Desktop/*.pdf`; tests
  that need it skip when it's missing. History must be cleaned before the first push.
- **Android app (2026-10-23):** research started → `research/2026-10-23-android-options.md`.
  The user wants it "clean and awesome", time is OK, and likes building it **the way Telegram
  Android is built** (to be checked against alternatives: Compose, classic Views, Flutter, RN,
  KMP, Tauri). Reuse the Rust core via `lectrice-ffi`.
  **Decided 2026-10-23: Kotlin + Jetpack Compose, applying Telegram's *principles* (§4.1 of the
  report), never copying Telegram code** (it's GPL). Test phones: Galaxy S10+ and Galaxy S26
  Ultra (others available). **No server option for the phone** (user 2026-10-23): everything
  on device. **The Android app is a separate project/repo** (user 2026-10-23). **Sync confirmed
  2026-10-23: one shared Rust core.** The Android repo depends on `lectrice-domain`, `-app`,
  `-infra` from this repo via Cargo git dependencies pinned to release tags (local path override
  for side-by-side dev); platform adapters live in each repo; the core gets feature flags for
  desktop-only parts, a CHANGELOG and semver tags. Never fork the core.
- **Android repo (2026-10-23):** `lectrice-android` (Android only; iOS would be a separate
  `lectrice-ios`). Local repo at `../lectrice-android` (branch `main`, remote
  `personal:kia-salvati/lectrice-android.git`, on GitHub, public), with README, licenses,
  .gitignore and its own CLAUDE.md. First commit `4ac4ca6` pushed to `main` 2026-10-23. Releases: signed APKs on GitHub
  Releases via Actions later.
- **Android answers (user 2026-10-23):** the user picks which model to download (fp32 or int8);
  try sherpa-onnx 2.0 (no GPL); "prepare while charging" only if needed; GitHub Releases only;
  lock-screen next = next sentence; **don't install the Android tools yet (metered network)**:
  give copy-paste download commands for later. Open question: GPU/NPU acceleration on the phone
  (being researched for the spike).
- **Android design approved (user 2026-10-23): option 3 "Quiet" v2**,
  `design/2026-10-23-android-quiet-v2.html` (brief `docs/2026-10-23-android-quiet-v2-brief.md`):
  option 1's library with Quiet's Open PDF button, tapping the pill opens option 4's Listen
  stage, option 4's export with voice / model quality / speed choices, the current sentence on the
  lock screen, and no visible scrollbars anywhere (scrolling still works). Answers: **export is
  M4B only** (no MP3, because of the license; the mockup's MP3 choice is dropped); **Listen view
  over the lock screen** is a setting, off by default; chapters access: best practice (Opus decides).
  Feature logic spec done 2026-10-24: `docs/android-logic/` (README index + 19 feature files;
  file 19 = new core/FFI changes C10–C27). Chapters: ⋯ → Chapters (2 taps), 1 tap from Listen.
  **User 2026-10-24:** "Remove from library" is in v1; all file-19 core changes go into
  **v1.1.0**; voice sample = one fixed default sentence; export predicts the file size first
  and, if space is short, blocks the full export or offers a partial one; the cache and the
  exported files must show in Android's own Settings → Apps → Lectrice → Storage. "Save to
  Audiobooks" **moves** the export to `Audiobooks/Lectrice/` (not a copy).
- **Versions are beta 0.x (user 2026-10-24)** in every repo. Tags retagged and pushed:
  `v1.0.0` → `v0.1.0`, `v1.1.0` → `v0.2.0` (old tags deleted on GitHub). The next core release
  (was "1.2.0") is **v0.3.0**; the desktop starts at v0.1.0, the Android app at v0.1.0. Done 2026-10-25:
  workspace version `0.3.0`, CHANGELOG headings 0.x, the Android/desktop core pins `tag = "v0.2.0"`
  (`docs/progress/versions-0x.md`).
- **Desktop app (user 2026-10-24): Tauri 2, Windows + Linux only for now, its own repo**
  `lectrice-desktop` (local `../lectrice-desktop`; same rules as the other repos: push only `main`
  + tags, public, no book text). One app for both systems: the shared core stays OS-agnostic and
  the composition root picks per-OS adapters/tools (clean architecture). Reuses the core via Cargo
  git deps pinned to tags (path override for local dev) and the web UI from `lectrice/frontend`.
  Export: M4B through the `AudioExporter` port (system ffmpeg on Linux, a bundled,
  checksum-verified LGPL ffmpeg on Windows). The user asked the main session to only manage
  agents and keep going until it's built. The user has a Windows PC for testing. GitHub repo
  exists: `git@github.com:kia-salvati/lectrice-desktop.git` (use the `personal:` SSH alias:
  `personal:kia-salvati/lectrice-desktop.git`). 2026-10-24: the user installed the Tauri/WebKitGTK
  and mingw/clang/NSIS packages and **accepted Microsoft's license for the xwin CRT/SDK
  download**. **Desktop designs picked (user 2026-10-24): the recommended ones**: UX1 A (one
  calm first-run card), UX2 C (settings dialog), UX3 C (⋯ panel export + a "ready" card on any
  screen), UX4 A (About tab in the settings dialog); "Check again" for missing ffmpeg (my default).
  File: `lectrice-desktop/design/2026-10-24-desktop-ux-options.html`. Small gaps (user
  2026-10-25, recommended): S1 A ("Import… ▾" menu: file or unpacked folder), S2 B (error title
  toasts), S3 A (toast centred above the pill while the chapters rail is open). **Designs: give the user the options and let
  them choose**; everything else goes ahead without waiting; ask only when truly needed.
- **Desktop tested by the user on Linux (2026-10-25): "works really well".** Two bugs reported:
  a constant "zzzz" buzz under the voice (heard over RustDesk; being checked digitally in
  D9) and the import-success screen jumbled for a split second (D9). **The user OK'd releasing
  this version** ("0.2 or something"): desktop **v0.1.0** (its first release) with core
  **v0.3.0** (core v0.2.0 is already taken), as a beta/pre-release, after the D9 fixes and the
  Windows check.
- **Released 2026-10-25 (user OK):** core `v0.3.0` and desktop `v0.1.0` (GitHub pre-releases with
  `.rpm`, `.deb`, Windows `.exe`, FFmpeg LGPL source + scripts). `gh` 2.103.2 is in `~/.local/bin`
  (logged in as kia-salvati).
- **Voice models (user 2026-10-25): three choices, the user picks any:** two official sherpa-onnx
  bundles (fp32 "Full quality", int8 "Smaller") and **ours: "Small" (`kokoro-lectrice-w8`, vocoder
  int8 weights + fp16 elsewhere, fp32 maths; sounds like fp32 at fp32 speed)**. Models are hosted on
  **GitHub, not inside the APK**; ours is released in the main repo under tag
  `model-kokoro-lectrice-small-v1` (asset `kokoro-lectrice-w8-multi-lang-v1_0.tar.bz2`, sha256
  `57595e9e…b1e2`). The user heard no difference between fp32, small and fast. Next: add it to the
  desktop/Android model manifests with its own engine id; "fast" (q8) still needs phone tests.
- **Phone variants test (2026-10-25, `lectrice-android/docs/progress/variants-cores.md`):** our
  "Fast" (q8) model: S10+ sustained 1.11 (fp32 1.76), S26 0.38 at 8 threads (fp32 0.58–0.75),
  34–41 % less battery; the user's ear test heard no difference. "Big cores only" made no
  difference (keep all cores; tune threads per phone class). **Decided (user 2026-10-25): the apps
  offer Fast (ours, default, ~111 MB), Full quality (official fp32, ~326 MB) and Small (ours,
  ~111 MB); the official int8 is dropped.** Fast still needs its own GitHub model release.
- **Android GO accepted (user 2026-10-25):** build the real Android app from `docs/android-logic/`
  (live reading on S26-class phones, Prepare-first on S10+-class phones).
- **LiteRT (user 2026-10-25):** in the ear test sherpa 1.13.8 sounded better than Google's LiteRT
  Kokoro in most clips; LiteRT CPU and GPU sound identical (only speed differs). Not tested on a
  phone yet; desktop LiteRT ≈ 2–4 weeks of work for speed only. Undecided.
- **NPU acceleration (user 2026-10-25): wanted as a toggle feature** in the Android app. NPU and
  CPU clips both "sounded really good" to the user. Available (and on by default) only on supported
  Snapdragon chips; elsewhere the toggle is disabled with a header saying it's only available on
  certain Snapdragon chips. Blocked on a proper license review of Qualcomm's QNN runtime (revocable
  terms, indemnity, export, GPL-3 espeak-ng in the same app); options include an optional
  "NPU pack" the phone downloads itself instead of shipping it in the APK.
  **License review done** (`research/2026-10-25-qnn-license-review.md`): no QNN in v0.x; no
  NPU-pack download of Qualcomm's libs; later only as an opt-in "NPU edition" APK with the NPU
  vocoder in its own process. Spec `docs/android-logic/22-npu-acceleration.md` (blocked on
  licensing + the user's eligibility answer + Qualcomm/lawyer answers). **Design picked (user
  2026-10-25): option B** (Performance section with the "only on certain Snapdragon chips" header
  + an NPU page) and **yes to the three brief §9 questions** (show the normal-edition row only once
  an NPU edition exists; keep the releases link; "Turn on NPU" as the primary button). The NPU
  vocoder files (chip-specific compiled contexts, ~830 MB / 1.25 GB with long buckets) are ours,
  downloaded on first use or compiled on the phone while charging. **User 2026-10-25: no public
  NPU release for now, but keep the work; make a personal (local, unpublished) NPU build for the
  user's S26 Ultra.**
- **Downloads (user 2026-10-24):** Google blocks `dl.google.com/android/repository/` and
  `developer.android.com` from this network. If an official download fails, **use a mirror,
  as long as the files can be checked against a checksum** (the Android SDK came from
  `https://mirrors.cloud.tencent.com/AndroidSDK/` via `SDK_TEST_BASE_URL`, sha1 checked).
  cmdline-tools 23 ("latest") needs the blocked Android CLI, so keep build 13114758 in
  `cmdline-tools/latest`. Installed: JDK 21 (`~/.local/opt/jdk-21`), SDK in `~/Android/Sdk`
  (platform-tools 37.0.1, platforms 36 + 37.0, build-tools 36.1.0 + 37.0.0, NDK 29.0.14206865).
  Google Maven (`maven.google.com`) and `repo.maven.apache.org` are blocked too: the Android
  build uses `https://maven.aliyun.com/repository/google` (set in the gitignored
  `local.properties`, PGP-verified; 63 unsigned files pinned by SHA-256 on first use) and
  `repo1.maven.org` for Maven Central. Spike built 2026-10-24 (`lectrice-android`, S1/S3/S4).
- **Chapters build (2026-10-23): option 1A** = rail take 1 (outline + filter) + label A (in the
  pill). My defaults: `C` = simple toggle; current chapter = nearest chapter-like title with the
  book's own number. Fable is building it on `feature/chapters`.
- **Chapters UI decided 2026-10-23:** option 3 (bottom sheet) combined with option 4 (rail), with
  the rail reworked into a whole-book overview with easier navigation, closable and reopenable.
  Fable is making `design/2026-10-23-chapter-options-v2.html`. Label style (A/B/C) and the
  "current chapter = nearest chapter-like title, book's own number" rule are not answered yet.
- **Next feature (2026-10-23):** show the current chapter beside the page number, and a chapters
  panel (chapters + sub-chapters, click to jump). Flow: Opus backend `toc` + brief → Fable UI
  options HTML → user picks → Fable builds → review → commit when the user approves.
  Waiting on the user: test the app, read the regenerated `docs/progress/B2-samples/`, listen to
  `T-k8s-chapter.m4b`, and decide on reading short domain names aloud (T changed it to "read").
  Earlier: **B1 voice approved by the user** ("it sounds awesome").
  The user often works remotely via RustDesk; remote audio works, so just give `xdg-open` paths.

Done earlier:
- Rust stack research: `research/2026-10-06-rust-stack.md` (sherpa-onnx 1.13.8, pdfium-render
  0.9.4, sentencex, axum 0.8, thiserror + tracing, workspace crates domain/infra/app/api).
- UI design options (Fable): `design/2026-10-06-reader-options.html`. **The user chose option D**
  (large text filling the screen + a small floating pill player: prev/play/next + short status;
  "more" opens a popover with progress, time left, speed, voice, export, open). The user called
  it "perfect".
- All earlier open questions answered on 2026-10-21 (see Decisions). All toolchains installed.

## How the user likes to work
- The user is a backend developer learning frontend. Give short, right-sized explanations of
  frontend concepts when they're used; don't repeat them.
- Minimal comments: comment only non-obvious intent and prefer self-explanatory code.
- Make code changes as explicit edits the user can review, not sed/python rewrite scripts.
- Don't run `git commit` until asked. When asked for commits, propose groups and messages first.
  No "Co-Authored-By: Claude" trailer.
- No published Artifacts. Deliver reports as local HTML/Markdown files.
- Keep the user's language simple and clear; they write informally, so confirm intent if
  something is ambiguous.
- Before finishing, re-read the diff and remove unused or speculative code.
