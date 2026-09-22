//! Executable architecture rules. Run with --features architecture-tests.
//! Each selector must trigger a coverage canary before its real rule is checked.

#[path = "architecture/support.rs"]
mod support;

use support::{architecture_rules, visibility_rules};

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

visibility_rules! {
    task_store_stays_internal {
        struct_name: TaskStore,
        visibility: PubCrate,
    }
}

#[path = "architecture/violations.rs"]
mod violations;
#[path = "architecture/extended.rs"]
mod extended;
