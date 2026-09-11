//! A small, framework-independent Clean Architecture example.
//!
//! Dependency direction: presentation -> application -> domain.
//! Infrastructure implements the port owned by application.
#![forbid(unsafe_code)]

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
