use clean_architecture::application::{CreateTaskError, TaskService};
use clean_architecture::domain::{InvalidTitle, RepositoryError, Task, TaskRepository};
use clean_architecture::infrastructure::InMemoryTaskRepository;
use clean_architecture::presentation::{TaskController, TaskView};
use std::process::Command;

#[test]
fn domain_rejects_blank_titles() {
    assert_eq!(Task::new(1, " \t\n "), Err(InvalidTitle));
}

#[test]
fn domain_normalizes_titles() {
    let task = Task::new(7, "  Ship it  ").expect("valid task");
    assert_eq!(task.id(), 7);
    assert_eq!(task.title(), "Ship it");
}

#[test]
fn use_case_persists_a_valid_task() {
    let mut service = TaskService::new(InMemoryTaskRepository::default());
    let task = service.create(1, "Write tests").expect("create task");
    assert_eq!(service.find(1), Ok(Some(task)));
    assert_eq!(service.find(999), Ok(None));
}

#[test]
fn invalid_input_is_not_persisted() {
    let mut service = TaskService::new(InMemoryTaskRepository::default());
    assert_eq!(
        service.create(1, " "),
        Err(CreateTaskError::InvalidTitle(InvalidTitle))
    );
    assert_eq!(service.find(1), Ok(None));
}

#[test]
fn duplicate_ids_do_not_overwrite_existing_tasks() {
    let mut service = TaskService::new(InMemoryTaskRepository::default());
    let original = service.create(1, "Original").expect("first insert");
    assert_eq!(
        service.create(1, "Replacement"),
        Err(CreateTaskError::Repository(RepositoryError::DuplicateId(1)))
    );
    assert_eq!(service.find(1), Ok(Some(original)));
}

struct UnavailableRepository;

impl TaskRepository for UnavailableRepository {
    fn insert(&mut self, _task: &Task) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    fn find(&self, _id: u64) -> Result<Option<Task>, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
}

#[test]
fn repository_can_be_replaced_without_changing_the_use_case() {
    let mut service = TaskService::new(UnavailableRepository);
    assert_eq!(
        service.create(1, "Write tests"),
        Err(CreateTaskError::Repository(RepositoryError::Unavailable))
    );
}

#[test]
fn controller_converts_domain_objects_to_a_view() {
    let service = TaskService::new(InMemoryTaskRepository::default());
    let mut controller = TaskController::new(service);
    assert_eq!(
        controller.create(3, "  Review architecture  "),
        Ok(TaskView {
            id: 3,
            title: "Review architecture".to_owned(),
        })
    );
}

#[test]
fn cli_runs_the_composed_application() {
    let output = Command::new(env!("CARGO_BIN_EXE_task-demo"))
        .arg("Write architecture tests")
        .output()
        .expect("run demo binary");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"1: Write architecture tests\n");
}

#[test]
fn cli_exits_unsuccessfully_for_blank_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_task-demo"))
        .arg("   ")
        .output()
        .expect("run demo binary");
    assert!(!output.status.success());
}
