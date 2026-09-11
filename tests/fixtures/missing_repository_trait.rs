//! Known limitation: StructRule::ImplementsTrait is ignored in cargo-pup 0.1.8.
pub mod application {
    pub trait TaskRepository {}
}

pub mod infrastructure {
    // Public, so the separate visibility rule does not reject it.
    // Unused, so Rust alone would not demand the trait implementation either.
    pub struct BrokenTaskRepository;
}
