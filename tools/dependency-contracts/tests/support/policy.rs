//! Inspect DECLARED direct dependencies, not the feature-filtered resolve graph.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Dev,
    Build,
}

#[derive(Clone, Copy)]
pub struct Rule {
    pub name: &'static str,
    pub package: &'static str,
    pub normal: &'static [&'static str],
    pub dev: &'static [&'static str],
    pub build: &'static [&'static str],
}

#[derive(Debug, PartialEq, Eq)]
pub struct DeclarationContext {
    pub alias: Option<String>,
    pub optional: bool,
    pub target: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    InvalidMetadata(String),
    Coverage(String),
    Forbidden {
        rule: String,
        package: String,
        dependency: String,
        kind: Kind,
        context: Box<DeclarationContext>,
    },
    WrongSource {
        package: String,
        dependency: String,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dependency contract failed: {self:?}")
    }
}

impl Error for Problem {}

type Checked<T> = Result<T, Problem>;

fn invalid(message: &str) -> Problem {
    Problem::InvalidMetadata(message.to_owned())
}

fn text<'a>(value: &'a Value, key: &str) -> Checked<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("Missing/string field: {key}")))
}

fn array<'a>(value: &'a Value, key: &str) -> Checked<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(&format!("Missing/array field: {key}")))
}

fn nullable_text(value: &Value) -> Checked<Option<&str>> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        _ => Err(invalid("Expected string or null")),
    }
}

fn required<'a>(value: &'a Value, key: &str) -> Checked<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| invalid(&format!("Missing field: {key}")))
}

// Package IDs are opaque: compare them verbatim, never parse or split them.
fn members(metadata: &Value) -> Checked<BTreeMap<&str, &Value>> {
    if metadata.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("Expected cargo metadata format-version 1"));
    }
    let ids = array(metadata, "workspace_members")?;
    let mut wanted = BTreeSet::new();
    for id in ids {
        let id = id
            .as_str()
            .ok_or_else(|| invalid("Non-string package ID"))?;
        if !wanted.insert(id) {
            return Err(invalid("Duplicate workspace member ID"));
        }
    }
    if wanted.is_empty() {
        return Err(Problem::Coverage("Workspace has no members".into()));
    }
    let mut found = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for package in array(metadata, "packages")? {
        let id = text(package, "id")?;
        if !seen.insert(id) {
            return Err(invalid("Duplicate package ID"));
        }
        if wanted.contains(id) {
            let name = text(package, "name")?;
            text(package, "manifest_path")?;
            array(package, "dependencies")?;
            if found.insert(name, package).is_some() {
                return Err(invalid("Ambiguous workspace package name"));
            }
        }
    }
    if found.len() != wanted.len() {
        return Err(invalid("Missing workspace package records"));
    }
    Ok(found)
}

pub fn coverage(metadata: &Value, rules: &[Rule]) -> Checked<()> {
    let packages = members(metadata)?;
    let mut covered = BTreeSet::new();
    let mut names = BTreeSet::new();
    for rule in rules {
        if rule.name.is_empty() || !names.insert(rule.name) {
            return Err(Problem::Coverage("Empty/duplicate rule name".into()));
        }
        if !packages.contains_key(rule.package) || !covered.insert(rule.package) {
            return Err(Problem::Coverage(format!(
                "Unknown/duplicate package: {}",
                rule.package
            )));
        }
        for allowed in [rule.normal, rule.dev, rule.build] {
            let mut seen = BTreeSet::new();
            for dependency in allowed {
                if !packages.contains_key(dependency) || !seen.insert(dependency) {
                    return Err(Problem::Coverage(format!(
                        "Unknown/duplicate allowance: {dependency}"
                    )));
                }
            }
        }
    }
    for name in packages.keys() {
        if !covered.contains(name) {
            return Err(Problem::Coverage(format!(
                "Unclassified workspace member: {name}"
            )));
        }
    }
    Ok(())
}

pub fn check_rule(metadata: &Value, rule: &Rule) -> Checked<()> {
    let packages = members(metadata)?;
    let package = packages
        .get(rule.package)
        .ok_or_else(|| Problem::Coverage(format!("Missing package: {}", rule.package)))?;
    for dependency in array(package, "dependencies")? {
        let name = text(dependency, "name")?;
        let kind = match nullable_text(required(dependency, "kind")?)? {
            None => Kind::Normal,
            Some("dev") => Kind::Dev,
            Some("build") => Kind::Build,
            Some(other) => return Err(invalid(&format!("Unknown dependency kind: {other}"))),
        };
        let alias = nullable_text(required(dependency, "rename")?)?;
        let target = nullable_text(required(dependency, "target")?)?;
        let optional = required(dependency, "optional")?
            .as_bool()
            .ok_or_else(|| invalid("optional must be boolean"))?;
        let allowed = match kind {
            Kind::Normal => rule.normal,
            Kind::Dev => rule.dev,
            Kind::Build => rule.build,
        };
        if !allowed.contains(&name) {
            return Err(Problem::Forbidden {
                rule: rule.name.into(),
                package: rule.package.into(),
                dependency: name.into(),
                kind,
                context: Box::new(DeclarationContext {
                    alias: alias.map(str::to_owned),
                    optional,
                    target: target.map(str::to_owned),
                }),
            });
        }
        // Allowances in this example identify WORKSPACE packages. Same name from
        // a registry, git or a different local path does not satisfy the contract.
        let expected = packages
            .get(name)
            .ok_or_else(|| Problem::Coverage(format!("Unknown allowance: {name}")))?;
        let expected_dir = Path::new(text(expected, "manifest_path")?)
            .parent()
            .ok_or_else(|| invalid("Manifest has no parent"))?;
        let source = nullable_text(required(dependency, "source")?)?;
        let path = nullable_text(dependency.get("path").unwrap_or(&Value::Null))?;
        let same_path = match path {
            Some(path) => {
                let actual = Path::new(path)
                    .canonicalize()
                    .map_err(|_| invalid("Dependency directory unavailable"))?;
                let expected = expected_dir
                    .canonicalize()
                    .map_err(|_| invalid("Workspace directory unavailable"))?;
                actual == expected
            }
            None => false,
        };
        if source.is_some() || !same_path {
            return Err(Problem::WrongSource {
                package: rule.package.into(),
                dependency: name.into(),
            });
        }
    }
    Ok(())
}

pub fn check_all(metadata: &Value, rules: &[Rule]) -> Checked<()> {
    coverage(metadata, rules)?;
    for rule in rules {
        check_rule(metadata, rule)?;
    }
    Ok(())
}
