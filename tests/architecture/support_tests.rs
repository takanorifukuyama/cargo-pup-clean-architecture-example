use super::*;

const DIAGNOSTIC: &str = "error: domain_inward_only: Use of module 'crate::infrastructure::Repo' is denied";

#[test]
fn accepts_only_a_named_error_with_positive_exit() {
    assert!(verify(Expectation::Denied, "domain_inward_only", Some(101), DIAGNOSTIC).is_ok());
}

#[test]
fn silent_success_is_not_a_detected_violation() {
    assert!(verify(Expectation::Denied, "domain_inward_only", Some(0), DIAGNOSTIC).is_err());
}

#[test]
fn another_rule_cannot_satisfy_the_contract() {
    assert!(verify(Expectation::Denied, "application_inward_only", Some(101), DIAGNOSTIC).is_err());
}

#[test]
fn configuration_and_compilation_errors_are_not_detection() {
    for message in ["error: could not parse pup.ron", "error[E0432]: unresolved import"] {
        assert!(verify(Expectation::Denied, "domain_inward_only", Some(101), message).is_err());
    }
}

#[test]
fn warnings_are_not_enforcement() {
    let warning = DIAGNOSTIC.replace("error:", "warning:");
    assert!(verify(Expectation::Denied, "domain_inward_only", Some(1), &warning).is_err());
}

#[test]
fn signals_and_panics_are_not_expected_failures() {
    assert!(verify(Expectation::Denied, "domain_inward_only", None, DIAGNOSTIC).is_err());
    let panic = format!("{DIAGNOSTIC}\nthread 'main' panicked at lint.rs:1");
    assert!(verify(Expectation::Denied, "domain_inward_only", Some(101), &panic).is_err());
}

#[test]
fn valid_code_must_succeed() {
    assert!(verify(Expectation::Pass, "rule", Some(0), "").is_ok());
    assert!(verify(Expectation::Pass, "rule", Some(1), "").is_err());
}

#[test]
fn known_gap_is_characterization_not_protection() {
    assert!(verify(Expectation::KnownGap, "rule", Some(0), "").is_ok());
    assert!(verify(Expectation::KnownGap, "domain_inward_only", Some(101), DIAGNOSTIC).is_err());
}

#[test]
fn normalizes_macro_paths_and_preserves_boundary_patterns() {
    assert_eq!(normalized_path("clean_architecture :: domain").unwrap(), "clean_architecture::domain");
    assert_eq!(import_pattern("crate :: infrastructure").unwrap(), "(^|::)infrastructure(::|$)");
    assert_eq!(import_pattern("std::fs").unwrap(), "^std::fs(::|$)");
    assert_eq!(import_pattern("sqlx").unwrap(), "^sqlx(::|$)");
}

#[test]
fn rejects_unsupported_paths_instead_of_injecting_regex_or_ron() {
    for path in ["", "foo::", "::foo", "foo.*", "Foo<T>", "foo\"", "r#type"] {
        assert!(normalized_path(path).is_err(), "accepted {path}");
    }
}

#[test]
fn generates_error_severity_and_child_module_matching() {
    let rule = Rule { name: "domain_inward_only", module: "clean_architecture::domain", deny_imports: &["crate::infrastructure", "std::fs"] };
    let config = configuration(&rule).unwrap();
    assert!(config.contains("severity: Error"));
    assert!(config.contains("^clean_architecture::domain(::|$)"));
    assert!(config.contains("(^|::)infrastructure(::|$)"));
    assert!(config.contains("^std::fs(::|$)"));
    assert!(configuration(&Rule { deny_imports: &[], ..rule }).is_err());
}

#[test]
fn pins_are_required_unique_and_quoted() {
    let pins = "version = \"0.1.8\"\ntoolchain = \"nightly-2026-01-22\"\n";
    assert_eq!(setting(pins, "version").unwrap(), "0.1.8");
    assert_eq!(setting(pins, "toolchain").unwrap(), "nightly-2026-01-22");
    assert!(setting(pins, "missing").is_err());
    assert!(setting("version = 1", "version").is_err());
    assert!(setting("version = \"1\"\nversion = \"2\"", "version").is_err());
}

#[test]
fn strips_terminal_colors() {
    assert_eq!(strip_ansi("\u{1b}[1;32mcargo-pup\u{1b}[0m version 0.1.8\n"), "cargo-pup version 0.1.8\n");
}

#[test]
fn temporary_projects_are_distinct_and_cleaned_up() {
    let first = Project::new().unwrap();
    let second = Project::new().unwrap();
    assert_ne!(first.0, second.0);
    let path = first.0.clone();
    drop(first);
    assert!(!path.exists());
    assert!(second.0.exists());
}

#[test]
fn missing_and_duplicate_rule_names_are_errors() {
    assert!(find_rule(&[], "missing").is_err());
    let rules = [
        Rule { name: "same", module: "a", deny_imports: &["b"] },
        Rule { name: "same", module: "c", deny_imports: &["d"] },
    ];
    assert!(find_rule(&rules, "same").is_err());
}
