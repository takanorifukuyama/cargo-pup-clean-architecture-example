mod fixture;
mod policy;
#[path = "../../../../tests/common/runtime.rs"]
mod runtime;

pub use fixture::Fixture;
pub use policy::{check_all, check_rule, coverage, Kind, Problem, Rule};
pub use runtime::TestResult;

// Declarations produce normal Rust tests; there is no proc-macro or nightly API.
macro_rules! crate_dependencies {
    ($($name:ident {
        package: $package:literal,
        allow_normal: [$($normal:literal),* $(,)?],
        allow_dev: [$($dev:literal),* $(,)?],
        allow_build: [$($build:literal),* $(,)?],
    })+) => {
        const RULES: &[$crate::support::Rule] = &[
            $($crate::support::Rule {
                name: stringify!($name),
                package: $package,
                normal: &[$($normal),*],
                dev: &[$($dev),*],
                build: &[$($build),*],
            }),+
        ];
        $(#[test]
        fn $name() -> $crate::support::TestResult {
            let fixture = $crate::support::Fixture::new(stringify!($name))?;
            let metadata = fixture.metadata()?;
            $crate::support::coverage(&metadata, RULES)?;
            let rule = RULES.iter().find(|r| r.name == stringify!($name))
                .ok_or("Missing generated rule")?;
            $crate::support::check_rule(&metadata, rule)?;
            fixture.pass("Declared dependencies conform")
        })+
    };
}

pub(crate) use crate_dependencies;
