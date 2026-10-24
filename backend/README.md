# Lectrice backend

Rust workspace: `lectrice-domain` ← `lectrice-infra`, `lectrice-app` ← `lectrice-api` (the binary).
Contract: `../docs/2026-10-21-design-spec.md`. Changes per release: `../CHANGELOG.md`.

The core (`lectrice-domain`, `-app`, `-infra`) is shared with the Android app, which pulls it by release tag.
`lectrice_app::services::assemble` wires adapters into services for every shell; `lectrice-api` only picks the
adapters from its config. `lectrice-infra` features, all on by default:

| Feature | Gates | Default |
|---|---|---|
| `pdfium` | `PdfiumTextSource` (pdfium-render) | on |
| `kokoro` | `KokoroSpeechEngine` (sherpa-onnx) | on |
| `sherpa-static` | links sherpa-onnx statically (implies `kokoro`) | on |
| `ffmpeg-export` | `FfmpegExporter` (the system ffmpeg) | on |
| `time-stretch` | `SonicTimeStretch` (exports with a baked-in speed; pure Rust) | on |

Android builds with `default-features = false, features = ["pdfium", "kokoro", "time-stretch"]`.

## Needs
- Rust ≥ 1.91 (rustup stable), `g++` (sherpa-onnx links libstdc++).
- Network on the first build: sherpa-onnx downloads its prebuilt static libs (21 MB) into `target/`.
  Offline: set `SHERPA_ONNX_ARCHIVE_DIR` to a folder holding the archive.
- `vendor/pdfium/lib/libpdfium.so` (pdfium-binaries `chromium/7881`, BSD-3, licenses next to it).
- `ffmpeg` with `libmp3lame` and `aac` for audio export (`[export] ffmpeg`). Without it, everything else
  works and exports answer `ExporterUnavailable`. `ffprobe` is only used by the ignored export tests:
  `cargo test -p lectrice-infra --test export_ffmpeg -- --ignored`.
- `LECTRICE_EXPORT_MAX_SENTENCES=60` exports only the first 60 sentences (quick manual checks).
- Exports are written as WAV parts of about 10 listening-minutes (`<data_dir>/exports/parts/<exportId>/`), then
  joined by ffmpeg. An export the server stop interrupted is paused and continues at its first unfinished part
  when the server starts again.

## Build, run, test
```bash
. "$HOME/.cargo/env"
cargo build
cargo run -p lectrice-api            # reads ./config.toml, listens on 127.0.0.1:7878
curl localhost:7878/api/health       # {"status":"ok","speechEngine":"..."}
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./build-release.sh                   # release build without /home/<user>/... paths in the binary
```
`cargo test -- --ignored` also runs tests that read the user's books in `~/Desktop`. They never write
into the repo: `pdf_fixtures` only checks that `test-data/books/*.jsonl` still match the books.

### Book test data (local only, never committed)
The books are copyrighted, so nothing taken from them is in git. `test-data/` and
`../docs/progress/B2-samples/` are gitignored.
- **Cleaner fixtures:** a few pages of each book as `TextRun`s in `test-data/books/*.jsonl`, used by
  `lectrice-domain/tests/cleaning_<book>.rs` and `cleaning_step_t_regressions.rs`. Without them (a fresh
  clone) those tests skip and print where the data should be; `cleaning_invented.rs` checks the same rules on
  invented text and always runs. To build them (the books must be in `~/Desktop`), or to rewrite them after
  changing `FIXTURE_PAGES` or the PDF reader:
  `LECTRICE_REGEN_FIXTURES=1 cargo test -p lectrice-infra --test pdf_fixtures -- --ignored`.
- **Review samples** (`docs/progress/B2-samples/*.txt`): the cleaned text of a few chapters, written by the
  dev command `cargo run -p lectrice-api -- clean <book.pdf> --out <dir> --pages <first>-<last>` (it writes
  `<name>.txt` and `<name>.document.json`). The pages used: Clean Architecture 52-64, Designing Distributed
  Systems 25-34, Design Patterns 99-109, The Kubernetes Book 222-235, LinkedIn Posts 8-30. For example:
  `cargo run -p lectrice-api -- clean ~/Desktop/Linkedin_Posts_2024_Blue.pdf --out ../docs/progress/B2-samples --pages 8-30`.
Stop the server with Ctrl+C or SIGTERM; it drains open requests first.

## Config
`config.toml` holds the defaults. Any key can be overridden as `LECTRICE_<SECTION>_<KEY>`
(`LECTRICE_SERVER_PORT=9000`, `LECTRICE_STORAGE_DATA_DIR=/tmp/lib`). `LECTRICE_CONFIG` points at a
different config file. Relative paths are taken from the config file's folder, and `~` is your home.
Logs are JSON lines; `RUST_LOG=debug` changes the level.
