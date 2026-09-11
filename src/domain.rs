//! Business rules and persistence ports. No knowledge of outer-layer implementations.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: u64,
    title: String,
}

impl Task {
    pub fn new(id: u64, title: &str) -> Result<Self, InvalidTitle> {
        let title = title.trim();
        if title.is_empty() {
            return Err(InvalidTitle);
        }
        Ok(Self {
            id,
            title: title.to_owned(),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTitle;

impl Display for InvalidTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "task title must not be empty")
    }
}

impl Error for InvalidTitle {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    DuplicateId(u64),
    Unavailable,
}

impl Display for RepositoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "task {id} already exists"),
            Self::Unavailable => write!(f, "task repository is unavailable"),
        }
    }
}

impl Error for RepositoryError {}

/// Implementations belong in infrastructure; callers depend only on this port.
pub trait TaskRepository {
    /// Inserts a new task without overwriting an existing ID.
    fn insert(&mut self, task: &Task) -> Result<(), RepositoryError>;
    fn find(&self, id: u64) -> Result<Option<Task>, RepositoryError>;
}
