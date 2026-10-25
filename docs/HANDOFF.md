# Lectrice: session handoff (2026-10-25)

Read this first, then `CLAUDE.md` (the full decision log and rules). The previous session ran
out of room; everything needed to continue is here.

## Repos
| Repo | Local | State |
|---|---|---|
| **lectrice** (core + web app) | `~/Desktop/learning/lectrice` → github.com/kia-salvati/lectrice | `main` pushed; tags `v0.1.0`, `v0.2.0`. **Uncommitted work on local `feature/desktop-support`** = next core **v0.3.0** |
| **lectrice-desktop** (Tauri 2) | `../lectrice-desktop` → github.com/kia-salvati/lectrice-desktop (empty on GitHub) | Built and reviewed, **nothing committed yet**; `origin` = `personal:kia-salvati/lectrice-desktop.git` |
| **lectrice-android** | `../lectrice-android` → github.com/kia-salvati/lectrice-android | `main` pushed (spike + `lectrice-ffi`); S7 work (bundled models, `run-phone.sh`, results, the purge one-liner in `rust/lectrice-ffi/src/api.rs`) **uncommitted** |
| sherpa2 worktree | `../lectrice-sherpa2` (branch `feature/android-spike-sherpa2`) | Spike only; sherpa 2.0 rejected; never push |

Versions are beta 0.x everywhere. Android + desktop pin the core at `tag = "v0.2.0"` and build
locally through `.cargo/config.toml` path overrides (they already use v0.3.0-only APIs such as
`purge_removed_at_start`, so move the pins to `v0.3.0` right after tagging it).

## Right now
- **Agent D9 was running when the session ended:** fixing the two bugs the user found on Linux.
  Its report lands in `lectrice-desktop/docs/progress/D9-fixes.md`. If that file is missing or
  incomplete, re-run the task:
  1. **"zzzz" buzz under the voice** (user listens via RustDesk; the web app sounded clean over
     RustDesk earlier). Capture the desktop app's output digitally (`pw-record`/sink monitor) and
     compare with the core's WAV for the same sentence; web app in a browser as control. Fix it
     if it's in the app (WebKitGTK/GStreamer path), else report "RustDesk".
  2. **Import-success screen jumbled for a split second** (likely the lazily loaded desktop
     chunk/CSS applying after first paint). Reproduce, fix.
  **Update 2026-10-25: the user hears no buzz on their own PC → the buzz is RustDesk.** D9 only
  confirms the desktop output is clean digitally; bug 2 is the real fix.
- **W3 bug (2026-10-25):** on the user's Windows PC the installer says "Completed" but installs
  nothing (no `%LOCALAPPDATA%\Lectrice`, the details box shows only "Completed", the shortcut is
  broken). **Fixed** (`lectrice-desktop/docs/progress/W2-fix.md`): Fedora's `makensis`
  (NSIS_MAX_STRLEN 8192) was mixed with the official zip's stubs (1024), so the Install section
  read as empty; `native/fetch.sh` now uses Fedora's matching mingw32-nsis stubs.
- **D9 done** (`docs/progress/D9-fixes.md`): the buzz is RustDesk (app output digitally clean);
  the first-run "voice is ready" → "Open a PDF" path had two wrong frames, fixed in
  `LibraryPage.tsx`, `useLibrary.ts`, `SampleCard.tsx`. Rebuilt: `.rpm` sha256 `831b7121…`,
  Windows installer sha256 **`04c73a0b…17c9fb`** (includes the NSIS fix + D9). The user retests W3
  with this one and reinstalls the rpm (`sudo dnf reinstall ./target/release/bundle/rpm/Lectrice-0.1.0-1.x86_64.rpm`).
- **Import flash (D10, `lectrice-desktop/docs/progress/D10-import-flash.md`):** not an app bug.
  It's KWin's window-close "Scale" animation of the GTK file dialog fading over the page while the
  page changes screens (proved: no mixed frames with KWin's effects off). The display runs on the
  AMD iGPU, not NVIDIA. No code changed. **User picked (2026-10-25): the small timing change**
  (keep the screen still until the dialog has gone, ~250 ms after focus returns, then
  "Reading…"); Opus brief → Fable builds → rebuild both installers.
- **W3 done (user 2026-10-25): the Windows app "was perfect"**, with the same dialog flash as Linux.
- **D11 done** (`lectrice-desktop/docs/progress/D11-dialog-settle.md`): the screen stays still until
  250 ms after focus returns (1 s cap) on Open PDF…; 46 frontend tests. Rebuilt: `.rpm` sha256
  `0c7b8fab…`, Windows installer sha256 `dc395678…` (in `release-for-windows/`). **The AppImage step
  failed (`failed to run linuxdeploy`)**: fix before the release (likely `patchelf` / the cached
  linuxdeploy in `~/.cache/tauri/`, see D7).
- **Waiting on the user:** the Windows PC test (W3): copy
  `lectrice-desktop/target/x86_64-pc-windows-msvc/release-for-windows/` (installer, invented
  `lighthouse-test.pdf`, `SHA256SUMS`, `2026-10-25-windows-test-checklist.md`) to the PC and follow
  the checklist; send back results + `%LOCALAPPDATA%\app.lectrice.desktop\logs\` newest log.
- **Release (user OK'd 2026-10-25):** desktop **v0.1.0** + core **v0.3.0**, beta/pre-release.
  Order: D9 fixes → W3 results (fix what it finds) → book-text scan → **propose commit groups for
  all three repos** (user approves) → merge `feature/desktop-support` into `main`, tag + push core
  `v0.3.0` → move the Android/desktop pins to `v0.3.0` → push desktop `main` → W4 CI (GitHub Actions
  build + release workflows; Windows core-test job) → W5 GitHub Releases with the `.rpm`, `.deb`,
  AppImage and Windows installer (+ FFmpeg LGPL source/build scripts, license notes, SmartScreen
  note). Every push/tag needs the user's OK.

## What exists
- **Web app + core** (v0.2.0 on GitHub; v0.3.0 on the branch): PDF → cleaned reading view → Kokoro
  `af_heart`, highlight, speed, 28 voices, chapters, read-ahead, cache, export (resumable parts,
  baked-in speed via Sonic, partial ranges), remove/restore books, voice samples. v0.3.0 adds the
  frontend `platform` module (web/desktop), media keys, `importPdf` sending bytes (WebKitGTK
  crashes on File bodies), Windows compile fixes, purge-at-start.
- **Desktop** (`lectrice-desktop`, steps D0–D8, W1, W2 done; reports in `docs/progress/`):
  in-app `lectrice://` protocol to the axum router (no network port); model download/import/switch
  (fp32 349.9 MB, int8 132.3 MB, SHA-256 pinned, Range resume); the user-picked screens (UX1 A,
  UX2 C, UX3 C, UX4 A, S1 A, S2 B, S3 A, "Check again"); export with Save as (moves) / Show in
  folder; own minimal LGPL ffmpeg (Linux test build 2.6 MB, `ffmpeg.exe` 2.3 MB, reproducible);
  `.rpm`/`.deb` 23 MB, AppImage 129 MB, Windows NSIS installer 13 MB (cross-built here).
  Build: `scripts/build-release.sh` (Linux), `scripts/build-windows.sh` (`LECTRICE_MAIN_DIR=../lectrice`).
  Dev run: `LECTRICE_MAIN_DIR=../lectrice LECTRICE_DEV_MODEL_DIR=../lectrice/models/kokoro-multi-lang-v1_0 cargo tauri dev`.
  Install: `sudo dnf install ./target/release/bundle/rpm/Lectrice-0.1.0-1.x86_64.rpm`.
  Known later items: quota-full → `StorageFull` in the core's `lectrice-infra/src/storage/mod.rs`;
  saved exports are 0600; placeholder app icon (`src-tauri/icons/icon.svg`); the "Open book" button
  on a stopped-export toast opens the export section (user may want only the reader).
- **Android spike** (`lectrice-android/docs/progress/S1–S7.md`): S10+ (Exynos 9820) is **red**
  (best RTF 1.22–1.58 unplugged, limit 0.9) → older phones need Prepare-first; Samsung throttles
  background work 4–8× (generation must run in a foreground service); sherpa 2.0 rejected
  (`research/2026-10-24-sherpa2-oov.md`). **S26 Ultra test started 2026-10-25** (SM-S948B, SM8850,
  Android 17, wireless adb `192.168.70.154`, paired; port via `adb mdns services`; Auto Blocker had
  to be turned off) → `lectrice-android/docs/progress/S5.md` + decision page
  `research/2026-10-25-android-m1-results.md`. **Paused 14:18: wireless debugging turned itself
  off** (asked the user to turn it on, plug a charger, keep the app in front). Early: fp32 RTF
  0.47–0.63 (green), import ~0.5 s, engine 0.7–1.5 s, PSS 0.8 GB; int8 slower; battery 53→44 % in
  20 min, 29→43 °C. Resume the S5 agent's remaining steps (pull `SM-S948B-20261025-102844.json`,
  finish bench/accelerators/play, soak unplugged, write S5.md + the decision page).
  **S5 done: GO** (decision page `research/2026-10-25-android-m1-results.md`, waits for the user's
  OK): S26 sustained RTF 0.583 (green, just) but heat throttles 2× after ~85 s at 8 threads;
  15.9 mAh/audio-min; older phones get Prepare-first. **Kokoro variants** (`research/2026-10-25-kokoro-variants.md`):
  our own "small" (w8, 111 MB, fp32 quality+speed) and "fast" (q8, 19 % faster) bundles in
  `models/kokoro-lectrice-{w8,q8}-…`, to test on phones. **NPU prep** (`research/2026-10-25-android-npu-test.md`,
  `lectrice-android/docs/progress/NPU-prep.md`): v81 contexts compiled (B64–B320), `npu-test` app
  not compiled yet (repo1.maven.org timing out; one kotlin-stdlib SHA-256 pin needs the user's OK);
  QNN license allows in-app shipping but is revocable etc. → local test only for now.
  **LiteRT laptop test** running → `research/2026-10-25-litert-desktop-test.md`.
  **NPU/GPU research done** → `research/2026-10-25-android-npu-gpu.md`. Key: Google's
  `litert-community/Kokoro-82M` (Apache-2.0, LiteRT, vocoder on GPU) claims RTF 0.12 GPU / 0.26
  CPU on the S26's chip (ours 0.47–0.63). Plan: (1) CPU fixes: big-core affinity, auto threads per
  SoC, foreground service, fp32 only (int8 file is badly quantized); (2) test LiteRT `omni_runner`
  on S26 + S10+ (≈600 MB models + Bazel build, ≥1 GB deps); (3) a LiteRT `SpeechEngine` adapter if
  it wins (must pass the ≤0.1 % skipped-words check); (4) QNN NPU vocoder only if battery is bad
  (proprietary `qnn-runtime` license review needed). Drop Exynos NPU/NNAPI/XNNPACK. Asked the user
  which steps to do. (Old note: the S26 Ultra waited until the user
  says so**: `spike/run-phone.sh <serial> <large.pdf> <medium.pdf> 2,4,6,8`. Wireless adb to the
  S10+ is paired (`192.168.70.129`, port changes; `adb mdns services`). Then S8 decision report and
  Android M2+ from `docs/android-logic/`.

## Rules that are easy to break
- **No text from the user's books** (`~/Desktop/*.pdf`) in any tracked file. Scan before every
  commit (9-word overlap check of changed files against `pdftotext` output of the books).
- **Push only `main` and version tags.** Feature branches stay local.
- **Commits only when asked; propose groups first; no Co-Authored-By trailer.** Author
  `kia <limsclaude@gmail.com>`. Remote alias `personal:`.
- **The main session only orchestrates agents.** Opus: research, specs, code, reviews, all UX
  thinking. Fable: design code only, from an Opus brief. **Designs: the user picks from options.**
  Everything else proceeds without waiting; ask only when truly needed.
- **Downloads:** Google's Android/Maven hosts and SourceForge are blocked here; use a mirror only
  when files can be checksum-verified. Never put personal data (e.g. the email) in request headers.
- No visible scrollbars in the Android and desktop designs.
- The user works via RustDesk; give `xdg-open` paths (no `#fragment` in `xdg-open`: use
  `google-chrome "file:///…#…"` for deep links).
- **Disk is tight** (~10 GB free; big `target/` dirs). `cargo clean` unused build dirs when needed.

## Key documents
- Web/backend contract: `docs/2026-10-21-design-spec.md`; CHANGELOG; build history `docs/progress/*.md`.
- Desktop: `lectrice-desktop/docs/2026-10-24-desktop-{spec,plan,ux-brief,model-contract}.md`,
  `docs/2026-10-25-desktop-ux-small-brief.md`, `docs/2026-10-25-windows-test-checklist.md`, `docs/progress/`.
- Android: `docs/android-logic/README.md`, `docs/2026-10-23-android-spec.md`,
  `docs/2026-10-23-android-m1-spike-plan.md`, `lectrice-android/docs/progress/`.
- Cleaner limits: `docs/progress/T.md`.
