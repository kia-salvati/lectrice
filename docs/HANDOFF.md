# Lectrice: session handoff (2026-10-24)

Read this first, then `CLAUDE.md` (the full decision log and rules).

## State
| What | Where | State |
|---|---|---|
| Web app v1 + chapters | `lectrice/` → github.com/kia-salvati/lectrice (public) | On `main`, pushed. Tag `v1.0.0` = v1 without chapters |
| Core v1.1.0 | branch `feature/core-v1.1` | Done: part 1, part 2 (C10–C30 except C20/C21/C6b), review fixes, follow-ups. 428 tests. Tag `v1.1.0` after commit |
| Android design | `design/2026-10-23-android-quiet-v2.html` | **Approved** (Quiet v2) + the new storage/remove/export-space screens; M4B only |
| Android logic spec | `docs/android-logic/` (README + 21 feature files) | Done. Start here for any Android work |
| Android M1 spike | `../lectrice-android` (`spike/`, `rust/lectrice-ffi`) | S1, S3, S4, S6 done; both APKs build (40.8 / 40.6 MB). **Waiting on the phones** |
| sherpa2 worktree | `../lectrice-sherpa2` (local branch `feature/android-spike-sherpa2`) | For the `sherpa2` spike build; never push |

**v1:** PDF → cleaned reading view (page labels and headings kept, not spoken) → Kokoro `af_heart`
reading sentence by sentence with a highlight; speed 0.75–2×, 28 voices, click-to-jump, recent
documents + "Continue", 10 min read-ahead (scaled by speed), ~1 h cache per book, MP3/M4B export,
chapters (outline rail / bottom sheet, "Ch. 7 · Title · p. 213" in the pill).

**Core v1.1.0:** infra feature flags (`pdfium`, `kokoro`, `sherpa-static`, `ffmpeg-export`,
`time-stretch`), wiring in `lectrice-app` (`services::assemble`), no desktop paths, resumable export
in ~10-minute parts, baked-in export speed (Sonic in Rust, see `NOTICE`), partial exports,
StorageFull pauses and keeps parts, fp32/int8 engine ids, read-ahead stop, voice samples, remove /
restore / purge of books, cache in its own folder, `CORE_VERSION`, `backend/build-release.sh`
(no `/home/<user>` paths). Reports: `docs/progress/core-v1.1*.md`. New web routes:
`DELETE /api/documents/{id}`, `GET /api/voices/{voiceId}/sample`.

**S6 result (`research/2026-10-24-sherpa2-oov.md`): stay on sherpa-onnx 1.13.8.** 2.0 skips
2.54 % of spoken words on average (4.87 % worst) even after all prototyped fixes; the limit is
0.1 % / 0.25 %. So the APK carries GPL-3 espeak-ng for now. NNAPI/XNNPACK desk check: expect no gain.
Ear-test WAVs (gitignored): `research/2026-10-24-sherpa2-ear-test/`.

## Run it
```bash
cd ~/Desktop/learning/lectrice/backend && cargo run --release -p lectrice-api   # :7878
cd ~/Desktop/learning/lectrice/frontend && npm run dev                          # :5173 (dev:mock = sample data)
```
Gitignored files that must exist locally: `models/kokoro-multi-lang-v1_0/` (+ `kokoro-int8-…`),
`backend/vendor/pdfium/lib/libpdfium.so`, `backend/test-data/books/`, `docs/progress/B2-samples/`.
How to get them back: `backend/README.md`. Android toolchain: see CLAUDE.md "Downloads".

## Rules that are easy to break
- **Public repos: no text from the user's books** (`~/Desktop/*.pdf`) in any tracked file. Titles
  and counts are fine. Scan before every commit (9-word overlap check against `pdftotext` output).
- **Push only `main` and version tags.** Feature and release branches stay local.
- **Commits:** only when asked; propose groups first; no Co-Authored-By trailer. Author
  `kia <limsclaude@gmail.com>`. Remote alias `personal:` (`~/.ssh/config`, key `~/.ssh/github`).
- **Agents:** the main session only orchestrates. **Opus** does research, specs, code, reviews and
  all UI/UX thinking; **Fable** only writes design code from Opus's brief. No other models. Designs
  are local HTML in `design/`, never published Artifacts. Each step writes `docs/progress/<step>.md`.
- Coding rules: `rules/` (universal parts only; see CLAUDE.md).
- **Downloads:** Google's Android SDK and Maven hosts are blocked here. Use a mirror only when
  files can be checked against a checksum (details in CLAUDE.md).
- Android design: no visible scrollbars anywhere (scrolling still works).
- The user often works via RustDesk; give `xdg-open` paths.

## Android decisions
- Kotlin + Jetpack Compose with Telegram's *principles* (never its code). One shared Rust core,
  pulled by tag via Cargo git dependencies, never forked. No server. GitHub Releases only.
- Design "Quiet" v2; export M4B only with voice / quality / baked-in speed; size predicted first,
  partial export if short of space; read-ahead stop rules; cache in `cacheDir`;
  `manageSpaceActivity`; "Save to Audiobooks" **moves** to `Audiobooks/Lectrice/`; "Remove from
  library" with 8 s undo. Full detail: `docs/android-logic/`.

## Next steps
1. **The user (phones), spike plan §4.2 and §7:**
   - **S0:** developer options + wireless debugging on the S10+ and S26 Ultra; the S10+ model
     number; copy 2 PDFs onto each phone; say whether at home (adb) or away (share mode).
   - **S5 (S26 Ultra) and S7 (S10+):** install both APKs, `spike/push-model.sh`, import, Run all
     (unplugged for the soak), Play 2 min (listen for gaps), pull the result JSON.
   - Listen to the ear-test WAVs (1.13.8 vs 2.0) if curious; the numbers already decide.
2. **Then S8:** the results report and the GO / Prepare-first / fallback decision.
3. Android M2+ build from `docs/android-logic/` (README build order).
4. Optional: GitHub Release pages for `v1.0.0` and `v1.1.0`.
5. Known cleaner limits (`docs/progress/T.md`): figure text and the GoF table come out as fragments,
   tables are only "Table.", a few line splits remain, "Tweet This Book!" is spoken.

## Key documents
- Web/backend contract: `docs/2026-10-21-design-spec.md`; build history: `docs/progress/*.md`.
- Android: `docs/android-logic/README.md`, `docs/2026-10-23-android-spec.md`,
  `docs/2026-10-23-android-m1-spike-plan.md`, `research/2026-10-24-sherpa2-oov.md`,
  `../lectrice-android/docs/progress/S1.md`–`S4.md`, `../lectrice-android/docs/versions.md`.
- Cleaner rules: `research/2026-10-21-pdf-survey.md`; crates: `research/2026-10-06-rust-stack.md`.
