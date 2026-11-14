# Lectrice: session handoff (updated 2026-11-14)

Read this first, then `CLAUDE.md` (the full decision log and rules). The sections below
"Repos" are older snapshots kept for history.

## Current state (2026-11-14): released
- **Released as GitHub pre-releases (2026-11-14), after the final check, the fixes and the verification review:**
  - **Core v0.4.0** (main repo, tag `v0.4.0`): audiobook map, Prepare jobs, the computer helper and the new crate
    `lectrice-link`, security fixes, cleaner v5. `CHANGELOG.md` `[0.4.0] - 2026-11-14`. Web app `0.4.0`.
  - **Desktop v0.2.0** (tag `v0.2.0`): three models (Fast recommended), the phone helper. Assets: `.rpm`, `.deb`,
    **AppImage** (works again), Windows NSIS `.exe`, FFmpeg 8.0.3 source + `.asc` + LGPL build scripts, `SHA256SUMS`.
    Pins the core by `tag = "v0.4.0"` (incl. `lectrice-link`); `Cargo.lock` has git sources (built without the
    path override). Staged files: `lectrice-desktop/target/release-v0.2.0/`.
  - **Android v0.1.0** (first release, tag `v0.1.0`): signed release APK `Lectrice-0.1.0.apk` (key on this laptop,
    `CN=Lectrice`) + `SHA256SUMS`. Pins the core by `tag = "v0.4.0"`. `versionCode=1`: raise it for the next APK.
- **Book titles are out of the new core commits** (user: "remove going forward"): the book tests use the keys
  `book-a` … `book-e` (A = the calibre e-book, B = the book with U+2010 syllable breaks, C = the all-Courier book, D = the Google
  Docs export, E = the XeTeX book). The gitignored `backend/test-data/books/expected.json` uses the same keys and
  `book-?.jsonl` links to the old fixture files. Older public mentions (`backend/README.md`, `toc.rs` unit tests,
  `cleaning_invented.rs`, some cleaner comments, docs, CLAUDE.md) are a later clean-up.
- Fixed in this pass: Android `:spike` compiles again; the Listen view uses the 2 × 2 tool grid at 200 % font on
  every width; an export the core pauses when the engine is rebuilt (pairing a computer, switching the helper) now
  continues at once (`core/work/EngineRebuild.kt` + test) instead of showing "Paused by Android". The clips of the
  unfinished 10-minute part are still made again after such a pause (by design: exports stream into parts, not into
  the cache). **This fix was not re-tested on the S10+** (the phone was locked with a password); JVM tests only.

## Pending
1. **User: the router-off test** of the helper (Wi-Fi without internet).
2. **User: Windows checks** of desktop v0.2.0: the helper's network detection and DPAPI key storage have never run
   on real Windows; a phone on another model than the desktop gets "Busy" there (D11); no "public network" hint.
3. **User: NPU results from the S26** (personal build, local only).
4. Re-test on the S10+: pair a computer while an export runs; the export should keep going.
5. Leftovers from `docs/review/2026-11-14-verification.md` §4 (local): N3 device-transfer rules, N4 model-load race,
   N5 lock-screen Listen view after restore, N6 footnote as "previous" line, N7 "References" chapter, N8 web voice
   list failure, N9 export poll trace line, N10 remaining review references (Android, desktop `Cargo.toml:21`),
   N13 version hint. Also the old book-title mentions above.
6. Local dev overrides: `lectrice-desktop/.cargo/config.toml` and `lectrice-android/.cargo/config.toml` still point
   at `../lectrice-core-android` (now merged into `main`); point them at `../lectrice/backend/crates` for new work.

## Repos
| Repo | Local | State |
|---|---|---|
| **lectrice** (core + web app) | `~/Desktop/learning/lectrice` → github.com/kia-salvati/lectrice | `main` + tags up to **`v0.4.0`**, `model-kokoro-lectrice-small-v1`, `model-kokoro-lectrice-fast-v1` pushed. `feature/core-android` (worktree `../lectrice-core-android`) and `feature/final-fixes` merged into `main`. **Kept local on purpose:** `docs/review/`, `docs/security/` (gitignored), `research/2026-10-25-qnn-license-review.md`, `research/2026-10-25-android-npu-test.md`, old screenshots `design/screens/android-[1-4]-*` |
| **lectrice-desktop** (Tauri 2) | `../lectrice-desktop` → github.com/kia-salvati/lectrice-desktop | `main` + tags `v0.1.0`, **`v0.2.0`** pushed (pins core `v0.4.0`) |
| **lectrice-android** | `../lectrice-android` → github.com/kia-salvati/lectrice-android | `main` + tag **`v0.1.0`** pushed (pins core `v0.4.0`). **Kept local on purpose (NPU):** the folders and files in `.git/info/exclude` and the NPU hunks in 20 tracked files (docs, Gradle files, `settings.gradle.kts`, `rust/lectrice-ffi/{Cargo.toml,src/composition.rs,src/lib.rs}`): **never `git add -A` / `commit -a` there**; stage by name |
| sherpa2 worktree | `../lectrice-sherpa2` | Spike only; sherpa 2.0 rejected; never push |

## Earlier snapshot (2026-11-13)
- Android M2–M4, features 23 (audiobook source) and 24 (desktop helper), desktop v0.2 models and security pass 1
  were built then; all of it is in the releases above.

D9 (import flash) and D11 (still screen until the file dialog has gone) are **committed** in core
`v0.3.0` (`frontend/`), and the desktop v0.1.0 installers include them.

## Released (2026-10-25)
- GitHub pre-releases: **desktop v0.1.0** (`.rpm`, `.deb`, Windows `.exe`, FFmpeg 8.0.3 source +
  `.asc` + build scripts, `SHA256SUMS`), **core v0.3.0**, and **`model-kokoro-lectrice-small-v1`**
  (our "Small" model, `kokoro-lectrice-w8-multi-lang-v1_0.tar.bz2`, sha256 `57595e9e…b1e2`).
- `gh` 2.103.2 is in `~/.local/bin` (logged in as kia-salvati). Release files were staged in
  `lectrice-desktop/target/release-v0.1.0/` and `target/model-release/`.
- The user tested desktop on Linux and Windows: "good". The buzz they heard is RustDesk's audio.

## Phone results (Android spike)
| | S10+ (Exynos 9820) | S26 Ultra (SM8850) |
|---|---|---|
| fp32 sustained RTF (10 min, unplugged) | 1.76 (red) | 0.58 at 8 threads (green, just); 0.75 at 4 |
| **"Fast" (our q8) sustained RTF** | **1.11** (still slower than real time) | **0.38** at 8 threads |
| Battery, fp32 → Fast | 36.9 → 21.6 mAh/audio-min (−41 %) | 13.3 → 8.8 (−34 %) |
| NPU (QNN, local test only) | n/a | 0.38 bench, 0.43→0.52 soak, 9.7 mAh/audio-min |
| Heat | moderate | severe within ~1 min at 8 threads |
- **Verdict (S8): GO**, older phones get Prepare-first (`research/2026-10-25-android-m1-results.md`,
  **waiting for the user's OK**). Generation must run in a foreground service (Samsung throttles
  background work 4–8×).
- **"Big cores only" made no difference** (keep `all`; tune threads per phone class: 4 on
  S10+-class, 8 on S26-class). `docs/progress/variants-cores.md`.
- **The user heard no difference between fp32, Small and Fast** (ear test 2026-10-25).
- Recommendation to decide: make **Fast the default**, keep Full quality (fp32), and replace the
  official int8 (slow, sounds different) with ours. The user earlier said "three choices: two
  official, one ours", so **ask** before dropping the official int8 or adding Fast.

## Research done (all in `research/`, uncommitted)
- `2026-10-25-android-npu-gpu.md`: options overview (QNN, LiteRT, Exynos, CPU tuning).
- `2026-10-25-kokoro-variants.md` (+ `-tools/`, ear-test WAVs gitignored): our Small (w8) and Fast
  (q8) bundles in `models/kokoro-lectrice-{w8,q8}-multi-lang-v1_0/`.
- `2026-10-25-android-npu-test.md`: NPU test on the S26 (results above; ear test passed).
- `2026-10-25-qnn-license-review.md`: no QNN in v0.x; no "NPU pack"; later only an opt-in "NPU
  edition" APK with the NPU in its own process; needs Qualcomm's written answers + a lawyer.
- `2026-10-25-litert-desktop-test.md`: Google's LiteRT Kokoro runs on the laptop; CPU 0.48 (ours
  0.30), **laptop GPU 0.11**; skipped words 0.007 % (passes) but 2.7 % of words are guessed from
  spelling (ear test: `research/2026-10-25-litert-ear-test/sNN-{sherpa1.13.8,litert,litert-gpu}.wav`);
  no GPL; Android GPU prebuilt `.so` licenses unclear. Phone GPU test not run yet (needs an arm64
  `omni_runner` build: steps in the report §"phone").
- `2026-10-25-android-m1-results.md`: the go/no-go decision page.

## Decided at the end of the session (user 2026-10-25)
- **Android GO accepted.** Next: build the real Android app (M2+).
- **Models: Fast (ours, default) + Full quality (official fp32) + Small (ours); drop the official
  int8.** To do: a GitHub model release for Fast (like `model-kokoro-lectrice-small-v1`), then add
  Fast and Small to the desktop and Android model manifests, each with its own engine id.
- **LiteRT: not for desktop.** For phones: not now (sherpa sounds better; Fast is already 0.38 on
  the S26). Effort if ever: phone test 2–4 days, app integration 2–4 weeks.
- Commits made and pushed (lectrice + lectrice-android), except the NPU test app and the two QNN
  research files, which stay local.

- **NPU (user 2026-10-25): no public NPU release, but don't drop it.** Next session: make a
  **personal NPU build for the user's S26 Ultra only** (local, never published or pushed;
  extends the local `npu-test/` work, see the kept-local files above). Size facts for later: the
  QNN libs add roughly tens of MB to the APK (npu-test APK 57 MB vs spike 40.8 MB; to measure);
  the phone also needs Full quality (~326 MB) + the chip-specific NPU vocoder files (~830 MB, or
  ~1.25 GB with long-sentence pieces; target ≤ 500 MB or compile on the phone while charging).
  Users of any future NPU edition would self-certify Qualcomm's terms on a first-use screen.

## Waiting on the user
1. **Qualcomm license eligibility:** see the private memory note `qnn-eligibility` (not in git).
   It only decides whether the NPU work continues.
2. (Background) LiteRT ear test: sherpa sounded better than LiteRT in most clips; LiteRT CPU and
   GPU sound the same. LiteRT is not tested on a phone.

## Next work
- **Android M2+** from `docs/android-logic/` (README build order), including the CPU fixes
  (thread count per phone class, foreground service), Prepare-first for slow phones, one
  generation queue for player + read-ahead, and the model choices with their own engine ids.
- **Desktop v0.2:** offer our model(s) in the model manifest (own engine id), the AppImage build fix
  (`failed to run linuxdeploy`; likely `patchelf` / `~/.cache/tauri/`), commit D9/D11, maybe the
  GPU path (LiteRT on the laptop GPU was 3× faster).
- **NPU toggle:** spec `docs/android-logic/22-npu-acceleration.md`; **design picked: option B** +
  yes to the 3 small questions; blocked on licensing.
- W4 CI (GitHub Actions) for desktop releases; core quota-full → `StorageFull`; saved exports 0600.

## Run it
```bash
# web
cd ~/Desktop/learning/lectrice/backend && cargo run --release -p lectrice-api   # :7878
cd ~/Desktop/learning/lectrice/frontend && npm run dev                          # :5173
# desktop (dev) and release builds
cd ~/Desktop/learning/lectrice-desktop
LECTRICE_MAIN_DIR=../lectrice LECTRICE_DEV_MODEL_DIR=../lectrice/models/kokoro-multi-lang-v1_0 cargo tauri dev
LECTRICE_MAIN_DIR=../lectrice scripts/build-release.sh    # rpm, deb (AppImage step currently fails)
LECTRICE_MAIN_DIR=../lectrice scripts/build-windows.sh    # NSIS installer (Fedora mingw32-nsis stubs)
# phones (wireless adb, paired): adb mdns services; spike/run-phone.sh <serial> <large.pdf> <medium.pdf> <threads>
```
Phones: S10+ and S26 Ultra over wireless adb (`adb devices -l`; serials and IPs are in the local memory notes, not in git).
Wireless debugging switches itself off sometimes; the user re-enables it (S26: Auto Blocker off).

## Rules that are easy to break
- **No text from the user's books** (`~/Desktop/*.pdf`) in any tracked file. Scan before every
  commit (9-word overlap check of changed files against `pdftotext` output of the books).
- **Push only `main` and version tags** (plus model release tags). Feature branches stay local.
- **Commit as the work goes and push when it makes sense** (user 2026-11-13): logical groups,
  no Co-Authored-By trailer, author `kia <limsclaude@gmail.com>`, remote alias `personal:`.
  Never commit the QNN research files, the personal NPU parts or unfixed security reports.
- **The main session only orchestrates agents.** Opus: research, specs, code, reviews, all UX
  thinking. Fable: design code only, from an Opus brief. **Designs: the user picks from options.**
- **Downloads:** Google's Android/Maven hosts, SourceForge and sometimes `repo1.maven.org` are
  blocked here; mirrors only when files can be checksum-verified. Never personal data in headers.
- **Laptop limits:** 13 GB RAM (it froze once with Bazel + Gradle in parallel): run heavy builds
  one at a time, Gradle `-Xmx2g --max-workers=2`, capped Bazel. Disk ~16 GB free (Bazel caches
  ~7.9 GB under `~/.cache/bazel`, `bazel clean --expunge` frees 6.5 GB).
- **Ctrl+C in the terminal stops all background agents**; Esc only interrupts the main reply.
- No visible scrollbars in the Android and desktop designs.
- The user works via RustDesk; give `xdg-open` paths (no `#fragment` in `xdg-open`: use
  `google-chrome "file:///…#…"`); for listening, the user sends files to their PC with LocalSend.

## Key documents
- Web/backend contract: `docs/2026-10-21-design-spec.md`; `CHANGELOG.md`; `docs/progress/*.md`.
- Desktop: `lectrice-desktop/docs/2026-10-24-desktop-{spec,plan,ux-brief,model-contract}.md`,
  `docs/2026-10-25-*.md`, `docs/progress/` (D0–D11, W1, W2, W2-fix).
- Android: `docs/android-logic/README.md` (+ file 22 NPU), `docs/2026-10-23-android-spec.md`,
  `docs/2026-10-23-android-m1-spike-plan.md`, `lectrice-android/docs/progress/`.
- Cleaner limits: `docs/progress/T.md`.
