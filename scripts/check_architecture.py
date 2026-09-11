#!/usr/bin/env python3
"""Verify real source, compilable violations, and a documented cargo-pup limitation.

Every case is checked in a fresh temporary copy: no mutation of the working tree,
no stale .pup cache, and no successful test due to an unrelated compiler error.
"""

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Case:
    name: str
    path: str = ""
    addition: str = ""
    expected_rule: str | None = None
    known_gap: bool = False


CASES = (
    Case("valid_architecture"),
    Case("domain_to_application", "src/domain.rs",
         "pub use crate::application::TaskService;",
         "domain_inward_only"),
    Case("domain_to_infrastructure", "src/domain.rs",
         "pub use crate::infrastructure::InMemoryTaskRepository;",
         "domain_inward_only"),
    Case("domain_to_presentation", "src/domain.rs",
         "pub use crate::presentation::TaskController;",
         "domain_inward_only"),
    Case("application_to_infrastructure", "src/application.rs",
         "pub use crate::infrastructure::InMemoryTaskRepository;",
         "application_inward_only"),
    Case("application_to_presentation", "src/application.rs",
         "pub use crate::presentation::TaskController;",
         "application_inward_only"),
    Case("presentation_to_infrastructure", "src/presentation.rs",
         "pub use crate::infrastructure::InMemoryTaskRepository;",
         "presentation_no_direct_storage"),
    Case("infrastructure_to_application", "src/infrastructure.rs",
         "pub use crate::application::TaskService;",
         "infrastructure_uses_domain_ports"),
    Case("infrastructure_to_presentation", "src/infrastructure.rs",
         "pub use crate::presentation::TaskController;",
         "infrastructure_uses_domain_ports"),
    Case("nested_domain_to_infrastructure", "src/domain.rs",
         "pub mod boundary_probe { pub use crate::infrastructure::InMemoryTaskRepository; }",
         "domain_inward_only"),
    Case("relative_aliased_import", "src/domain.rs",
         "pub use super::infrastructure::InMemoryTaskRepository as ForbiddenRepository;",
         "domain_inward_only"),
    Case("domain_to_filesystem", "src/domain.rs",
         "pub use std::fs::File;",
         "domain_inward_only"),
    Case("fully_qualified_path_known_gap", "src/domain.rs",
         "pub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {\n"
         "    crate::infrastructure::InMemoryTaskRepository::default()\n}",
         known_gap=True),
)


def verify_pup_result(case: Case, result: subprocess.CompletedProcess[str]) -> None:
    """A nonzero exit alone is NOT evidence that an architecture rule worked."""
    output = result.stdout or ""
    if case.expected_rule:
        expected = (case.expected_rule, "Use of module", "is denied")
        if result.returncode <= 0 or not all(token in output for token in expected):
            raise AssertionError(
                f"Expected a named import violation for {case.expected_rule}; "
                f"exit={result.returncode}.\n{output[-5000:]}"
            )
    elif result.returncode != 0:
        label = "Documented gap changed; inspect and update the characterization" if case.known_gap else "Valid architecture failed"
        raise AssertionError(f"{label}; exit={result.returncode}.\n{output[-5000:]}")


def run_command(args: list[str], cwd: Path, log: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            args, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=180, check=False,
        )
    except subprocess.TimeoutExpired as error:
        partial = error.stdout or b""
        if isinstance(partial, bytes):
            partial = partial.decode("utf-8", errors="replace")
        log.write_text(f"TIMEOUT: {args!r}\n{partial}", encoding="utf-8")
        raise RuntimeError(f"Command timed out; inspect {log}") from error
    log.write_text(
        f"COMMAND: {args!r}\nEXIT: {result.returncode}\n{result.stdout}",
        encoding="utf-8",
    )
    return result


def run_case(case: Case, pup: Path, toolchain: str, logs: Path) -> None:
    with tempfile.TemporaryDirectory(prefix=f"pup-{case.name}-") as directory:
        project = Path(directory)
        for filename in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "pup.ron"):
            shutil.copy2(ROOT / filename, project / filename)
        for folder in ("src", "tests"):
            shutil.copytree(ROOT / folder, project / folder)
        if case.path:
            with (project / case.path).open("a", encoding="utf-8") as handle:
                handle.write(f"\n{case.addition}\n")
        env = dict(os.environ, CARGO_TERM_COLOR="never", NO_COLOR="1")
        env["PATH"] = f"{pup.parent}{os.pathsep}{env.get('PATH', '')}"
        checked = run_command(
            ["cargo", f"+{toolchain}", "check", "--locked", "--all-targets"],
            project, logs / f"{case.name}.compile.log", env,
        )
        if checked.returncode != 0:
            raise AssertionError(
                "Fixture must compile without pup before testing its architecture.\n"
                + checked.stdout[-5000:]
            )
        result = run_command(
            [str(pup), "check", "--locked", "--all-targets"],
            project, logs / f"{case.name}.pup.log", env,
        )
        verify_pup_result(case, result)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=[case.name for case in CASES])
    args = parser.parse_args()
    config = tomllib.loads((ROOT / "pup-toolchain.toml").read_text(encoding="utf-8"))
    pup = ROOT / ".tools" / "cargo-pup" / "bin" / "cargo-pup"
    if not pup.is_file():
        parser.error("Run python3 scripts/setup_pup.py first.")
    env = dict(os.environ, CARGO_TERM_COLOR="never", NO_COLOR="1")
    version = subprocess.run(
        [str(pup), "--version"], text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, timeout=30, check=True, env=env,
    ).stdout
    plain_version = re.sub(r"\x1b\[[0-9;]*m", "", version).strip()
    if plain_version != f"cargo-pup version {config['version']}":
        parser.error(f"Unexpected cargo-pup version: {plain_version!r}. Run setup_pup.py.")
    print(f"{plain_version}; {config['toolchain']}", flush=True)
    cases = [case for case in CASES if not args.case or case.name == args.case]
    logs = ROOT / ".test-artifacts" / "architecture"
    logs.mkdir(parents=True, exist_ok=True)
    rows = []
    failed = 0
    for case in cases:
        try:
            run_case(case, pup, config["toolchain"], logs)
            outcome = "KNOWN GAP confirmed (not protection)" if case.known_gap else "PASS"
        except (AssertionError, OSError, RuntimeError) as error:
            failed += 1
            outcome = "FAIL"
            print(f"{case.name}: {error}", flush=True)
        print(f"{outcome}: {case.name}", flush=True)
        rows.append(f"| `{case.name}` | {outcome} |")
    summary = "\n".join([
        "## Architecture regression suite", "",
        "Violating fixtures must compile normally, then fail with the expected named cargo-pup diagnostic.",
        "The fully-qualified-path case is a documented blind spot, NOT an allowed dependency.", "",
        "| Case | Result |", "| --- | --- |", *rows, "",
        f"Cases: {len(cases)}. Failures: {failed}.", "",
    ])
    (logs / "summary.md").write_text(summary, encoding="utf-8")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as handle:
            handle.write(summary)
    print(f"Logs: {logs}", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
