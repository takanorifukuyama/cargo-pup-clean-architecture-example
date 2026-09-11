//! Business data and invariants. No persistence or presentation dependencies.
use std::error::Error;
use std::fmt;

pub const MAX_TITLE_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: u64,
    title: String,
}

impl Task {
    pub fn new(id: u64, title: &str) -> Result<Self, InvalidTitle> {
        let title = title.trim();
        if title.is_empty() {
            return Err(InvalidTitle::Empty);
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(InvalidTitle::TooLong);
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
pub enum InvalidTitle {
    Empty,
    TooLong,
}

impl fmt::Display for InvalidTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "task title must not be empty"),
            Self::TooLong => write!(f, "task title must be at most {MAX_TITLE_CHARS} characters"),
        }
    }
}

impl Error for InvalidTitle {}

#[cfg(test)]
mod tests {
    use crate::domain::{InvalidTitle, Task, MAX_TITLE_CHARS};

    #[test]
    fn trims_title() {
        let task = Task::new(1, "  test the design  ").unwrap();
        assert_eq!(task.title(), "test the design");
        assert_eq!(task.id(), 1);
    }

    #[test]
    fn rejects_blank_title() {
        assert_eq!(Task::new(1, " \t\n"), Err(InvalidTitle::Empty));
    }

    #[test]
    fn rejects_too_long_title() {
        assert_eq!(
            Task::new(1, &"a".repeat(MAX_TITLE_CHARS + 1)),
            Err(InvalidTitle::TooLong)
        );
    }

    #[test]
    fn length_is_unicode_scalar_count_not_bytes() {
        assert!(Task::new(1, &"あ".repeat(MAX_TITLE_CHARS)).is_ok());
        assert_eq!(
            Task::new(1, &"あ".repeat(MAX_TITLE_CHARS + 1)),
            Err(InvalidTitle::TooLong)
        );
    }
}
