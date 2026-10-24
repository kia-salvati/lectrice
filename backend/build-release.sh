#!/usr/bin/env bash
# `cargo build --release` without this machine's absolute paths (/home/<user>/...) in the binaries.
# Cargo's `trim-paths` profile option isn't stable yet (Rust 1.99), so the paths are remapped by rustc.
# Extra arguments go to cargo, e.g. `./build-release.sh -p lectrice-api`.
set -euo pipefail
cd "$(dirname "$0")"

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
# rustc applies the last prefix that matches, so the most specific comes last.
cargo build --release \
  --config "build.rustflags=[\"--remap-path-prefix=$HOME=/home\", \"--remap-path-prefix=$cargo_home=/cargo\", \"--remap-path-prefix=$PWD=/lectrice/backend\"]" \
  "$@"
