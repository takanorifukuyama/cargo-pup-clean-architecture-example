//! Small, sample-local macro facade over the cargo-pup CLI, not a new analyzer.
//! RON is generated in a fresh fixture; it is never a second source of truth.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

macro_rules! architecture_rules {
    ($($name:ident {
        module: $module:path,
        deny_imports: [$($denied:path),+ $(,)?],
    })+) => {
        const RULES: &[$crate::support::Rule] = &[
            $($crate::support::Rule {
                name: stringify!($name),
                module: stringify!($module),
                deny_imports: &[$(stringify!($denied)),+],
            }),+
        ];

        $(#[test]
        fn $name() -> $crate::support::TestResult {
            $crate::support::check(
                stringify!($name),
                $crate::support::find_rule(RULES, stringify!($name))?,
                None,
                $crate::support::Expectation::Pass,
            )
        })+
    };
}

macro_rules! architecture_violation {
    ($name:ident {
        rule: $rule:ident,
        in_file: $file:literal,
        add: { $($code:tt)* }
    }) => {
        #[test]
        fn $name() -> $crate::support::TestResult {
            $crate::support::check(
                stringify!($name),
                $crate::support::find_rule($crate::RULES, stringify!($rule))?,
                Some($crate::support::Edit {
                    file: $file,
                    code: stringify!($($code)*),
                }),
                $crate::support::Expectation::Denied,
            )
        }
    };
}

pub(crate) use architecture_rules;
pub(crate) use architecture_violation;

pub struct Rule {
    pub name: &'static str,
    pub module: &'static str,
    pub deny_imports: &'static [&'static str],
}

pub struct Edit {
    pub file: &'static str,
    pub code: &'static str,
}

#[derive(Clone, Copy)]
pub enum Expectation {
    Pass,
    Denied,
    KnownGap,
}

pub fn find_rule<'a>(rules: &'a [Rule], name: &str) -> TestResult<&'a Rule> {
    let mut matches = rules.iter().filter(|rule| rule.name == name);
    let rule = matches
        .next()
        .ok_or_else(|| format!("Unknown rule: {name}"))?;
    if matches.next().is_some() {
        return Err(format!("Duplicate rule: {name}").into());
    }
    Ok(rule)
}

// Only simple ASCII paths are supported. Reject generic/raw/Unicode paths instead
// of accidentally treating user input as a regular expression or RON syntax.
fn normalized_path(path: &str) -> TestResult<String> {
    let path: String = path.chars().filter(|ch| !ch.is_whitespace()).collect();
    let valid = path.split("::").all(|segment| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    });
    if !valid {
        return Err(format!("Expected a simple ASCII Rust path, got {path:?}").into());
    }
    Ok(path)
}

fn import_pattern(path: &str) -> TestResult<String> {
    let path = normalized_path(path)?;
    // Preserve the original policy's syntactic matching of crate/super imports.
    // This deliberately does NOT claim resolved dependency-graph semantics.
    Ok(match path.strip_prefix("crate::") {
        Some(local) => format!("(^|::){local}(::|$)"),
        None => format!("^{path}(::|$)"),
    })
}

fn configuration(rule: &Rule) -> TestResult<String> {
    let module = normalized_path(rule.module)?;
    normalized_path(rule.name)?;
    if rule.deny_imports.is_empty() {
        return Err("An architecture rule must forbid at least one import".into());
    }
    let denied = rule
        .deny_imports
        .iter()
        .map(|path| import_pattern(path).map(|pattern| format!("{pattern:?}")))
        .collect::<TestResult<Vec<_>>>()?
        .join(", ");
    // The validated path alphabet cannot inject regex metacharacters or quotes.
    Ok(format!(
        "(lints: [Module((name: {:?}, matches: Module({:?}), rules: [RestrictImports(allowed_only: None, denied: Some([{denied}]), severity: Error)]))])\n",
        rule.name,
        format!("^{module}(::|$)"),
    ))
}

fn setting<'a>(text: &'a str, key: &str) -> TestResult<&'a str> {
    let mut values = text.lines().filter_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then_some(value.trim())
    });
    let value = values.next().ok_or_else(|| format!("Missing pin: {key}"))?;
    if values.next().is_some() {
        return Err(format!("Duplicate pin: {key}").into());
    }
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(|| format!("Pin must be a quoted string: {key}"))?;
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "-._".contains(ch))
    {
        return Err(format!("Invalid pin: {key}").into());
    }
    Ok(value)
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn verify(expected: Expectation, rule: &str, code: Option<i32>, output: &str) -> TestResult {
    let output = strip_ansi(output);
    let valid = match expected {
        Expectation::Pass | Expectation::KnownGap => code == Some(0),
        Expectation::Denied => {
            code.is_some_and(|value| value > 0)
                && [rule, "Use of module", "is denied"]
                    .iter()
                    .all(|token| output.contains(token))
                && output.lines().any(|line| {
                    let line = line.trim_start();
                    line.starts_with("error:") || line.starts_with("error[")
                })
                && !output.contains("internal compiler error")
                && !output.contains("panicked at")
        }
    };
    if valid {
        Ok(())
    } else {
        Err(format!("Unexpected cargo-pup result for {rule}, exit={code:?}:\n{output}").into())
    }
}

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> TestResult<Self> {
        let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pup-rust-{}-{time}-{id}", std::process::id()));
        // create_dir fails if the path already exists: never adopt someone else's directory.
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_directory(source: &Path, target: &Path) -> TestResult {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(format!("Unsupported fixture entry: {}", entry.path().display()).into());
        }
    }
    Ok(())
}

// File-backed output avoids pipe deadlocks. Each command has a bounded runtime.
fn execute(command: &mut Command, log: &Path) -> TestResult<(Option<i32>, String)> {
    let file = File::create(log)?;
    command
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file));
    command
        .env("CARGO_TERM_COLOR", "never")
        .env("NO_COLOR", "1");
    // Do not inherit an outer cargo test's target directory / rustc wrapper.
    for name in [
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180) {
            #[cfg(unix)]
            {
                // Kill cargo and its rustc/pup descendants, not only the parent.
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Command timed out: {command:?}; log: {}", log.display()).into());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = String::from_utf8_lossy(&fs::read(log)?).into_owned();
    writeln!(
        OpenOptions::new().append(true).open(log)?,
        "\nCOMMAND: {command:?}\nEXIT: {:?}",
        status.code()
    )?;
    Ok((status.code(), output))
}

fn run(
    root: &Path,
    logs: &Path,
    name: &str,
    rule: &Rule,
    edit: Option<Edit>,
    expected: Expectation,
) -> TestResult {
    let pins = include_str!("../../pup-toolchain.toml");
    let version = setting(pins, "version")?;
    let toolchain = setting(pins, "toolchain")?;
    let pup = root
        .join(".tools/cargo-pup/bin")
        .join(format!("cargo-pup{}", std::env::consts::EXE_SUFFIX));
    if !pup.is_file() {
        return Err("cargo-pup is not installed. Run: python3 scripts/setup_pup.py".into());
    }
    let (code, output) = execute(
        Command::new(&pup).arg("--version"),
        &logs.join(format!("{name}.version.log")),
    )?;
    if code != Some(0) || strip_ansi(&output).trim() != format!("cargo-pup version {version}") {
        return Err(
            format!("Wrong cargo-pup version; rerun scripts/setup_pup.py:\n{output}").into(),
        );
    }
    let project = Project::new()?;
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "pup-toolchain.toml",
    ] {
        fs::copy(root.join(file), project.0.join(file))?;
    }
    for directory in ["src", "tests"] {
        copy_directory(&root.join(directory), &project.0.join(directory))?;
    }
    let config = configuration(rule)?;
    fs::write(project.0.join("pup.ron"), &config)?;
    fs::write(logs.join(format!("{name}.pup.ron")), config)?;
    if let Some(edit) = edit {
        let path = Path::new(edit.file);
        if !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            || !path.starts_with("src")
        {
            return Err(format!(
                "Mutation must name a relative file under src/: {}",
                edit.file
            )
            .into());
        }
        let mut file = OpenOptions::new().append(true).open(project.0.join(path))?;
        writeln!(file, "\n{}", edit.code)?;
    }
    // Compiling all default-feature targets must succeed BEFORE the lint runs.
    // We never invoke cargo test recursively. The feature-gated runner is not compiled here.
    let (code, output) = execute(
        Command::new("rustup")
            .args([
                "run",
                toolchain,
                "cargo",
                "check",
                "--locked",
                "--all-targets",
                "--target-dir",
            ])
            .arg(project.0.join("target"))
            .current_dir(&project.0),
        &logs.join(format!("{name}.compile.log")),
    )?;
    if code != Some(0) {
        return Err(format!(
            "Fixture must compile normally before checking architecture:\n{output}"
        )
        .into());
    }
    let mut paths = vec![pup.parent().ok_or("Missing tool directory")?.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let (code, output) = execute(
        Command::new(&pup)
            .args(["check", "--locked", "--all-targets"])
            .env("PATH", std::env::join_paths(paths)?)
            .current_dir(&project.0),
        &logs.join(format!("{name}.pup.log")),
    )?;
    verify(expected, rule.name, code, &output)
}

pub fn check(name: &str, rule: &Rule, edit: Option<Edit>, expected: Expectation) -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let logs = root.join(".test-artifacts/architecture");
    fs::create_dir_all(&logs)?;
    let result = run(root, &logs, name, rule, edit, expected);
    let outcome = match (&result, expected) {
        (Err(_), _) => "FAIL",
        (Ok(()), Expectation::KnownGap) => "KNOWN GAP confirmed (not protection)",
        (Ok(()), _) => "PASS",
    };
    // One file per case: libtest's parallel execution cannot interleave summary writes.
    fs::write(
        logs.join(format!("{name}.result.md")),
        format!("| `{name}` | {outcome} |\n"),
    )?;
    println!("{outcome}: {name}; logs: {}", logs.display());
    result
}

#[cfg(test)]
#[path = "support_tests.rs"]
mod tests;
