# Lectrice: session handoff (2026-10-23)

Read this first, then `CLAUDE.md` (the full decision log and rules). Everything below is current as
of 2026-10-23.

## What exists
| | Where | State |
|---|---|---|
| **Web app v1 + chapters** | `~/Desktop/learning/lectrice` → github.com/kia-salvati/lectrice (public) | Done and pushed to `main`. Tag `v1.0.0` = v1 without chapters |
| **Android app** | `~/Desktop/learning/lectrice-android` → github.com/kia-salvati/lectrice-android (public) | Only starter files (README, licenses, .gitignore, CLAUDE.md) on `main`. No code yet |
| **Voice-model research** (side topic) | `~/Desktop/learning/voice-models/2026-10-23-voice-and-anime-models.md` | Done; not part of the repos |

**v1 does:** PDF → cleaned reading view (headers, footers and citation markers removed; page numbers,
printed labels and headings kept) → Kokoro `af_heart` voice reading sentence by sentence with a
highlight, speed 0.75–2×, 28 English voices, click-to-jump, recent documents + "Continue", 10 min of
read-ahead scaled by speed, a ~1 h cache per book, and MP3/M4B whole-book export. **Chapters:** a
`toc` per document, a closable whole-book outline rail on wide screens, a bottom sheet elsewhere,
and "Ch. 7 · Title · p. 213" in the player pill.

## Run it
```bash
cd ~/Desktop/learning/lectrice/backend && cargo run --release -p lectrice-api   # :7878
cd ~/Desktop/learning/lectrice/frontend && npm run dev                          # :5173 (dev:mock = sample data)
```
Gitignored files that must exist locally: `models/kokoro-multi-lang-v1_0/`,
`backend/vendor/pdfium/lib/libpdfium.so`, `backend/test-data/books/` (book fixtures),
`docs/progress/B2-samples/`. How to get each one back: `backend/README.md`.

## Rules that are easy to break
- **Public repos: no text from the user's copyrighted books** (`~/Desktop/*.pdf`) in any tracked file:
  no samples, fixtures or prose quotes. Titles and counts are fine. Book tests read the PDFs directly,
  are `#[ignore]`, and skip without them. Scan for book sentences before every commit.
- **Push only `main` and version tags.** Feature and release branches stay local.
- **Commits:** only when the user asks; propose groups first; no Co-Authored-By trailer. Author is
  `kia <limsclaude@gmail.com>` (repo-local config). Remote alias `personal:` = `~/.ssh/config`
  (key `~/.ssh/github`, port 443).
- **Agents:** the main session orchestrates and doesn't do the work itself (keeps context small).
  **Opus** for research, specs, code and reviews; **Fable** only for UI design and building.
  For UI work, Opus writes the brief and options, then Fable builds. No other models. Design options
  are local HTML in `design/`, never published Artifacts. Each step writes `docs/progress/<step>.md`.
- Coding rules: `rules/` (universal parts only; see CLAUDE.md for what to ignore).
- The user often works remotely via RustDesk; remote audio works, so give `xdg-open` paths.
- The user is on a **metered network** right now: no big downloads without asking.

## Uncommitted right now
- `lectrice/CLAUDE.md`: notes on the `main` branch and the "push only main + tags" rule.
- `lectrice/docs/HANDOFF.md`: this file.
- `lectrice-android/CLAUDE.md`: the same push rule.
Commit them with the next change (ask first).

## Next steps
1. **Android M1, the go/no-go spike** (`docs/2026-10-23-android-m1-spike-plan.md`).
   - **Waiting on the user:** run the download block in §3a when on an unmetered network (~1.6 GB,
     no sudo; +2–3 GB on the first Gradle build), then say so.
   - Then the core changes for **v1.1.0** (spec §3.5: infra feature flags, move the wiring into
     `lectrice-app`, no desktop paths in the core, CHANGELOG + semver tags, resumable export), the
     `lectrice-ffi` crate, and the spike on the Galaxy S10+ (slow case) and the S26 Ultra
     (decides go/no-go). Pass line: real-time factor ≤ 0.6 green, ≤ 0.9 amber.
   - Includes: fp32 vs int8, sherpa-onnx 1.13.8 vs 2.0.0 (2.0 drops GPL espeak-ng but skips unknown
     words: switch if ≤ 0.1 % skipped on average and ≤ 0.25 % in the worst book + an ear test), and
     the accelerator experiment (cpu / xnnpack / nnapi; QNN NPU only if the CPU result disappoints).
2. **Android decisions already made:** Kotlin + Jetpack Compose with Telegram's *principles* (never
   its code); one shared Rust core, pulled by tag via Cargo git dependencies, never forked; no
   server, everything on the phone; the user picks the model to download; GitHub Releases only
   (signed APK via Actions later; back up the key); lock-screen "next" = next sentence; iOS, if
   ever, is a separate `lectrice-ios` repo.
3. Optional: a GitHub Release page for `v1.0.0` (the user offered; release notes not written yet).
4. Known v1 cleaner limits (`docs/progress/T.md`): figure text and the GoF table come out as
   fragments, tables are only "Table.", a few line splits remain, and "Tweet This Book!" is spoken.

## Key documents
- `docs/2026-10-21-design-spec.md`: the web/backend contract (incl. `toc`).
- `docs/2026-10-21-implementation-plan.md`, `docs/progress/*.md`: how v1 was built and reviewed.
- `docs/2026-10-23-chapters-brief.md`, `design/2026-10-23-chapter-options-v2.html` (take 1 + label A).
- `research/2026-10-23-android-options.md`, `docs/2026-10-23-android-spec.md`,
  `docs/2026-10-23-android-m1-spike-plan.md`.
- `research/2026-10-21-pdf-survey.md` (cleaner rules), `research/2026-10-06-rust-stack.md` (crates).
