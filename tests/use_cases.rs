use clean_architecture::application::{CreateTask, CreateTaskError, SaveError, TaskRepository};
use clean_architecture::domain::{InvalidTitle, Task};
use clean_architecture::infrastructure::InMemoryTaskRepository;
use clean_architecture::presentation::{create_task, CreateTaskRequest, CreateTaskResponse};

#[test]
fn creates_and_persists_a_valid_task() {
    let mut repository = InMemoryTaskRepository::default();
    let task = CreateTask::new(&mut repository)
        .execute(1, "  write architecture tests  ")
        .unwrap();
    assert_eq!(task.title(), "write architecture tests");
    assert_eq!(repository.get(1), Some(&task));
}

#[test]
fn invalid_input_does_not_call_the_repository() {
    #[derive(Default)]
    struct CountingTaskRepository(usize);
    impl TaskRepository for CountingTaskRepository {
        fn save(&mut self, _task: Task) -> Result<(), SaveError> {
            self.0 += 1;
            Ok(())
        }
    }
    let mut repository = CountingTaskRepository::default();
    assert_eq!(
        CreateTask::new(&mut repository).execute(1, " "),
        Err(CreateTaskError::InvalidTitle(InvalidTitle::Empty))
    );
    assert_eq!(repository.0, 0);
}

#[test]
fn duplicate_id_does_not_overwrite_existing_task() {
    let mut repository = InMemoryTaskRepository::default();
    let first = CreateTask::new(&mut repository)
        .execute(1, "first task")
        .unwrap();
    assert_eq!(
        CreateTask::new(&mut repository).execute(1, "replacement"),
        Err(CreateTaskError::Storage(SaveError::DuplicateId))
    );
    assert_eq!(repository.get(1), Some(&first));
}

#[test]
fn storage_failure_is_propagated_without_panicking() {
    struct FailingTaskRepository;
    impl TaskRepository for FailingTaskRepository {
        fn save(&mut self, _task: Task) -> Result<(), SaveError> {
            Err(SaveError::Unavailable)
        }
    }
    let mut repository = FailingTaskRepository;
    assert_eq!(
        CreateTask::new(&mut repository).execute(1, "valid task"),
        Err(CreateTaskError::Storage(SaveError::Unavailable))
    );
}

#[test]
fn presentation_maps_the_request_and_response() {
    let mut repository = InMemoryTaskRepository::default();
    let response = create_task(
        &mut CreateTask::new(&mut repository),
        CreateTaskRequest {
            id: 42,
            title: "  review the PR  ".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(
        response,
        CreateTaskResponse {
            id: 42,
            title: "review the PR".to_owned(),
        }
    );
    assert!(repository.get(42).is_some());
}

#[test]
fn presentation_does_not_hide_validation_errors() {
    let mut repository = InMemoryTaskRepository::default();
    let response = create_task(
        &mut CreateTask::new(&mut repository),
        CreateTaskRequest {
            id: 1,
            title: String::new(),
        },
    );
    assert_eq!(
        response,
        Err(CreateTaskError::InvalidTitle(InvalidTitle::Empty))
    );
}
