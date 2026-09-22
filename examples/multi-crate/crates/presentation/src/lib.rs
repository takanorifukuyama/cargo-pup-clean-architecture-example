//! Controller independent of concrete storage.

use task_application::{CreateTaskError, TaskService};
use task_domain::TaskRepository;

#[derive(Debug, PartialEq, Eq)]
pub struct TaskView {
    pub id: u64,
    pub title: String,
}

pub struct TaskController<R> {
    service: TaskService<R>,
}

impl<R: TaskRepository> TaskController<R> {
    pub fn new(service: TaskService<R>) -> Self {
        Self { service }
    }

    pub fn create(&mut self, id: u64, title: &str) -> Result<TaskView, CreateTaskError> {
        let task = self.service.create(id, title)?;
        Ok(TaskView {
            id: task.id(),
            title: task.title().to_owned(),
        })
    }
}
