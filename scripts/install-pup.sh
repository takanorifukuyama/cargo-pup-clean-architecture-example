#!/usr/bin/env bash
set -euo pipefail

# Pin both the compiler internals and the exact inspected upstream source.
readonly NIGHTLY="nightly-2026-01-22"
readonly PUP_REV="a2c06497096123d4d37f622ddadb934831c60e92"

rustup toolchain install "$NIGHTLY" --profile minimal \
  --component rust-src --component rustc-dev --component llvm-tools-preview
cargo "+$NIGHTLY" install cargo_pup --locked \
  --git https://github.com/DataDog/cargo-pup.git --rev "$PUP_REV"
cargo pup --version
