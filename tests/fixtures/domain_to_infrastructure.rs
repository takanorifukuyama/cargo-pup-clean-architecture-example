pub mod infrastructure {
    pub struct Database;
}

pub mod domain {
    // Compiles, but the domain must not import a storage adapter.
    pub use crate::infrastructure::Database;
}
