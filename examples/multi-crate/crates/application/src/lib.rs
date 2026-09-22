//! Use cases know domain ports, not concrete repositories.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use task_domain::{InvalidTitle, RepositoryError, Task, TaskRepository};

pub struct TaskService<R> {
    repository: R,
}

impl<R: TaskRepository> TaskService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn create(&mut self, id: u64, title: &str) -> Result<Task, CreateTaskError> {
        let task = Task::new(id, title).map_err(CreateTaskError::InvalidTitle)?;
        self.repository
            .insert(&task)
            .map_err(CreateTaskError::Repository)?;
        Ok(task)
    }

    pub fn find(&self, id: u64) -> Result<Option<Task>, RepositoryError> {
        self.repository.find(id)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CreateTaskError {
    InvalidTitle(InvalidTitle),
    Repository(RepositoryError),
}

impl Display for CreateTaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTitle(error) => Display::fmt(error, f),
            Self::Repository(error) => Display::fmt(error, f),
        }
    }
}

impl Error for CreateTaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTitle(error) => Some(error),
            Self::Repository(error) => Some(error),
        }
    }
}
