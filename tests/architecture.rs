//! Executable architecture rules. Start here, not in a Python script or RON file.
//!
//! Run: cargo test --locked --features architecture-tests --test architecture
//! The local macro generates ordinary #[test] functions; cargo-pup does the analysis.

#[path = "architecture/support.rs"]
mod support;

use support::architecture_rules;

architecture_rules! {
    domain_inward_only {
        module: clean_architecture::domain,
        deny_imports: [
            crate::application,
            crate::infrastructure,
            crate::presentation,
            axum,
            sqlx,
            tokio,
            reqwest,
            std::fs,
            std::net,
            std::process,
        ],
    }
    application_inward_only {
        module: clean_architecture::application,
        deny_imports: [
            crate::infrastructure,
            crate::presentation,
            axum,
            sqlx,
            reqwest,
        ],
    }
    presentation_no_direct_storage {
        module: clean_architecture::presentation,
        deny_imports: [crate::infrastructure, sqlx],
    }
    infrastructure_uses_domain_ports {
        module: clean_architecture::infrastructure,
        deny_imports: [crate::application, crate::presentation],
    }
}

// Mutation tests use the SAME rules, rather than a second configuration file.
#[path = "architecture/violations.rs"]
mod violations;
