//! Real cargo-pup controls: empty selectors fail; visibility violations compile first.

use crate::support::{
    self, Edit, Expectation, Replacement, Rule, TestResult, Visibility, VisibilityRule,
};

macro_rules! visibility_violation {
    ($name:ident, $replacement:literal) => {
        #[test]
        fn $name() -> TestResult {
            let rule = &crate::VISIBILITY_RULES[0];
            support::check_visibility(
                stringify!($name),
                rule,
                Some(Replacement {
                    file: "src/infrastructure.rs",
                    before: "pub(crate) struct TaskStore",
                    after: $replacement,
                }),
                Expectation::Denied,
            )
        }
    };
}

visibility_violation!(internal_storage_cannot_be_public, "pub struct TaskStore");
visibility_violation!(crate_visibility_is_an_explicit_contract, "struct TaskStore");
visibility_violation!(
    narrower_visibility_is_rejected,
    "pub(in crate::infrastructure) struct TaskStore"
);

#[test]
fn misspelled_module_is_rejected() -> TestResult {
    support::check(
        "misspelled_module_is_rejected",
        &Rule {
            name: "missing_module",
            module: "clean_architecture::domian",
            deny_imports: &["crate::infrastructure"],
        },
        None,
        Expectation::MissingTarget,
    )
}

#[test]
fn cfg_disabled_module_is_rejected() -> TestResult {
    support::check(
        "cfg_disabled_module_is_rejected",
        &Rule {
            name: "disabled_module",
            module: "clean_architecture::disabled_scope",
            deny_imports: &["std::fs"],
        },
        Some(Edit {
            file: "src/lib.rs",
            code: "#[cfg(any())] pub mod disabled_scope {}",
        }),
        Expectation::MissingTarget,
    )
}

#[test]
fn misspelled_struct_is_rejected() -> TestResult {
    support::check_visibility(
        "misspelled_struct_is_rejected",
        &VisibilityRule {
            name: "missing_struct",
            struct_name: "TaskStroe",
            visibility: Visibility::PubCrate,
        },
        None,
        Expectation::MissingTarget,
    )
}

#[test]
fn suppressing_the_canary_is_rejected() -> TestResult {
    // Disable compiler lints inside a disposable fixture only. The real domain
    // exists, but coverage must reject the lack of a named canary diagnostic.
    support::check_suppressed_coverage(
        "suppressing_the_canary_is_rejected",
        support::find_rule(crate::RULES, "domain_inward_only")?,
    )
}
