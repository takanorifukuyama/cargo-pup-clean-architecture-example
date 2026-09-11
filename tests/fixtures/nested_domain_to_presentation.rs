pub mod presentation {
    pub struct Controller;
}

pub mod domain {
    pub mod nested {
        // Ensures that the matcher covers descendants, not only the root.
        pub use crate::presentation::Controller;
    }
}
