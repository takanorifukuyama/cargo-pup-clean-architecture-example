use super::runtime::{execute, Project, TestResult};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Fixture {
    _project: Project,
    pub workspace: PathBuf,
    pub logs: PathBuf,
}

fn copy(source: &Path, destination: &Path) -> TestResult {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name() == "target" {
            continue;
        }
        let target = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err("Symlinks/special files are not supported in fixtures".into());
        }
    }
    Ok(())
}

impl Fixture {
    pub fn new(name: &str) -> TestResult<Self> {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("Invalid fixture name".into());
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        let project = Project::new()?;
        let workspace = project.0.join("workspace");
        copy(&root.join("examples/multi-crate"), &workspace)?;
        let logs = root.join(".test-artifacts/dependency-contracts").join(name);
        fs::create_dir_all(&logs)?;
        Ok(Self {
            _project: project,
            workspace,
            logs,
        })
    }

    pub fn cargo(&self, phase: &str, args: &[&str]) -> TestResult<(Option<i32>, String)> {
        execute(
            Command::new("rustup")
                .args(["run", "stable", "cargo"])
                .args(args)
                .current_dir(&self.workspace),
            &self.logs.join(format!("{phase}.log")),
        )
    }

    pub fn succeed(&self, phase: &str, args: &[&str]) -> TestResult {
        let (code, output) = self.cargo(phase, args)?;
        if code != Some(0) {
            return Err(format!("{phase} must succeed before policy evaluation:\n{output}").into());
        }
        Ok(())
    }

    pub fn compile_mutation(&self) -> TestResult {
        // Only the disposable copy may update its lock after a manifest mutation.
        self.succeed("lock", &["generate-lockfile", "--offline", "--quiet"])?;
        self.succeed(
            "compile",
            &[
                "check",
                "--workspace",
                "--all-targets",
                "--locked",
                "--offline",
                "--quiet",
            ],
        )
    }

    pub fn metadata(&self) -> TestResult<Value> {
        let (code, output) = self.cargo(
            "metadata",
            &[
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--locked",
                "--offline",
                "--quiet",
            ],
        )?;
        if code != Some(0) {
            return Err(format!("cargo metadata failed:\n{output}").into());
        }
        // Never use resolve.nodes: it can omit inactive optional dependencies.
        let metadata: Value = serde_json::from_str(&output)?;
        fs::write(
            self.logs.join("metadata.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;
        Ok(metadata)
    }

    pub fn pass(&self, detail: &str) -> TestResult {
        fs::write(self.logs.join("result.txt"), format!("PASS: {detail}\n"))?;
        println!("{detail}; logs: {}", self.logs.display());
        Ok(())
    }

    pub fn replace(&self, file: &str, before: &str, after: &str) -> TestResult {
        let path = self.workspace.join(file);
        let text = fs::read_to_string(&path)?;
        if before.is_empty() || text.matches(before).count() != 1 {
            return Err(format!("Expected one replacement in {file}").into());
        }
        fs::write(path, text.replacen(before, after, 1))?;
        Ok(())
    }

    pub fn dependency(&self, layer: &str, table: &str, entry: &str) -> TestResult {
        let path = self.workspace.join(format!("crates/{layer}/Cargo.toml"));
        let text = fs::read_to_string(&path)?;
        let heading = format!("[{table}]\n");
        let text = if text.contains(&heading) {
            if text.matches(&heading).count() != 1 {
                return Err("Duplicate dependency table in fixture".into());
            }
            text.replacen(&heading, &format!("{heading}{entry}\n"), 1)
        } else {
            format!("{text}\n{heading}{entry}\n")
        };
        fs::write(path, text)?;
        Ok(())
    }

    pub fn probe(&self, package: &str, version: &str) -> TestResult {
        // Outside the inspected workspace; does not introduce a Cargo cycle.
        let path = self
            .workspace
            .parent()
            .ok_or("Missing parent")?
            .join("probe");
        fs::create_dir_all(path.join("src"))?;
        fs::write(path.join("Cargo.toml"), format!(
            "[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2021\"\npublish = false\n\n[workspace]\n"
        ))?;
        fs::write(path.join("src/lib.rs"), "pub struct Probe;\n")?;
        Ok(())
    }
}
