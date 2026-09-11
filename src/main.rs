#![forbid(unsafe_code)]

use clean_architecture::application::TaskService;
use clean_architecture::infrastructure::InMemoryTaskRepository;
use clean_architecture::presentation::TaskController;
use std::error::Error;

/// Composition root: this is the one place allowed to wire concrete adapters.
fn main() -> Result<(), Box<dyn Error>> {
    let title = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let title = if title.is_empty() {
        "Try architecture tests"
    } else {
        &title
    };
    let repository = InMemoryTaskRepository::default();
    let service = TaskService::new(repository);
    let mut controller = TaskController::new(service);
    let task = controller.create(1, title)?;
    println!("{}: {}", task.id, task.title);
    Ok(())
}
