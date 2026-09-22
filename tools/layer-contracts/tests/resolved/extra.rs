use super::*;

#[test]
fn crate_root_reexport_resolves_to_original_layer() -> TestResult {
    let source = format!(
        "{} pub use infrastructure::Repo as Alias;",
        fixture("pub use crate::Alias;", "")
    );
    let graph = run_source("crate_root_reexport", &source, &[])?;
    assert_forbidden("crate_root_reexport", &graph)
}

fn copy_source(source: &Path, target: &Path) -> TestResult {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_source(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err("Unsupported source entry".into());
        }
    }
    Ok(())
}

#[test]
fn original_cargo_pup_known_gap_is_rejected_by_new_collector() -> TestResult {
    let project = Project::new()?;
    let source = project.0.join("src");
    copy_source(&root().join("src"), &source)?;
    let path = source.join("domain.rs");
    let original = fs::read_to_string(&path)?;
    fs::write(path, format!("{original}\npub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {{\n    crate::infrastructure::InMemoryTaskRepository::default()\n}}\n"))?;
    let graph = analyze(
        "original_known_gap",
        &source.join("lib.rs"),
        "clean_architecture",
        &[],
    )?;
    report("original_known_gap", &CLEAN, &graph)?;
    let findings = CLEAN
        .check(&graph)
        .expect_err("Original known gap survived");
    assert!(
        findings.iter().any(|f| matches!(f,
            Finding::Forbidden { from, to, edge }
            if from == "domain" && to == "infrastructure"
                && edge.symbol.ends_with("InMemoryTaskRepository")
        )),
        "Expected actual repository type, got {findings:?}"
    );
    Ok(())
}

#[test]
fn ordinary_compilation_error_is_not_architecture_detection() -> TestResult {
    let source = fixture("pub fn broken() { let _: u64 = \"wrong\"; }", "");
    let error = run_source("compiler_error", &source, &[]).expect_err("Broken fixture compiled");
    assert!(error.to_string().contains("baseline failed"));
    let logs = root().join(".test-artifacts/resolved-layers/compiler_error");
    assert!(!logs.join("graph.txt").exists());
    Ok(())
}

#[test]
fn allowing_warnings_does_not_disable_reference_collection() -> TestResult {
    let source = format!(
        "#![allow(warnings)]\n{}",
        fixture(
            "pub fn leak() { let _ = crate::infrastructure::Repo::new(); }",
            ""
        )
    );
    let graph = run_source("allow_warnings", &source, &[])?;
    assert_forbidden("allow_warnings", &graph)
}
