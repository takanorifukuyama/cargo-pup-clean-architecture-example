//! Composition root: the one place that selects and connects concrete adapters.
use clean_architecture::application::CreateTask;
use clean_architecture::infrastructure::InMemoryTaskRepository;
use clean_architecture::presentation::{create_task, CreateTaskRequest};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let title = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "設計をテストする".to_owned());
    let mut repository = InMemoryTaskRepository::default();
    let mut use_case = CreateTask::new(&mut repository);
    let response = create_task(&mut use_case, CreateTaskRequest { id: 1, title })?;
    println!("Created task #{}: {}", response.id, response.title);
    Ok(())
}
