//! These contracts run on stable Rust, without cargo-pup or a feature flag.
//! The negative controls compile the SAME macro, rather than emulating its logic.

#[path = "common/type_contracts.rs"]
mod macros;
#[path = "common/runtime.rs"]
mod runtime;

use clean_architecture::domain::{Task, TaskRepository};
use clean_architecture::infrastructure::InMemoryTaskRepository;
use macros::type_contracts;
use runtime::{execute, Project, TestResult};
use std::fs;
use std::path::Path;
use std::process::Command;

type_contracts! {
    repository_contract {
        type: InMemoryTaskRepository,
        implements: [TaskRepository, Send, Sync],
    }
    task_can_cross_threads {
        type: Task,
        implements: [Send, Sync],
    }
}

// An unrelated compiler error, warning or rustc crash is never proof of E0277.
fn verify_bound_error(code: Option<i32>, output: &str, bound: &str) -> TestResult {
    let valid = code.is_some_and(|value| value > 0)
        && output.lines().any(|line| {
            if !line.starts_with("error[E0277]:") {
                return false;
            }
            line.contains(bound)
                || match bound {
                    "Send" => line.contains("sent between threads"),
                    "Sync" => line.contains("shared between threads"),
                    _ => false,
                }
        })
        && !output.contains("internal compiler error")
        && !output.contains("panicked at");
    if valid {
        Ok(())
    } else {
        Err(format!("Expected E0277 for {bound}, exit={code:?}:\n{output}").into())
    }
}

fn compiler_case(
    name: &str,
    declarations: &str,
    contract: &str,
    bound: Option<&str>,
) -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = Project::new()?;
    let logs = root.join(".test-artifacts/type-contracts");
    fs::create_dir_all(&logs)?;
    let macro_path = root.join("tests/common/type_contracts.rs");
    let source = project.0.join("case.rs");
    // First prove that the fixture itself compiles without the contract.
    for (phase, text) in [
        ("baseline", declarations.to_owned()),
        (
            "contract",
            format!(
                "#[path = {macro_path:?}] mod macros;\nuse macros::type_contracts;\n{declarations}\n{contract}\n"
            ),
        ),
    ] {
        fs::write(&source, &text)?;
        fs::write(logs.join(format!("{name}.{phase}.rs")), text)?;
        let (code, output) = execute(
            Command::new("rustup")
                .args([
                    "run",
                    "stable",
                    "rustc",
                    "--edition=2021",
                    "--test",
                    "--emit=metadata",
                    "--error-format=human",
                    "--crate-name=contract_fixture",
                    "--out-dir",
                ])
                .arg(&project.0)
                .arg(&source),
            &logs.join(format!("{name}.{phase}.log")),
        )?;
        if phase == "contract" {
            if let Some(bound) = bound {
                verify_bound_error(code, &output, bound)?;
                continue;
            }
        }
        if code != Some(0) {
            return Err(format!("{name}: {phase} must compile successfully:\n{output}").into());
        }
    }
    Ok(())
}

macro_rules! type_contract_violation {
    ($name:ident {
        declarations: { $($declaration:tt)* }
        type: $ty:ty,
        requires: $bound:path,
        diagnostic: $diagnostic:literal,
    }) => {
        #[test]
        fn $name() -> TestResult {
            compiler_case(
                stringify!($name),
                stringify!($($declaration)*),
                stringify!(type_contracts! {
                    contract_probe { type: $ty, implements: [$bound], }
                }),
                Some($diagnostic),
            )
        }
    };
}

type_contract_violation! {
    missing_repository_implementation_is_rejected {
        declarations: { trait TaskRepository {} struct Repository; }
        type: Repository,
        requires: TaskRepository,
        diagnostic: "TaskRepository",
    }
}

type_contract_violation! {
    non_send_type_is_rejected {
        declarations: { struct LocalRepository(std::rc::Rc<u8>); }
        type: LocalRepository,
        requires: Send,
        diagnostic: "Send",
    }
}

type_contract_violation! {
    non_sync_type_is_rejected {
        declarations: { struct MutableRepository(std::cell::Cell<u8>); }
        type: MutableRepository,
        requires: Sync,
        diagnostic: "Sync",
    }
}

#[test]
fn valid_compiler_control() -> TestResult {
    compiler_case(
        "valid_compiler_control",
        "trait TaskRepository {} struct Repository; impl TaskRepository for Repository {}",
        "type_contracts! { control { type: Repository, implements: [TaskRepository, Send, Sync], } }",
        None,
    )
}

#[test]
fn bound_diagnostics_cannot_be_satisfied_by_unrelated_errors() {
    for (code, output) in [
        (Some(0), "error[E0277]: TaskRepository is not implemented"),
        (None, "error[E0277]: TaskRepository is not implemented"),
        (Some(1), "error[E0412]: cannot find type TaskRepository"),
        (Some(1), "warning: TaskRepository is not implemented"),
        (Some(1), "error[E0277]: AnotherTrait is not implemented"),
    ] {
        assert!(verify_bound_error(code, output, "TaskRepository").is_err());
    }
    assert!(verify_bound_error(
        Some(1),
        "error[E0277]: TaskRepository is not implemented",
        "TaskRepository"
    )
    .is_ok());
}
