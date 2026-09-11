pub mod application {
    pub trait TaskRepository {}
}

pub mod infrastructure {
    // Unused, so Rust alone would not demand the trait implementation.
    pub struct BrokenTaskRepository;
}
