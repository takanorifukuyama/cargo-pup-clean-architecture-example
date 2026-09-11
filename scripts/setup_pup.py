#!/usr/bin/env python3
"""Install the pinned cargo-pup toolchain and CLI (Python 3.11+, rustup required)."""

import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    config = tomllib.loads((ROOT / "pup-toolchain.toml").read_text(encoding="utf-8"))
    toolchain = config["toolchain"]
    version = config["version"]
    destination = ROOT / ".tools" / "cargo-pup"
    subprocess.run(
        [
            "rustup", "toolchain", "install", toolchain, "--profile", "minimal",
            "--component", "rust-src", "--component", "rustc-dev",
            "--component", "llvm-tools-preview",
        ],
        check=True,
        cwd=ROOT,
    )
    subprocess.run(
        [
            "cargo", f"+{toolchain}", "install", "cargo_pup",
            "--version", f"={version}", "--locked", "--root", str(destination),
        ],
        check=True,
        cwd=ROOT,
    )
    binary_dir = destination / "bin"
    if os.environ.get("GITHUB_PATH"):
        with Path(os.environ["GITHUB_PATH"]).open("a", encoding="utf-8") as handle:
            handle.write(f"{binary_dir}\n")
    print(f"Installed cargo-pup {version} with {toolchain}.", flush=True)
    print(f'For interactive cargo pup commands: export PATH="{binary_dir}:$PATH"')


if __name__ == "__main__":
    main()
