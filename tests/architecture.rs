//! Tests for the architecture rules themselves, including known false negatives.
//! Run explicitly after installing the pinned cargo-pup:
//! cargo test --test architecture -- --ignored --nocapture --test-threads=1

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> io::Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "cargo-pup-clean-architecture-{}-{timestamp}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        for file in ["Cargo.toml", "Cargo.lock", "pup.ron", "rust-toolchain.toml"] {
            fs::copy(source.join(file), root.join(file))?;
        }
        copy_directory(&source.join("src"), &root.join("src"))?;
        Ok(Self { root })
    }

    fn append(&self, file: &str, source: &str) -> io::Result<()> {
        let mut output = fs::OpenOptions::new()
            .append(true)
            .open(self.root.join(file))?;
        writeln!(output, "\n{source}")
    }

    fn cargo(&self, arguments: &[&str]) -> io::Result<Output> {
        Command::new("cargo")
            .args(arguments)
            .current_dir(&self.root)
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_NET_OFFLINE", "true")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .output()
    }

    fn assert_compiles(&self) -> TestResult {
        // Check with the same compiler used by pup. A compile error is NOT proof
        // of a working architecture rule. These fixtures have no dependencies.
        let output = self.cargo(&["+nightly-2026-01-22", "check", "--locked", "--offline"])?;
        assert!(
            output.status.success(),
            "fixture must compile before linting: {}\n{}",
            self.root.display(),
            transcript(&output)
        );
        Ok(())
    }

    fn assert_pup_accepts(&self) -> TestResult {
        self.assert_compiles()?;
        let output = self.cargo(&["pup"])?;
        assert!(
            output.status.success(),
            "expected cargo-pup to accept {}\n{}",
            self.root.display(),
            transcript(&output)
        );
        Ok(())
    }

    fn assert_pup_rejects(&self, rule: &str) -> TestResult {
        self.assert_compiles()?;
        let output = self.cargo(&["pup"])?;
        let log = transcript(&output);
        assert!(
            !output.status.success(),
            "expected architecture violation, but cargo-pup passed:\n{log}"
        );
        // Do not accept a missing tool, malformed RON, or unrelated build error.
        assert!(
            log.contains(&format!("Applied by cargo-pup rule '{rule}'."))
                && log.contains("is denied"),
            "expected the named import diagnostic, not an arbitrary failure:\n{log}"
        );
        eprintln!("Confirmed violation of {rule}:\n{log}");
        Ok(())
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Failed fixture retained at {}", self.root.display());
        } else if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("Could not remove {}: {error}", self.root.display());
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::other("fixtures must not contain symlinks"));
        }
    }
    Ok(())
}

fn transcript(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

macro_rules! rejects_import {
    ($name:ident, $layer:literal, $target:literal, $rule:literal) => {
        #[test]
        #[ignore = "requires the pinned cargo-pup and nightly toolchain"]
        fn $name() -> TestResult {
            let sandbox = Sandbox::new()?;
            sandbox.append(
                concat!("src/", $layer, ".rs"),
                concat!("pub use crate::", $target, " as ArchitectureProbe;"),
            )?;
            sandbox.assert_pup_rejects($rule)
        }
    };
}

rejects_import!(
    domain_rejects_application,
    "domain",
    "application::PlaceOrderInput",
    "domain_no_outer_imports"
);
rejects_import!(
    domain_rejects_infrastructure,
    "domain",
    "infrastructure::InMemoryOrderRepository",
    "domain_no_outer_imports"
);
rejects_import!(
    domain_rejects_presentation,
    "domain",
    "presentation::submit_order",
    "domain_no_outer_imports"
);
rejects_import!(
    application_rejects_infrastructure,
    "application",
    "infrastructure::InMemoryOrderRepository",
    "application_no_adapter_imports"
);
rejects_import!(
    application_rejects_presentation,
    "application",
    "presentation::submit_order",
    "application_no_adapter_imports"
);
rejects_import!(
    infrastructure_rejects_presentation,
    "infrastructure",
    "presentation::submit_order",
    "infrastructure_no_presentation_imports"
);
rejects_import!(
    presentation_rejects_infrastructure,
    "presentation",
    "infrastructure::InMemoryOrderRepository",
    "presentation_no_infrastructure_imports"
);

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn accepts_clean_architecture() -> TestResult {
    Sandbox::new()?.assert_pup_accepts()
}

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn rejects_outer_import_in_nested_domain_module() -> TestResult {
    let sandbox = Sandbox::new()?;
    sandbox.append(
        "src/domain.rs",
        "pub mod nested { pub use crate::infrastructure::InMemoryOrderRepository; }",
    )?;
    sandbox.assert_pup_rejects("domain_no_outer_imports")
}

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn rejects_relative_outer_import() -> TestResult {
    let sandbox = Sandbox::new()?;
    sandbox.append(
        "src/domain.rs",
        "pub use super::infrastructure::InMemoryOrderRepository;",
    )?;
    sandbox.assert_pup_rejects("domain_no_outer_imports")
}

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn rejects_grouped_outer_import() -> TestResult {
    let sandbox = Sandbox::new()?;
    sandbox.append(
        "src/domain.rs",
        "pub use crate::{application::Receipt, infrastructure::InMemoryOrderRepository};",
    )?;
    sandbox.assert_pup_rejects("domain_no_outer_imports")
}

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn known_gap_fully_qualified_path_is_not_detected() -> TestResult {
    let sandbox = Sandbox::new()?;
    sandbox.append(
        "src/domain.rs",
        "pub fn architecture_probe() {\n\
         let _repository = crate::infrastructure::InMemoryOrderRepository::default();\n\
         }",
    )?;
    // Architecturally WRONG, but no `use` item exists for RestrictImports to see.
    // An upstream improvement should break this characterization test.
    sandbox.assert_pup_accepts()
}

#[test]
#[ignore = "requires the pinned cargo-pup and nightly toolchain"]
fn known_gap_root_reexport_is_not_resolved() -> TestResult {
    let sandbox = Sandbox::new()?;
    sandbox.append(
        "src/lib.rs",
        "pub use infrastructure::InMemoryOrderRepository as Database;",
    )?;
    sandbox.append("src/domain.rs", "pub use crate::Database;")?;
    // The `use` path no longer contains `infrastructure`. It is not resolved.
    sandbox.assert_pup_accepts()
}
