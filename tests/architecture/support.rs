//! Sample-local facade over cargo-pup, with fail-closed coverage probes.
//! Rules remain Rust declarations; RON and compiler workspaces are temporary.

#[path = "../common/runtime.rs"]
mod runtime;

pub use runtime::TestResult;
use runtime::{execute, Project};
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};
use std::process::Command;

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

macro_rules! visibility_rules {
    ($($name:ident {
        struct_name: $structure:ident,
        visibility: $visibility:ident,
    })+) => {
        const VISIBILITY_RULES: &[$crate::support::VisibilityRule] = &[
            $($crate::support::VisibilityRule {
                name: stringify!($name),
                struct_name: stringify!($structure),
                visibility: $crate::support::Visibility::$visibility,
            }),+
        ];
        $(#[test]
        fn $name() -> $crate::support::TestResult {
            let rule = VISIBILITY_RULES.iter()
                .find(|rule| rule.name == stringify!($name))
                .ok_or("Missing visibility rule")?;
            $crate::support::check_visibility(
                stringify!($name), rule, None, $crate::support::Expectation::Pass,
            )
        })+
    };
}

pub(crate) use architecture_rules;
pub(crate) use architecture_violation;
pub(crate) use visibility_rules;

pub struct Rule {
    pub name: &'static str,
    pub module: &'static str,
    pub deny_imports: &'static [&'static str],
}

pub struct Edit {
    pub file: &'static str,
    pub code: &'static str,
}

pub struct Replacement {
    pub file: &'static str,
    pub before: &'static str,
    pub after: &'static str,
}

pub enum Visibility {
    Private,
    Public,
    PubCrate,
}

pub struct VisibilityRule {
    pub name: &'static str,
    // cargo-pup 0.1.8 matches the SHORT struct name, not a resolved type path.
    pub struct_name: &'static str,
    pub visibility: Visibility,
}

#[derive(Clone, Copy)]
pub enum Expectation {
    Pass,
    Denied,
    KnownGap,
    MissingTarget,
}

#[derive(Debug)]
struct MissingTarget(String);

impl fmt::Display for MissingTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "No coverage evidence for {}: missing/cfg-disabled target or suppressed canary", self.0)
    }
}

impl Error for MissingTarget {}

#[derive(Clone, Copy)]
enum Lint<'a> {
    Imports(&'a Rule),
    Visibility(&'a VisibilityRule),
}

enum Mutation {
    Append(Edit),
    Replace(Replacement),
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
    Ok(format!(
        "(lints: [Module((name: {:?}, matches: Module({:?}), rules: [RestrictImports(allowed_only: None, denied: Some([{denied}]), severity: Error)]))])\n",
        rule.name,
        format!("^{module}(::|$)"),
    ))
}

impl Lint<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::Imports(rule) => rule.name,
            Self::Visibility(rule) => rule.name,
        }
    }

    fn config(self, coverage: bool) -> TestResult<String> {
        normalized_path(self.name())?;
        let name = if coverage {
            format!("coverage_{}", self.name())
        } else {
            self.name().to_owned()
        };
        match self {
            Self::Imports(rule) => {
                let actual = configuration(rule)?;
                if !coverage {
                    return Ok(actual);
                }
                let module = normalized_path(rule.module)?;
                Ok(format!(
                    "(lints: [Module((name: {name:?}, matches: Module({:?}), rules: [MustBeNamed(\"^$\", Error)]))])\n",
                    format!("^{module}(::|$)"),
                ))
            }
            Self::Visibility(rule) => {
                let structure = normalized_path(rule.struct_name)?;
                if structure.contains("::") {
                    return Err("Struct selectors accept a short name, not a type path".into());
                }
                let requirement = if coverage {
                    "MustBeNamed(\"^$\", Error)"
                } else {
                    match rule.visibility {
                        Visibility::Private => "MustBePrivate(Error)",
                        Visibility::Public => "MustBePublic(Error)",
                        Visibility::PubCrate => "MustBePubCrate(Error)",
                    }
                };
                Ok(format!(
                    "(lints: [Struct((name: {name:?}, matches: Name({:?}), rules: [{requirement}]))])\n",
                    format!("^{structure}$"),
                ))
            }
        }
    }

    fn denial_tokens(self) -> Vec<String> {
        match self {
            Self::Imports(_) => vec!["Use of module".into(), "is denied".into()],
            Self::Visibility(rule) => vec![
                format!("Struct '{}'", rule.struct_name),
                match rule.visibility {
                    Visibility::Private => "must be private",
                    Visibility::Public => "must be pub",
                    Visibility::PubCrate => "must be pub(crate)",
                }
                .into(),
            ],
        }
    }
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

// Require the rule and its diagnostic in the SAME error, not a warning elsewhere.
fn named_error(code: Option<i32>, output: &str, rule: &str, tokens: &[&str]) -> bool {
    if !code.is_some_and(|value| value > 0)
        || output.contains("internal compiler error")
        || output.contains("panicked at")
    {
        return false;
    }
    let output = strip_ansi(output);
    let mut blocks = Vec::new();
    let mut block = String::new();
    for line in output.lines() {
        if line.starts_with("error:")
            || line.starts_with("error[")
            || line.starts_with("warning:")
            || line.starts_with("warning[")
        {
            blocks.push(std::mem::take(&mut block));
        }
        block.push_str(line);
        block.push('\n');
    }
    blocks.push(block);
    let note = format!("Applied by cargo-pup rule '{rule}'.");
    let inline = format!("error: {rule}:");
    blocks.iter().any(|block| {
        (block.starts_with("error:") || block.starts_with("error["))
            && (block.contains(&note) || block.starts_with(&inline))
            && tokens.iter().all(|token| block.contains(token))
    })
}

fn verify(expected: Expectation, rule: &str, code: Option<i32>, output: &str) -> TestResult {
    let valid = match expected {
        Expectation::Pass | Expectation::KnownGap => code == Some(0),
        Expectation::Denied => named_error(code, output, rule, &["Use of module", "is denied"]),
        Expectation::MissingTarget => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!("Unexpected cargo-pup result for {rule}, exit={code:?}:\n{output}").into())
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

fn replace_once(text: &str, before: &str, after: &str) -> TestResult<String> {
    if before.is_empty() || text.matches(before).count() != 1 {
        return Err("A mutation must replace exactly one occurrence".into());
    }
    Ok(text.replacen(before, after, 1))
}

fn mutate(project: &Path, mutation: Mutation) -> TestResult {
    let file = match &mutation {
        Mutation::Append(edit) => edit.file,
        Mutation::Replace(edit) => edit.file,
    };
    let path = Path::new(file);
    if !path.components().all(|part| matches!(part, Component::Normal(_)))
        || !path.starts_with("src")
    {
        return Err(format!("Mutation must name a relative file under src/: {file}").into());
    }
    let path = project.join(path);
    match mutation {
        Mutation::Append(edit) => {
            writeln!(OpenOptions::new().append(true).open(path)?, "\n{}", edit.code)?;
        }
        Mutation::Replace(edit) => {
            let updated = replace_once(&fs::read_to_string(&path)?, edit.before, edit.after)?;
            fs::write(path, updated)?;
        }
    }
    Ok(())
}

fn run(
    root: &Path,
    logs: &Path,
    name: &str,
    lint: Lint<'_>,
    mutation: Option<Mutation>,
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
        return Err(format!("Wrong cargo-pup version; rerun scripts/setup_pup.py:\n{output}").into());
    }
    let project = Project::new()?;
    for file in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "pup-toolchain.toml"] {
        fs::copy(root.join(file), project.0.join(file))?;
    }
    for directory in ["src", "tests"] {
        copy_directory(&root.join(directory), &project.0.join(directory))?;
    }
    if let Some(mutation) = mutation {
        mutate(&project.0, mutation)?;
    }
    // Compile all default-feature targets first; never recursively execute tests.
    let (code, output) = execute(
        Command::new("rustup")
            .args(["run", toolchain, "cargo", "check", "--locked", "--all-targets", "--target-dir"])
            .arg(project.0.join("target"))
            .current_dir(&project.0),
        &logs.join(format!("{name}.compile.log")),
    )?;
    if code != Some(0) {
        return Err(format!("Fixture must compile normally before checking architecture:\n{output}").into());
    }
    let mut paths = vec![pup.parent().ok_or("Missing tool directory")?.to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    let search_path = std::env::join_paths(paths)?;
    // Same selector, same source and same --lib scope. ^$ cannot match a Rust name.
    // No .pup reuse: cargo-pup does not reliably invalidate on RON changes alone.
    for coverage in [true, false] {
        let phase = if coverage { "coverage" } else { "pup" };
        let config = lint.config(coverage)?;
        fs::write(project.0.join("pup.ron"), &config)?;
        fs::write(logs.join(format!("{name}.{phase}.ron")), config)?;
        let cache = project.0.join(".pup");
        if cache.exists() {
            fs::remove_dir_all(cache)?;
        }
        let (code, output) = execute(
            Command::new(&pup)
                .args(["check", "--locked", "--lib"])
                .env("PATH", &search_path)
                .current_dir(&project.0),
            &logs.join(format!("{name}.{phase}.log")),
        )?;
        if coverage {
            if code == Some(0) {
                return Err(Box::new(MissingTarget(lint.name().to_owned())));
            }
            let subject = match lint {
                Lint::Imports(_) => "Module must match",
                Lint::Visibility(_) => "Struct must match",
            };
            if !named_error(code, &output, &format!("coverage_{}", lint.name()), &[subject, "^$"]) {
                return Err(format!("Coverage probe failed for an unexpected reason:\n{output}").into());
            }
        } else {
            match lint {
                Lint::Imports(rule) => verify(expected, rule.name, code, &output)?,
                Lint::Visibility(_) => {
                    let tokens = lint.denial_tokens();
                    let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
                    let valid = match expected {
                        Expectation::Pass => code == Some(0),
                        Expectation::Denied => named_error(code, &output, lint.name(), &tokens),
                        _ => false,
                    };
                    if !valid {
                        return Err(format!("Unexpected visibility result for {}:\n{output}", lint.name()).into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_lint(name: &str, lint: Lint<'_>, mutation: Option<Mutation>, expected: Expectation) -> TestResult {
    normalized_path(name)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let logs = root.join(".test-artifacts/architecture");
    fs::create_dir_all(&logs)?;
    let result = run(root, &logs, name, lint, mutation, expected);
    let result = if matches!(expected, Expectation::MissingTarget) {
        match result {
            Err(error) if error.downcast_ref::<MissingTarget>().is_some() => Ok(()),
            Err(error) => Err(error),
            Ok(()) => Err("An empty selector was unexpectedly accepted".into()),
        }
    } else {
        result
    };
    let outcome = match (&result, expected) {
        (Err(_), _) => "FAIL",
        (Ok(()), Expectation::KnownGap) => "KNOWN GAP confirmed (not protection)",
        (Ok(()), Expectation::MissingTarget) => "PASS (missing target rejected)",
        (Ok(()), _) => "PASS",
    };
    fs::write(logs.join(format!("{name}.result.md")), format!("| `{name}` | {outcome} |\n"))?;
    println!("{outcome}: {name}; logs: {}", logs.display());
    result
}

pub fn check(name: &str, rule: &Rule, edit: Option<Edit>, expected: Expectation) -> TestResult {
    check_lint(name, Lint::Imports(rule), edit.map(Mutation::Append), expected)
}

pub fn check_visibility(
    name: &str,
    rule: &VisibilityRule,
    edit: Option<Replacement>,
    expected: Expectation,
) -> TestResult {
    check_lint(name, Lint::Visibility(rule), edit.map(Mutation::Replace), expected)
}

#[cfg(test)]
#[path = "support_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_tests.rs"]
mod guard_tests;
