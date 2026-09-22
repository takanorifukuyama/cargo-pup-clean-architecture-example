//! Each snippet is inserted only into an isolated temporary copy.
//! It must compile normally, then fail with the expected cargo-pup diagnostic.

use crate::support::{self, architecture_violation, Edit, Expectation};

architecture_violation! {
    domain_to_application {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use crate::application::TaskService; }
    }
}

architecture_violation! {
    domain_to_infrastructure {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use crate::infrastructure::InMemoryTaskRepository; }
    }
}

architecture_violation! {
    domain_to_presentation {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use crate::presentation::TaskController; }
    }
}

architecture_violation! {
    application_to_infrastructure {
        rule: application_inward_only,
        in_file: "src/application.rs",
        add: { pub use crate::infrastructure::InMemoryTaskRepository; }
    }
}

architecture_violation! {
    application_to_presentation {
        rule: application_inward_only,
        in_file: "src/application.rs",
        add: { pub use crate::presentation::TaskController; }
    }
}

architecture_violation! {
    presentation_to_infrastructure {
        rule: presentation_no_direct_storage,
        in_file: "src/presentation.rs",
        add: { pub use crate::infrastructure::InMemoryTaskRepository; }
    }
}

architecture_violation! {
    infrastructure_to_application {
        rule: infrastructure_uses_domain_ports,
        in_file: "src/infrastructure.rs",
        add: { pub use crate::application::TaskService; }
    }
}

architecture_violation! {
    infrastructure_to_presentation {
        rule: infrastructure_uses_domain_ports,
        in_file: "src/infrastructure.rs",
        add: { pub use crate::presentation::TaskController; }
    }
}

architecture_violation! {
    nested_domain_to_infrastructure {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: {
            pub mod boundary_probe {
                pub use crate::infrastructure::InMemoryTaskRepository;
            }
        }
    }
}

architecture_violation! {
    relative_aliased_import {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: {
            pub use super::infrastructure::InMemoryTaskRepository as ForbiddenRepository;
        }
    }
}

architecture_violation! {
    domain_to_filesystem {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use std::fs::File; }
    }
}

// This is characterization, NOT permission to depend on infrastructure.
// Keep it explicit instead of making a normal violation test silently succeed.
#[test]
fn fully_qualified_path_known_gap() -> support::TestResult {
    support::check(
        "fully_qualified_path_known_gap",
        support::find_rule(crate::RULES, "domain_inward_only")?,
        Some(Edit {
            file: "src/domain.rs",
            code: stringify! {
                pub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {
                    crate::infrastructure::InMemoryTaskRepository::default()
                }
            },
        }),
        Expectation::KnownGap,
    )
}
