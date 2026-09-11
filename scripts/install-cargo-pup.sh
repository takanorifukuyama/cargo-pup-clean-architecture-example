#!/usr/bin/env bash
set -euo pipefail

# Keep in sync with tests/architecture.rs and the documented compatibility pair.
PUP_VERSION=0.1.8
PUP_TOOLCHAIN=nightly-2026-01-22

command -v rustup >/dev/null || {
  echo 'rustup is required. Install Rust from https://rustup.rs first.' >&2
  exit 1
}

rustup toolchain install "$PUP_TOOLCHAIN" --profile minimal \
  --component rust-src --component rustc-dev --component llvm-tools-preview
cargo +"$PUP_TOOLCHAIN" install cargo_pup --version "$PUP_VERSION" --locked
cargo pup --version
