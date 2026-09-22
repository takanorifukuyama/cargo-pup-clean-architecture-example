//! Run from the repository root:
//! cargo test --manifest-path tools/dependency-contracts/Cargo.toml --locked

mod support;

use serde_json::{json, Value};
use std::fs;
use support::{check_all, check_rule, coverage, crate_dependencies, Fixture, Kind, Problem, TestResult};

crate_dependencies! {
    domain_is_independent {
        package: "task-domain",
        allow_normal: [],
        allow_dev: [],
        allow_build: [],
    }
    application_uses_domain_only {
        package: "task-application",
        allow_normal: ["task-domain"],
        allow_dev: [],
        allow_build: [],
    }
    infrastructure_uses_domain_ports {
        package: "task-infrastructure",
        allow_normal: ["task-domain"],
        allow_dev: [],
        allow_build: [],
    }
    presentation_has_no_storage_dependency {
        package: "task-presentation",
        allow_normal: ["task-application", "task-domain"],
        allow_dev: [],
        allow_build: [],
    }
    composition_root_wires_adapters {
        package: "task-cli",
        allow_normal: ["task-application", "task-infrastructure", "task-presentation"],
        allow_dev: ["task-domain"],
        allow_build: [],
    }
}

fn denied(fixture: &Fixture, from: &str, to: &str, kind: Kind) -> TestResult {
    fixture.compile_mutation()?;
    let metadata = fixture.metadata()?;
    coverage(&metadata, RULES)?;
    let problem = check_all(&metadata, RULES).expect_err("Forbidden dependency was accepted");
    assert!(matches!(
        &problem,
        Problem::Forbidden { package, dependency, kind: actual, .. }
            if package == from && dependency == to && *actual == kind
    ), "Wrong failure: {problem}");
    fixture.pass(&format!("Expected rejection: {problem}"))
}

macro_rules! forbidden_probe {
    ($name:ident, $table:literal, $kind:ident, $optional:literal) => {
        #[test]
        fn $name() -> TestResult {
            let fixture = Fixture::new(stringify!($name))?;
            fixture.probe("forbidden-io", "0.1.0")?;
            fixture.dependency("domain", $table, concat!(
                "hidden_storage = { package = \"forbidden-io\", path = \"../../../probe\", optional = ",
                $optional, " }"
            ))?;
            denied(&fixture, "task-domain", "forbidden-io", Kind::$kind)
        }
    };
}

forbidden_probe!(renamed_runtime_dependency_is_rejected, "dependencies", Normal, "false");
forbidden_probe!(inactive_optional_dependency_is_rejected, "dependencies", Normal, "true");
forbidden_probe!(development_dependency_is_rejected, "dev-dependencies", Dev, "false");
forbidden_probe!(build_dependency_without_build_script_is_rejected, "build-dependencies", Build, "false");
forbidden_probe!(other_platform_dependency_is_rejected, "target.'cfg(windows)'.dependencies", Normal, "false");
forbidden_probe!(other_platform_build_dependency_is_rejected, "target.'cfg(windows)'.build-dependencies", Build, "false");

#[test]
fn application_cannot_add_a_concrete_adapter() -> TestResult {
    let fixture = Fixture::new("application_cannot_add_a_concrete_adapter")?;
    fixture.dependency("application", "dependencies", "task-infrastructure = { path = \"../infrastructure\" }")?;
    denied(&fixture, "task-application", "task-infrastructure", Kind::Normal)
}

#[test]
fn presentation_cannot_add_storage() -> TestResult {
    let fixture = Fixture::new("presentation_cannot_add_storage")?;
    fixture.dependency("presentation", "dependencies", "task-infrastructure = { path = \"../infrastructure\" }")?;
    denied(&fixture, "task-presentation", "task-infrastructure", Kind::Normal)
}

#[test]
fn development_allowance_does_not_allow_runtime_use() -> TestResult {
    let fixture = Fixture::new("development_allowance_does_not_allow_runtime_use")?;
    fixture.dependency("cli", "dependencies", "task-domain = { path = \"../domain\" }")?;
    denied(&fixture, "task-cli", "task-domain", Kind::Normal)
}

#[test]
fn allowed_dependency_may_be_renamed() -> TestResult {
    let fixture = Fixture::new("allowed_dependency_may_be_renamed")?;
    fixture.replace(
        "crates/application/Cargo.toml",
        "task-domain = { path = \"../domain\" }",
        "model = { package = \"task-domain\", path = \"../domain\" }",
    )?;
    fixture.replace("crates/application/src/lib.rs", "use task_domain::", "use model::")?;
    fixture.compile_mutation()?;
    check_all(&fixture.metadata()?, RULES)?;
    fixture.pass("Allowed package remains allowed under an alias")
}

#[test]
fn inherited_workspace_dependency_is_checked() -> TestResult {
    let fixture = Fixture::new("inherited_workspace_dependency_is_checked")?;
    fixture.dependency("application", "dependencies", "placeholder = { package = \"task-domain\", path = \"../domain\" }")?;
    // Remove the temporary entry and replace the actual declaration with inheritance.
    fixture.replace("crates/application/Cargo.toml", "placeholder = { package = \"task-domain\", path = \"../domain\" }\n", "")?;
    let manifest = fixture.workspace.join("Cargo.toml");
    let original = fs::read_to_string(&manifest)?;
    fs::write(manifest, format!("{original}\n[workspace.dependencies]\ntask-domain = {{ path = \"crates/domain\" }}\n"))?;
    fixture.replace("crates/application/Cargo.toml", "task-domain = { path = \"../domain\" }", "task-domain.workspace = true")?;
    fixture.compile_mutation()?;
    check_all(&fixture.metadata()?, RULES)?;
    fixture.pass("Workspace-inherited declaration is checked by actual package identity")
}

#[test]
fn same_name_from_a_different_path_is_rejected() -> TestResult {
    let fixture = Fixture::new("same_name_from_a_different_path_is_rejected")?;
    // A different version keeps this a valid Cargo graph rather than a lock collision.
    fixture.probe("task-domain", "0.2.0")?;
    fixture.dependency("application", "dependencies", "imposter = { package = \"task-domain\", path = \"../../../probe\" }")?;
    fixture.compile_mutation()?;
    let problem = check_all(&fixture.metadata()?, RULES).expect_err("Wrong source was accepted");
    assert_eq!(problem, Problem::WrongSource {
        package: "task-application".into(), dependency: "task-domain".into(),
    });
    fixture.pass(&format!("Expected rejection: {problem}"))
}

#[test]
fn new_workspace_member_requires_a_contract() -> TestResult {
    let fixture = Fixture::new("new_workspace_member_requires_a_contract")?;
    fixture.replace("Cargo.toml", "members = [\n", "members = [\n    \"crates/unclassified\",\n")?;
    let package = fixture.workspace.join("crates/unclassified");
    fs::create_dir_all(package.join("src"))?;
    fs::write(package.join("Cargo.toml"), "[package]\nname = \"unclassified\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")?;
    fs::write(package.join("src/lib.rs"), "pub struct Independent;\n")?;
    fixture.compile_mutation()?;
    let problem = check_all(&fixture.metadata()?, RULES).expect_err("Unclassified member was accepted");
    assert!(matches!(&problem, Problem::Coverage(message) if message.contains("unclassified")));
    fixture.pass(&format!("Expected rejection: {problem}"))
}

#[test]
fn missing_duplicate_and_misspelled_rules_are_rejected() -> TestResult {
    let fixture = Fixture::new("missing_duplicate_and_misspelled_rules_are_rejected")?;
    let metadata = fixture.metadata()?;
    assert!(matches!(coverage(&metadata, &RULES[..4]), Err(Problem::Coverage(_))));
    let mut rules = RULES.to_vec();
    rules.push(RULES[0]);
    assert!(matches!(coverage(&metadata, &rules), Err(Problem::Coverage(_))));
    let mut rules = RULES.to_vec();
    rules[0].package = "task-domian";
    assert!(matches!(coverage(&metadata, &rules), Err(Problem::Coverage(_))));
    let mut rules = RULES.to_vec();
    rules[1].normal = &["task-domian"];
    assert!(matches!(coverage(&metadata, &rules), Err(Problem::Coverage(_))));
    fixture.pass("Missing, duplicate and misspelled policy declarations fail closed")
}

#[test]
fn empty_or_malformed_metadata_never_passes() {
    for metadata in [Value::Null, json!({}), json!({"version": 2}), json!({"version": 1, "workspace_members": ["missing"], "packages": []})] {
        assert!(matches!(check_all(&metadata, RULES), Err(Problem::InvalidMetadata(_))));
    }
    assert!(matches!(check_all(&json!({"version": 1, "workspace_members": [], "packages": []}), &[]), Err(Problem::Coverage(_))));
}

#[test]
fn unknown_dependency_kind_is_not_silently_ignored() -> TestResult {
    let fixture = Fixture::new("unknown_dependency_kind_is_not_silently_ignored")?;
    let mut metadata = fixture.metadata()?;
    let app = metadata["packages"].as_array_mut().ok_or("Missing packages")?
        .iter_mut().find(|p| p["name"] == "task-application").ok_or("Missing application")?;
    app["dependencies"][0]["kind"] = json!("future-kind");
    assert!(matches!(check_all(&metadata, RULES), Err(Problem::InvalidMetadata(_))));
    fixture.pass("Unknown dependency kinds are errors")
}

#[test]
fn compiler_rejects_fully_qualified_outer_type_without_dependency() -> TestResult {
    let fixture = Fixture::new("compiler_rejects_fully_qualified_outer_type_without_dependency")?;
    fixture.succeed("baseline", &["check", "--workspace", "--all-targets", "--locked", "--offline", "--quiet"])?;
    let path = fixture.workspace.join("crates/domain/src/lib.rs");
    let original = fs::read_to_string(&path)?;
    fs::write(path, format!("{original}\npub fn forbidden() -> task_infrastructure::InMemoryTaskRepository {{ task_infrastructure::InMemoryTaskRepository::default() }}\n"))?;
    let (code, output) = fixture.cargo("compiler-boundary", &[
        "check", "--package", "task-domain", "--lib", "--locked", "--offline", "--message-format=json",
    ])?;
    assert!(code.is_some_and(|code| code > 0), "Expected compiler failure: {output}");
    let diagnosed = output.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .any(|entry| {
            entry["reason"] == "compiler-message"
                && entry["message"]["level"] == "error"
                && entry["message"]["code"]["code"] == "E0433"
                && entry["message"]["message"].as_str().is_some_and(|text| text.contains("task_infrastructure"))
        });
    assert!(diagnosed, "Missing E0433 for task_infrastructure: {output}");
    assert!(!output.contains("internal compiler error"), "Compiler crashed");
    fixture.pass("Compiler rejects this cross-crate path before an import lint is needed")
}
