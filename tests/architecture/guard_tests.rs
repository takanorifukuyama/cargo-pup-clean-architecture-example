use super::*;

#[test]
fn module_canary_uses_the_same_selector() {
    let rule = Rule {
        name: "domain",
        module: "clean_architecture::domain",
        deny_imports: &["std::fs"],
    };
    let lint = Lint::Imports(&rule);
    for coverage in [false, true] {
        assert!(lint.config(coverage).unwrap().contains("^clean_architecture::domain(::|$)"));
    }
    let probe = lint.config(true).unwrap();
    assert!(probe.contains("coverage_domain"));
    assert!(probe.contains("MustBeNamed(\"^$\", Error)"));
}

#[test]
fn visibility_rules_generate_implemented_rules_and_exact_short_names() {
    for (visibility, expected) in [
        (Visibility::Private, "MustBePrivate(Error)"),
        (Visibility::Public, "MustBePublic(Error)"),
        (Visibility::PubCrate, "MustBePubCrate(Error)"),
    ] {
        let rule = VisibilityRule { name: "storage", struct_name: "TaskStore", visibility };
        let lint = Lint::Visibility(&rule);
        assert!(lint.config(false).unwrap().contains(expected));
        for coverage in [false, true] {
            assert!(lint.config(coverage).unwrap().contains("Name(\"^TaskStore$\")"));
        }
    }
}

#[test]
fn qualified_struct_selectors_are_rejected_not_silently_ignored() {
    let rule = VisibilityRule {
        name: "bad_selector",
        struct_name: "infrastructure::TaskStore",
        visibility: Visibility::PubCrate,
    };
    assert!(Lint::Visibility(&rule).config(false).is_err());
    assert!(Lint::Visibility(&rule).config(true).is_err());
}

#[test]
fn diagnostic_tokens_cannot_be_combined_across_errors_or_warnings() {
    let output = "warning: Module must match ^$\n  = note: Applied by cargo-pup rule 'coverage_domain'.\nerror: unrelated failure";
    assert!(!named_error(Some(1), output, "coverage_domain", &["Module must match"]));
    let output = "error: Module must match ^$\n  = note: Applied by cargo-pup rule 'other'.\nerror: unrelated failure\n  = note: Applied by cargo-pup rule 'coverage_domain'.";
    assert!(!named_error(Some(1), output, "coverage_domain", &["Module must match"]));
    let output = "error: Module must match ^$\n  = note: Applied by cargo-pup rule 'coverage_domain'.";
    assert!(named_error(Some(1), output, "coverage_domain", &["Module must match", "^$"]));
    assert!(!named_error(Some(1), output, "coverage_dom", &["Module must match"]));
}

#[test]
fn replacement_mutations_require_one_unambiguous_occurrence() {
    assert!(replace_once("hello", "", "x").is_err());
    assert!(replace_once("hello", "missing", "x").is_err());
    assert!(replace_once("hello hello", "hello", "x").is_err());
    assert_eq!(replace_once("hello world", "hello", "goodbye").unwrap(), "goodbye world");
}
