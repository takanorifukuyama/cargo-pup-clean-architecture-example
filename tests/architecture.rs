//! Black-box tests of cargo-pup itself, using the SAME pup.ron as the app.
//! Every negative fixture must compile with ordinary Rust first and then fail
//! with its expected named lint. A missing tool or syntax error is NOT a pass.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const NIGHTLY: &str = "+nightly-2026-01-22";
const CONFIG: &str = include_str!("../pup.ron");
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct ScratchProject {
    path: PathBuf,
}

impl ScratchProject {
    fn new() -> io::Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-pup-example-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        let project = Self { path };
        fs::create_dir(project.path.join("src"))?;
        fs::write(project.path.join("pup.ron"), CONFIG)?;
        Ok(project)
    }

    fn fixture(source: &str) -> io::Result<Self> {
        let project = Self::new()?;
        fs::write(
            project.path.join("Cargo.toml"),
            "[package]\nname = \"architecture-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\nname = \"clean_architecture\"\n",
        )?;
        fs::write(project.path.join("src/lib.rs"), source)?;
        Ok(project)
    }

    fn example() -> io::Result<Self> {
        let project = Self::new()?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        fs::copy(root.join("Cargo.toml"), project.path.join("Cargo.toml"))?;
        fs::copy(root.join("Cargo.lock"), project.path.join("Cargo.lock"))?;
        copy_directory(&root.join("src"), &project.path.join("src"))?;
        Ok(project)
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = Command::new("cargo")
            .args(args)
            .current_dir(&self.path)
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_TARGET_DIR", self.path.join("target"))
            .output()
            .expect("could not run cargo; install Rust with rustup first");
        println!("$ cargo {}\n{}", args.join(" "), output_text(&output));
        output
    }

    fn assert_compiles(&self) {
        let output = self.run(&[NIGHTLY, "check", "--lib", "--offline"]);
        assert!(
            output.status.success(),
            "fixture must compile BEFORE linting; this is not an architecture violation:\n{}",
            output_text(&output)
        );
    }

    fn lint(&self) -> Output {
        self.run(&["pup", "--lib", "--offline"])
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Kept failing fixture at {}", self.path.display());
        } else {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
#[ignore = "requires cargo-pup 0.1.8; run bash scripts/install-cargo-pup.sh, then cargo architecture-test"]
fn architecture_contracts_and_known_limitations() -> io::Result<()> {
    let example = ScratchProject::example()?;
    example.assert_compiles();
    let output = example.lint();
    assert!(
        output.status.success(),
        "the actual example must pass cargo-pup; check installation/configuration:\n{}",
        output_text(&output)
    );
    println!("PASS: actual application respects the configured rules");

    let violations = [
        (
            "domain -> infrastructure",
            include_str!("fixtures/domain_to_infrastructure.rs"),
            "domain_no_outer_imports",
        ),
        (
            "application -> infrastructure",
            include_str!("fixtures/application_to_infrastructure.rs"),
            "application_no_adapters",
        ),
        (
            "presentation -> infrastructure",
            include_str!("fixtures/presentation_to_infrastructure.rs"),
            "presentation_no_infrastructure",
        ),
        (
            "infrastructure -> presentation",
            include_str!("fixtures/infrastructure_to_presentation.rs"),
            "infrastructure_no_presentation",
        ),
        (
            "nested domain -> presentation",
            include_str!("fixtures/nested_domain_to_presentation.rs"),
            "domain_no_outer_imports",
        ),
        (
            "non-public repository adapter",
            include_str!("fixtures/non_public_repository.rs"),
            "repository_adapters_public",
        ),
    ];

    for (name, source, rule) in violations {
        let project = ScratchProject::fixture(source)?;
        project.assert_compiles();
        let output = project.lint();
        let diagnostic = output_text(&output);
        assert!(!output.status.success(), "violation was missed: {name}");
        let expected = format!("Applied by cargo-pup rule '{rule}'.");
        assert!(
            diagnostic.contains(&expected),
            "{name} failed for the wrong reason; expected {expected}:\n{diagnostic}"
        );
        println!("PASS: {name} rejected by {rule}");
    }

    // Characterization tests, NOT allowed design patterns. If upstream starts
    // rejecting either case, inspect the new diagnostics and update the docs.
    let known_limitations = [
        (
            "fully qualified dependency bypasses import restrictions",
            include_str!("fixtures/fully_qualified_bypass.rs"),
        ),
        (
            "StructRule::ImplementsTrait does not enforce trait implementation",
            include_str!("fixtures/missing_repository_trait.rs"),
        ),
    ];
    for (name, source) in known_limitations {
        let project = ScratchProject::fixture(source)?;
        project.assert_compiles();
        let output = project.lint();
        assert!(
            output.status.success(),
            "known limitation changed, or tooling failed ({name}); inspect output and update docs:\n{}",
            output_text(&output)
        );
        println!("KNOWN LIMITATION: {name}");
    }
    Ok(())
}
