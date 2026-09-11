//! A small order application with explicit architectural boundaries.
//!
//! Modules are intentionally in one crate so forbidden dependencies still compile:
//! this lets the architecture tests demonstrate what cargo-pup actually detects.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
