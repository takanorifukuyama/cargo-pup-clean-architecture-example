//! Composition root: the only place that wires the two outer adapters together.

use clean_architecture_example::infrastructure::InMemoryOrderRepository;
use clean_architecture_example::presentation::submit_order;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut repository = InMemoryOrderRepository::default();
    let message = submit_order(&mut repository, "Coffee", 2, 600)?;
    println!("{message}");
    Ok(())
}
