//! Deliberately BAD architecture that RestrictImports currently does not catch.
pub mod infrastructure {
    pub struct Database;
}

pub mod domain {
    // No `use` item: the forbidden dependency lives in the type/body paths.
    pub fn forbidden_dependency() -> crate::infrastructure::Database {
        crate::infrastructure::Database
    }
}
