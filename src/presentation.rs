//! Transport DTOs and a controller. It knows the use case, not its storage.
use crate::application::{CreateTask, CreateTaskError, TaskRepository};

pub struct CreateTaskRequest {
    pub id: u64,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CreateTaskResponse {
    pub id: u64,
    pub title: String,
}

pub fn create_task<R: TaskRepository + ?Sized>(
    use_case: &mut CreateTask<'_, R>,
    request: CreateTaskRequest,
) -> Result<CreateTaskResponse, CreateTaskError> {
    let task = use_case.execute(request.id, &request.title)?;
    Ok(CreateTaskResponse {
        id: task.id(),
        title: task.title().to_owned(),
    })
}
