pub mod infrastructure {
    pub struct Database;
}

pub mod presentation {
    // A controller must go through a use case rather than importing storage.
    pub use crate::infrastructure::Database;
}
