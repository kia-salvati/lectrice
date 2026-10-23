# Lectrice: session handoff (2026-10-24)

Read this first, then `CLAUDE.md` (the full decision log and rules).

## State
| What | Where | State |
|---|---|---|
| Web app v1 + chapters | `lectrice/` → github.com/kia-salvati/lectrice (public) | On `main`, pushed. Tag `v1.0.0` = v1 without chapters |
| Core v1.1.0 | local branch `feature/core-v1.1` (checked out) | Built and tested, **not committed**. `docs/progress/core-v1.1.md` |
| Android design | `design/2026-10-23-android-quiet-v2.html` | **Approved** (option 3 "Quiet" v2) |
| Android logic spec | `docs/android-logic/` (README + 21 feature files) | Done. Start here for any Android work |
| Android app | `../lectrice-android` → github.com/kia-salvati/lectrice-android (public) | Starter files only |

**v1:** PDF → cleaned reading view (page labels and headings kept, not spoken) → Kokoro `af_heart`
reading sentence by sentence with a highlight; speed 0.75–2×, 28 voices, click-to-jump, recent
documents + "Continue", 10 min read-ahead (scaled by speed), ~1 h cache per book, MP3/M4B export,
chapters (outline rail / bottom sheet, "Ch. 7 · Title · p. 213" in the pill).

**Core v1.1.0 so far:** infra feature flags (`pdfium`, `kokoro`, `sherpa-static`, `ffmpeg-export`),
wiring moved into `lectrice-app` (`services::assemble`), no desktop paths in the core, resumable
export in ~10-minute parts, a Kokoro `provider` setting, `CHANGELOG.md`. 351 tests pass; clippy and fmt
clean. Review `lectrice-domain/src/export.rs` closely (edited with a python replace). The design spec §8
still has the old error text. **The new core changes in `docs/android-logic/19-*.md` (C10–C30) also
go into v1.1.0**; tag it only after the user reviews and commits.

## Run it
```bash
cd ~/Desktop/learning/lectrice/backend && cargo run --release -p lectrice-api   # :7878
cd ~/Desktop/learning/lectrice/frontend && npm run dev                          # :5173 (dev:mock = sample data)
```
Gitignored files that must exist locally: `models/kokoro-multi-lang-v1_0/`,
`backend/vendor/pdfium/lib/libpdfium.so`, `backend/test-data/books/`, `docs/progress/B2-samples/`.
How to get them back: `backend/README.md`.

## Rules that are easy to break
- **Public repos: no text from the user's books** (`~/Desktop/*.pdf`) in any tracked file. Titles
  and counts are fine. Book tests are `#[ignore]` and skip without the PDFs. Scan before every commit.
- **Push only `main` and version tags.** Feature and release branches stay local.
- **Commits:** only when asked; propose groups first; no Co-Authored-By trailer. Author
  `kia <limsclaude@gmail.com>`. Remote alias `personal:` (`~/.ssh/config`, key `~/.ssh/github`).
- **Agents:** the main session only orchestrates. **Opus** does research, specs, code, reviews and
  all UI/UX thinking (ideas, options, exact build briefs); **Fable** only writes design code from
  Opus's brief. No other models. Designs are local HTML in `design/`, never published Artifacts.
  Each step writes `docs/progress/<step>.md`.
- Coding rules: `rules/` (universal parts only; see CLAUDE.md).
- Android design: no visible scrollbars anywhere (scrolling still works).
- The user often works via RustDesk; give `xdg-open` paths. **Metered network:** no big downloads
  without asking.

## Uncommitted (on `feature/core-v1.1`)
Core v1.1.0 code + `CHANGELOG.md` + `docs/progress/core-v1.1.md`; `CLAUDE.md`, this file; the Android
design briefs, mockups, screenshots and progress notes; `docs/android-logic/`. Propose commit groups first.

## Android decisions
- Kotlin + Jetpack Compose with Telegram's *principles* (never its code). One shared Rust core,
  pulled by tag via Cargo git dependencies, never forked. No server. GitHub Releases only. iOS, if
  ever, is `lectrice-ios`.
- Design "Quiet" v2: option 1's library with Quiet's round Open PDF button; tapping the pill status
  opens the Listen stage; chapters via ⋯ → Chapters (1 tap from Listen); current sentence as the
  lock-screen title; Listen over the lock screen is a setting, off by default.
- The user picks the model (fp32 or int8). Lock-screen "next" = next sentence. Voice sample = one
  fixed sentence, made on demand and cached.
- **Export: M4B only** (no MP3, license). Pick voice, model quality and speed (baked in, Sonic).
  Size is predicted first; if space is short, the full export is blocked, with "Free up space" or
  a partial export. Out of space mid-export pauses and keeps the parts.
- Read-ahead stops on pause (60 s), sleep timer, book change, low memory, heat, battery saver.
- **Storage:** the audio cache lives in `cacheDir` (shows as "Cache" in Android's Settings → Apps →
  Lectrice); `manageSpaceActivity` opens our Storage screen; exports stay in app storage until
  "Save to Audiobooks" **moves** them to `Audiobooks/Lectrice/` (user confirmed 2026-10-24).
- "Remove from library" is in v1 (confirm + 8 s undo; exported files kept by default).

## Next steps
1. **Waiting on the user:** review the core diff and say when to commit.
2. **Design pass for the new screens** (Opus brief → Fable), listed in `docs/android-logic/README.md`
   §7: book sheet + remove/undo (L7*), Storage screen (T3*), not enough space / partial export
   (E5b, E6, E7), and the E3/E4/E5 variants.
3. **Android M1 spike** (`docs/2026-10-23-android-m1-spike-plan.md`):
   - The user runs `bash ~/Downloads/lectrice-android-dl.sh` on an unmetered network (~1.6 GB, no
     sudo, answer `y` to the licenses), then an agent adds the `~/.zshenv` lines (shown first).
   - Agents write `lectrice-ffi` and the spike app (S3/S4; can be done before the download). Then
     `cd ~/Desktop/learning/lectrice-android && ./gradlew :spike:assembleSherpa1Release :spike:assembleSherpa2Release`
     downloads the other 2–3 GB (unmetered too).
   - Run on the Galaxy S10+ and S26 Ultra. Pass line: real-time factor ≤ 0.6 green, ≤ 0.9 amber.
     Also fp32 vs int8, sherpa-onnx 1.13.8 vs 2.0.0, accelerators.
4. Optional: a GitHub Release page for `v1.0.0`.
5. Known cleaner limits (`docs/progress/T.md`): figure text and the GoF table come out as fragments,
   tables are only "Table.", a few line splits remain, "Tweet This Book!" is spoken.

## Key documents
- Web/backend contract: `docs/2026-10-21-design-spec.md`; build history: `docs/progress/*.md`.
- Android: `docs/android-logic/README.md`, `docs/2026-10-23-android-spec.md`,
  `docs/2026-10-23-android-m1-spike-plan.md`, `docs/2026-10-23-android-quiet-v2-brief.md`,
  `research/2026-10-23-android-options.md`.
- Cleaner rules: `research/2026-10-21-pdf-survey.md`; crates: `research/2026-10-06-rust-stack.md`.
