//! Compiler integration controls. Missing toolchain/driver is an ERROR, never a skip.

#[path = "../../../tests/common/runtime.rs"]
mod runtime;

use layer_contracts::{layer_rules, Finding, Graph, Policy};
use runtime::{execute, Project, TestResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TOOLCHAIN: &str = include_str!("../toolchain.txt");

layer_rules! {
    CLEAN {
        acyclic: true,
        root("clean_architecture") => [domain, application, infrastructure, presentation],
        domain("clean_architecture::domain") => [],
        application("clean_architecture::application") => [domain],
        infrastructure("clean_architecture::infrastructure") => [domain],
        presentation("clean_architecture::presentation") => [application, domain],
    }
}

layer_rules! {
    FIXTURE {
        acyclic: true,
        root("fixture") => [domain, infrastructure, facade],
        domain("fixture::domain") => [],
        infrastructure("fixture::infrastructure") => [domain],
        facade("fixture::facade") => [infrastructure],
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(domain: &str, facade: &str) -> String {
    format!(
        r#"
        pub mod domain {{ {domain} }}
        pub mod infrastructure {{
            pub struct Repo;
            impl Repo {{
                pub fn new() -> Self {{ Self }}
                pub fn count(&self) -> u64 {{ 1 }}
            }}
            pub trait Port {{}}
            impl Port for Repo {{}}
        }}
        pub mod facade {{ {facade} }}
    "#
    )
}

fn analyze(name: &str, source: &Path, crate_name: &str, cfg: &[&str]) -> TestResult<Graph> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Invalid case name".into());
    }
    let project = Project::new()?;
    let logs = root().join(".test-artifacts/resolved-layers").join(name);
    // Do not leave a previous local run's PASS or graph beside a new failure.
    if logs.exists() {
        fs::remove_dir_all(&logs)?;
    }
    fs::create_dir_all(&logs)?;
    let driver = std::env::var_os("LAYER_DRIVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/debug")
                .join(format!("layer-driver{}", std::env::consts::EXE_SUFFIX))
        });
    if !driver.is_file() {
        return Err("Build layer-driver with the pinned toolchain first".into());
    }
    let (code, sysroot) = execute(
        Command::new("rustup").args(["run", TOOLCHAIN.trim(), "rustc", "--print", "sysroot"]),
        &logs.join("sysroot.log"),
    )?;
    if code != Some(0) {
        return Err(format!("Toolchain unavailable: {sysroot}").into());
    }
    let output = project.0.join("graph.txt");
    for (phase, program) in [
        ("baseline", Path::new("rustc")),
        ("collector", driver.as_path()),
    ] {
        let mut command = Command::new("rustup");
        command.args(["run", TOOLCHAIN.trim()]).arg(program);
        command
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--emit=metadata",
                "--crate-name",
            ])
            .arg(crate_name)
            .arg("--sysroot")
            .arg(sysroot.trim())
            .arg("--out-dir")
            .arg(&project.0)
            .arg(source);
        for feature in cfg {
            command.arg("--cfg").arg(format!("feature={feature:?}"));
        }
        command.env("LAYER_GRAPH_OUT", &output);
        let (code, text) = execute(&mut command, &logs.join(format!("{phase}.log")))?;
        if code != Some(0) {
            return Err(
                format!("{name}: {phase} failed (not an architecture rejection):\n{text}").into(),
            );
        }
    }
    let text = fs::read_to_string(output)?;
    let graph = Graph::decode(&text).map_err(|e| format!("Malformed compiler graph: {e}"))?;
    if graph.crate_name != crate_name {
        return Err("Collector returned the wrong crate".into());
    }
    fs::write(logs.join("graph.txt"), text)?;
    fs::write(logs.join("graph.dot"), graph.dot())?;
    Ok(graph)
}

fn run_source(name: &str, source: &str, cfg: &[&str]) -> TestResult<Graph> {
    let project = Project::new()?;
    let path = project.0.join("lib.rs");
    fs::write(&path, source)?;
    let result = analyze(name, &path, "fixture", cfg);
    let logs = root().join(".test-artifacts/resolved-layers").join(name);
    fs::write(logs.join("fixture.rs"), source)?;
    result
}

fn report(name: &str, policy: &Policy, graph: &Graph) -> TestResult {
    let logs = root().join(".test-artifacts/resolved-layers").join(name);
    fs::write(logs.join("policy.md"), policy.report(graph))?;
    Ok(())
}

fn assert_forbidden(name: &str, graph: &Graph) -> TestResult {
    report(name, &FIXTURE, graph)?;
    let findings = FIXTURE
        .check(graph)
        .expect_err("Outer dependency was missed");
    assert!(
        findings.iter().any(|f| matches!(f,
            Finding::Forbidden { from, to, edge }
            if from == "domain" && to == "infrastructure" && !edge.location.is_empty()
        )),
        "Expected resolved domain -> infrastructure, got {findings:?}"
    );
    fs::write(
        root()
            .join(".test-artifacts/resolved-layers")
            .join(name)
            .join("result.txt"),
        "PASS: expected resolved dependency rejected\n",
    )?;
    Ok(())
}

macro_rules! resolved_violation {
    ($name:ident, $domain:expr, $facade:expr) => {
        #[test]
        fn $name() -> TestResult {
            let graph = run_source(stringify!($name), &fixture($domain, $facade), &[])?;
            assert_forbidden(stringify!($name), &graph)
        }
    };
}

resolved_violation!(
    fully_qualified_constructor,
    "pub fn leak() { let _ = crate::infrastructure::Repo::new(); }",
    ""
);
resolved_violation!(
    fully_qualified_return_type,
    "pub fn leak() -> crate::infrastructure::Repo { crate::infrastructure::Repo }",
    ""
);
resolved_violation!(unused_import, "pub use crate::infrastructure::Repo;", "");
resolved_violation!(
    grouped_aliased_import,
    "pub use crate::infrastructure::{Repo as Storage, Port};",
    ""
);
resolved_violation!(glob_import, "pub use crate::infrastructure::*;", "");
resolved_violation!(
    facade_reexport,
    "pub use crate::facade::Alias;",
    "pub use crate::infrastructure::Repo as Alias;"
);
resolved_violation!(chained_reexport,
    "pub use crate::facade::Alias;",
    "pub mod bridge { pub use crate::infrastructure::Repo as Inner; } pub use bridge::Inner as Alias;");
resolved_violation!(
    method_on_inferred_receiver,
    "pub fn leak() -> u64 { crate::facade::make().count() }",
    "pub fn make() -> crate::infrastructure::Repo { crate::infrastructure::Repo }"
);
resolved_violation!(
    generic_type_argument,
    "pub fn leak(_: Option<crate::infrastructure::Repo>) {}",
    ""
);
resolved_violation!(
    trait_bound,
    "pub fn leak<T: crate::infrastructure::Port>() {}",
    ""
);
resolved_violation!(type_alias_inference,
    "pub fn leak() { let _: crate::facade::Alias = crate::facade::make(); }",
    "pub type Alias = crate::infrastructure::Repo; pub fn make() -> Alias { crate::infrastructure::Repo }");
resolved_violation!(expanded_macro,
    "macro_rules! leak { () => { pub fn leak() { let _ = crate::infrastructure::Repo::new(); } }; } leak!();", "");
resolved_violation!(
    nested_module_and_relative_path,
    "pub mod nested { pub use super::super::infrastructure::Repo; }",
    ""
);

#[test]
fn actual_single_crate_obeys_layers() -> TestResult {
    let graph = analyze(
        "actual_single_crate",
        &root().join("src/lib.rs"),
        "clean_architecture",
        &[],
    )?;
    report("actual_single_crate", &CLEAN, &graph)?;
    assert_eq!(CLEAN.check(&graph), Ok(()));
    assert!(
        graph
            .edges
            .iter()
            .any(|e| e.from.ends_with("::application") && e.to.ends_with("::domain")),
        "Collector silently missed real references"
    );
    Ok(())
}

#[test]
fn permitted_directions_and_local_names_pass() -> TestResult {
    let graph = run_source(
        "permitted_directions",
        &fixture(
            "pub struct Repo; pub fn pure() -> Repo { Repo }",
            "pub use crate::infrastructure::Repo;",
        ),
        &[],
    )?;
    report("permitted_directions", &FIXTURE, &graph)?;
    assert_eq!(FIXTURE.check(&graph), Ok(()));
    Ok(())
}

#[test]
fn string_literals_are_not_dependencies() -> TestResult {
    let graph = run_source("string_literals", &fixture(
        "pub const TEXT: &str = \"crate::infrastructure::Repo\"; // crate::infrastructure::Repo\n", ""
    ), &[])?;
    assert_eq!(FIXTURE.check(&graph), Ok(()));
    Ok(())
}

#[test]
fn feature_specific_reference_is_checked_when_enabled() -> TestResult {
    let source = fixture(
        "#[cfg(feature = \"leak\")] pub fn leak() { let _ = crate::infrastructure::Repo::new(); }",
        "",
    );
    let off = run_source("feature_off", &source, &[])?;
    assert_eq!(FIXTURE.check(&off), Ok(()));
    let on = run_source("feature_on", &source, &["leak"])?;
    assert_forbidden("feature_on", &on)
}

#[test]
fn new_module_requires_classification() -> TestResult {
    let source = format!("{} pub mod unclassified {{}}", fixture("", ""));
    let graph = run_source("new_module", &source, &[])?;
    let findings = FIXTURE
        .check(&graph)
        .expect_err("Unclassified module passed");
    assert!(findings.contains(&Finding::Unclassified("fixture::unclassified".into())));
    Ok(())
}

#[test]
fn missing_or_cfg_disabled_layer_is_rejected() -> TestResult {
    let source = fixture("", "").replace("pub mod domain", "#[cfg(any())] pub mod domain");
    let graph = run_source("missing_layer", &source, &[])?;
    let findings = FIXTURE.check(&graph).expect_err("Absent layer passed");
    assert!(findings.contains(&Finding::MissingLayer("domain".into())));
    Ok(())
}

#[test]
fn permitted_edges_can_still_form_a_forbidden_cycle() -> TestResult {
    layer_rules! {
        CYCLE {
            acyclic: true,
            root("fixture") => [domain, infrastructure],
            domain("fixture::domain") => [infrastructure],
            infrastructure("fixture::infrastructure") => [domain],
        }
    }
    let graph = run_source(
        "cycle",
        "
        pub mod domain { pub struct Model; pub fn take(_: crate::infrastructure::Repo) {} }
        pub mod infrastructure { pub struct Repo; pub fn take(_: crate::domain::Model) {} }
    ",
        &[],
    )?;
    report("cycle", &CYCLE, &graph)?;
    let findings = CYCLE.check(&graph).expect_err("Cycle was missed");
    assert!(findings
        .iter()
        .any(|f| matches!(f, Finding::Cycle(path) if path.len() == 3)));
    assert!(!findings
        .iter()
        .any(|f| matches!(f, Finding::Forbidden { .. })));
    let permitted = Policy {
        acyclic: false,
        ..CYCLE
    };
    assert_eq!(permitted.check(&graph), Ok(()));
    Ok(())
}

#[path = "resolved/extra.rs"]
mod extra;
