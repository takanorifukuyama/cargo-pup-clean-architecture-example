pub mod presentation {
    pub struct Controller;
}

pub mod infrastructure {
    // A storage adapter must not know about controllers.
    pub use crate::presentation::Controller;
}
