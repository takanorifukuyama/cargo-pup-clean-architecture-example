pub mod application {
    pub trait TaskRepository {}
}

pub mod infrastructure {
    // This project's CLI constructs adapters from a separate binary crate.
    pub(crate) struct HiddenTaskRepository;

    impl crate::application::TaskRepository for HiddenTaskRepository {}
}
