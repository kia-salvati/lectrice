# Core v1.1.0: follow-ups from S3 (2026-10-24)

Small fixes found while building the Android spike (`../lectrice-android/docs/progress/S3.md`). Uncommitted, on
`feature/core-v1.1`.

## 1. Logging rule: no file names

- `ImportPdf` (`lectrice-app/src/documents/import_pdf.rs`) logged `file_name` at info. Removed; the line now has the
  request id and the size in bytes. The name is still stored in the document (shown, never logged).
- Every `info!`/`warn!`/`error!`/`debug!` in the core and the server was checked (146 calls, read whole). The only other
  case was the `clean` dev command (`lectrice-api/src/dev_clean/mod.rs`): it logged the PDF's file name and the two
  output paths built from it. It now logs the document id and the output folder.
- Left as is, because they hold no book content: `main.rs` (config path, data folder), `dev_speak` (the output path the
  user typed), `run.rs` "export file written" (`exports/<doc id>/<voice>[.<engine>.<speed>].m4b`), error details with
  paths (library paths use the document id; ffmpeg runs with `-loglevel error`, so its stderr has no tags).

## 2. Core version

`lectrice_app::CORE_VERSION` = `env!("CARGO_PKG_VERSION")` of `lectrice-app`, which is the workspace version (one
source: `backend/Cargo.toml` `[workspace.package] version`, which equals the release tag). `lectrice-ffi` can expose it:

```rust
#[uniffi::export]
pub fn core_version() -> String { lectrice_app::CORE_VERSION.to_string() }
```

The web API's health response has no version field, so nothing was added there (that would be a response-shape change
with the frontend types and mock). The server's startup log (`Config loaded`) now carries `core_version`.

## 3. Spec: `message` → `user_message`

`docs/2026-10-23-android-spec.md` §4.6 and §5.2 (`CoreError::Failed` and, for one shape, `CoreErrorInfo`) now say
`user_message`, with a line on why: uniffi makes `CoreError` a Kotlin exception, and a `message` field clashes with
`Throwable.message`. `docs/android-logic/19-core-and-ffi-changes.md` doesn't name the field; `12-errors-states-and-slow-phones.md`
did and was updated.

## 4. No absolute paths in release binaries

Cargo's `trim-paths` profile option is still unstable on 1.99.1 (checked: "feature `trim-paths` is required"), and a
tracked `.cargo/config.toml` can't hold `--remap-path-prefix` for any user (the prefixes are absolute). So it's a
script: `backend/build-release.sh` runs `cargo build --release` with
`--config build.rustflags=[...]` remapping `$HOME` → `/home`, `$CARGO_HOME` → `/cargo` and the backend folder →
`/lectrice/backend` (rustc uses the last prefix that matches, so the most specific comes last). Extra arguments go to
cargo. A plain `cargo build --release` still embeds the paths; switching between the two rebuilds the crates.

The core can't set this for the Android build: rustflags come from the top-level build. Lines for
`../lectrice-android/rust/build-android.sh`, right after `build_args=(build -p lectrice-ffi --lib --release)` (cargo-ndk
4.1.2 doesn't set `RUSTFLAGS`/`CARGO_ENCODED_RUSTFLAGS`, so `build.rustflags` is used; the `$HOME` rule also covers the
core checkout and the sherpa2 worktree):

```bash
# No /home/<user>/... paths in the library (Cargo's trim-paths isn't stable yet); rustc uses the last matching prefix.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
build_args+=(--config "build.rustflags=[\"--remap-path-prefix=$HOME=/home\", \"--remap-path-prefix=$cargo_home=/cargo\", \"--remap-path-prefix=$ROOT=/lectrice-android\"]")
```

## Verification (in `backend/`)

| Command | Result |
|---|---|
| `cargo fmt --check` | ok |
| `cargo clippy --all-targets -- -D warnings` | ok |
| `cargo test` | ok: 428 passed, 0 failed, 12 ignored |
| `cargo build --release` | ok; `strings target/release/lectrice-api \| grep -c /home/kia` = **306** (all `~/.cargo/registry/src/...`) |
| `./build-release.sh` | ok; `grep -c /home/kia` = **0** |

What remains in the remapped binary: 323 `/cargo/registry/src/...` paths (crate panic locations, now without the user)
and 261 `/home/runner/work/...` paths. Those come from sherpa-onnx's prebuilt static libraries (built on GitHub's CI
runners); they aren't ours and name no user of this machine.

## Files changed

`backend/crates/lectrice-app/src/documents/import_pdf.rs`, `backend/crates/lectrice-app/src/lib.rs`,
`backend/crates/lectrice-api/src/main.rs`, `backend/crates/lectrice-api/src/dev_clean/mod.rs`,
`backend/build-release.sh` (new), `backend/README.md`, `CHANGELOG.md`, `docs/2026-10-23-android-spec.md`,
`docs/android-logic/12-errors-states-and-slow-phones.md`, and this note.
