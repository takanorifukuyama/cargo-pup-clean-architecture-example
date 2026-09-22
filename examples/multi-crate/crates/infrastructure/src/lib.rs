//! Replaceable adapter implementing the domain's persistence port.

use std::collections::{btree_map::Entry, BTreeMap};
use task_domain::{RepositoryError, Task, TaskRepository};

#[derive(Debug, Default)]
pub struct InMemoryTaskRepository {
    tasks: TaskStore,
}

#[derive(Debug, Default)]
pub(crate) struct TaskStore {
    entries: BTreeMap<u64, Task>,
}

impl TaskRepository for InMemoryTaskRepository {
    fn insert(&mut self, task: &Task) -> Result<(), RepositoryError> {
        match self.tasks.entries.entry(task.id()) {
            Entry::Vacant(entry) => {
                entry.insert(task.clone());
                Ok(())
            }
            Entry::Occupied(_) => Err(RepositoryError::DuplicateId(task.id())),
        }
    }

    fn find(&self, id: u64) -> Result<Option<Task>, RepositoryError> {
        Ok(self.tasks.entries.get(&id).cloned())
    }
}
