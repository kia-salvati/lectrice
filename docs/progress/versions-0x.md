# Versions renumbered to beta 0.x (2026-10-25)

User decision 2026-10-24: every repo is in beta, so versions are 0.x. Tags were already moved
(`v1.0.0` → `v0.1.0`, `v1.1.0` → `v0.2.0`, old tags deleted on GitHub). This step changed the files only.
No tags created or moved, nothing committed or pushed, no branch switch or stash.

## Main repo (`lectrice`, branch `feature/desktop-support`)
- `backend/Cargo.toml` `[workspace.package] version` `1.2.0` → `0.3.0`; `backend/Cargo.lock` follows (the four
  `lectrice-*` crates at `0.3.0`, written by cargo).
- `CHANGELOG.md`: a note at the top about the renumbering; `[1.0.0]` → `[0.1.0]`, `[1.1.0]` → `[0.2.0]`; `[Unreleased]`
  says "To be tagged as 0.3.0"; the semver rule now says a breaking core change raises the **minor** version while in
  0.x; in-text "1.0"/"1.1" → "0.1"/"0.2"; compare/tag links use `v0.1.0`/`v0.2.0`. Kept: the Keep a Changelog 1.1.0
  link, `kokoro-v1.0` (model id).
- `frontend/package.json` and the lockfile's own version `0.0.0` → `0.3.0`.
- Frontend desktop mock/About line and its test: `coreVersion` `1.2.0` → `0.3.0`.
- Backend code comments that named our releases ("records saved before v1.1", "v1.0's id") → v0.2 / v0.1.
  The `kokoro-v1.0` engine id (Kokoro's model version) is unchanged.
- Docs: `docs/HANDOFF.md` (pending item now done), `docs/2026-10-23-android-spec.md` (§3.2 pins, §3.4 semver rule,
  §3.5, M0), `docs/2026-10-21-design-spec.md` ("(v1.1)" markers → "(v0.2)"), `docs/android-logic/` (README, 10, 11,
  12, 18, 19), `docs/2026-10-23-android-design-brief.md` and the two Android mockups' About line
  ("Version 0.1.0 · core 0.2.0"). `README.md` and `backend/README.md` had no version mentions.
- `CLAUDE.md`: only the "Versions are beta 0.x" pending item marked done.
- Product-scope words ("not in v1", "Android v1", "web v1") and branch/file names (`feature/core-v1.1`,
  `core-v1.1.md`) are unchanged.

## Android repo (`lectrice-android`)
- `rust/lectrice-ffi/Cargo.toml`: the three core deps `tag = "v1.1.0"` → `tag = "v0.2.0"`. The workspace/crate version
  stays `0.1.0`. `Cargo.lock` follows the path override (core crates `1.2.0` → `0.3.0`, written by cargo).
- The only app module is `spike`, `versionName = "0.1.0-spike"`: left as it is.
- `docs/progress/S3.md`, `S7.md`: core tag → `v0.2.0` (S7 notes the result files record the old name).

## Desktop repo (`lectrice-desktop`; W2 running in parallel)
- `Cargo.toml`: the four core deps `tag = "v1.1.0"` → `tag = "v0.2.0"`. App stays `0.1.0`.
- `docs/2026-10-24-desktop-spec.md`, `-plan.md`, `-ux-brief.md`, `-model-contract.md`,
  `design/2026-10-24-desktop-ux-options.html`: core `v1.2.0`/`v1.1.0` → `v0.3.0`/`v0.2.0`.

## Checks
- Main backend: `cargo build --workspace`, `cargo test --workspace` (all pass), `cargo clippy --workspace
  --all-targets -- -D warnings` (clean).
- Frontend: `npx tsc -b` clean, `npm test` 35/35.
- Android: `cargo metadata` resolves; `cargo build -p lectrice-ffi` (host) builds.
- Desktop: `cargo metadata` resolves. `cargo build --workspace` fails in `src-tauri/src/composition/platform.rs`
  (E0597, `ffmpeg_locator` borrows `resource_dir`): that is W2's in-progress Windows code, not this change; the core
  crates compiled.
- Grep for `1.2.0|v1.1.0|v1.0.0`: only third-party versions plus the leftovers below.

## Leftovers
- **Disk was full** (`/home` 15 MB free) during the desktop build. I deleted the regenerable
  `lectrice/backend/target/debug/incremental` (about 5 GB freed, 95 % used). The three `target/` folders hold about
  63 GB (main 32, desktop 25, android 6); a `cargo clean` of old ones would help.
- Desktop `Cargo.lock`: re-resolving after the tag change also moved some `windows-sys` users from 0.61.2 to 0.60.2 and
  dropped an unused `libloading 0.9.0`. W2's own `cargo xwin clippy` run wrote the same result, so it is cargo's
  resolution for the current manifests; W2 may want to look at it.
- Historical reports keep the old names on purpose (they describe what was true then): main
  `docs/progress/core-v1.1*.md`, desktop `docs/progress/D2.md`, `D3*.md`, `D6.md`, android `docs/results/*.json`
  (`coreRevision: "v1.1.0"`), and the `CLAUDE.md` history entries.
- When `v0.3.0` is tagged: move the Android and desktop pins to `v0.3.0`.
