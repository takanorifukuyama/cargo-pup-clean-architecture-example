pub mod infrastructure {
    pub struct Database;
}

pub mod application {
    // The use case must depend on its port, not a concrete adapter.
    pub use crate::infrastructure::Database;
}
