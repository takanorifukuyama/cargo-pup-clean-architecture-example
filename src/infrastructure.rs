//! A replaceable in-memory implementation of the application's output port.
use crate::application::{SaveError, TaskRepository};
use crate::domain::Task;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct InMemoryTaskRepository {
    tasks: BTreeMap<u64, Task>,
}

impl InMemoryTaskRepository {
    pub fn get(&self, id: u64) -> Option<&Task> {
        self.tasks.get(&id)
    }
}

impl TaskRepository for InMemoryTaskRepository {
    fn save(&mut self, task: Task) -> Result<(), SaveError> {
        match self.tasks.entry(task.id()) {
            Entry::Vacant(entry) => {
                entry.insert(task);
                Ok(())
            }
            Entry::Occupied(_) => Err(SaveError::DuplicateId),
        }
    }
}
