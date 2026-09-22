#![forbid(unsafe_code)]

use std::error::Error;
use task_application::TaskService;
use task_infrastructure::InMemoryTaskRepository;
use task_presentation::TaskController;

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
