# Lectrice: session handoff (2026-10-25, evening)

Read this first, then `CLAUDE.md` (the full decision log and rules). The previous session got too
long; everything needed to continue is here. **No agents are running.**

## Repos
| Repo | Local | State |
|---|---|---|
| **lectrice** (core + web app) | `~/Desktop/learning/lectrice` → github.com/kia-salvati/lectrice | `main` + tags `v0.1.0`, `v0.2.0`, **`v0.3.0`** and `model-kokoro-lectrice-small-v1` pushed. Local branch `feature/desktop-support` = `main`; all pushed. **Kept local on purpose:** `research/2026-10-25-qnn-license-review.md` (redact §6.1a before ever committing) and `research/2026-10-25-android-npu-test.md`; old option screenshots `design/screens/android-[1-4]-*` |
| **lectrice-desktop** (Tauri 2) | `../lectrice-desktop` → github.com/kia-salvati/lectrice-desktop | `main` + tag **`v0.1.0`** pushed (pins core `v0.3.0`). Clean |
| **lectrice-android** | `../lectrice-android` → github.com/kia-salvati/lectrice-android | `main` pushed (spike, `lectrice-ffi`, bundled models, `cores=` option, automation, S5/S7/variants results). **To do: move the core pin `tag = "v0.2.0"` → `v0.3.0`** (it already uses v0.3.0 APIs through the path override). **Kept local on purpose (NPU, licensing):** `npu-test/`, its edits in `settings.gradle.kts`, `gradle/libs.versions.toml`, `gradle/verification-*`, `docs/progress/NPU-prep.md`, `docs/results/S26U-npu-run1.json` |
| sherpa2 worktree | `../lectrice-sherpa2` | Spike only; sherpa 2.0 rejected; never push |

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
Phones: S10+ `adb-R58M23PXZYM-…` (192.168.70.129), S26 Ultra `adb-R5GL7354WDB-…` (192.168.70.154).
Wireless debugging switches itself off sometimes; the user re-enables it (S26: Auto Blocker off).

## Rules that are easy to break
- **No text from the user's books** (`~/Desktop/*.pdf`) in any tracked file. Scan before every
  commit (9-word overlap check of changed files against `pdftotext` output of the books).
- **Push only `main` and version tags** (plus model release tags). Feature branches stay local.
- **Commits only when asked; propose groups first; no Co-Authored-By trailer.** Author
  `kia <limsclaude@gmail.com>`. Remote alias `personal:`.
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
