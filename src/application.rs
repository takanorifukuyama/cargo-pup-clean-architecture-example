//! The use case owns its output port, rather than depending on an adapter.
use crate::domain::{InvalidTitle, Task};
use std::error::Error;
use std::fmt;

pub trait TaskRepository {
    /// Save a new task without replacing an existing ID.
    fn save(&mut self, task: Task) -> Result<(), SaveError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    DuplicateId,
    Unavailable,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId => write!(f, "task ID already exists"),
            Self::Unavailable => write!(f, "task storage is unavailable"),
        }
    }
}

impl Error for SaveError {}

#[derive(Debug, PartialEq, Eq)]
pub enum CreateTaskError {
    InvalidTitle(InvalidTitle),
    Storage(SaveError),
}

impl fmt::Display for CreateTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTitle(error) => write!(f, "{error}"),
            Self::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl Error for CreateTaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTitle(error) => Some(error),
            Self::Storage(error) => Some(error),
        }
    }
}

pub struct CreateTask<'a, R: TaskRepository + ?Sized> {
    repository: &'a mut R,
}

impl<'a, R: TaskRepository + ?Sized> CreateTask<'a, R> {
    pub fn new(repository: &'a mut R) -> Self {
        Self { repository }
    }

    pub fn execute(&mut self, id: u64, title: &str) -> Result<Task, CreateTaskError> {
        let task = Task::new(id, title).map_err(CreateTaskError::InvalidTitle)?;
        self.repository
            .save(task.clone())
            .map_err(CreateTaskError::Storage)?;
        Ok(task)
    }
}
