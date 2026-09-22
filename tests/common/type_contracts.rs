//! Sample-local macros using actual Rust bounds, not cargo-pup trait rules.

macro_rules! type_contracts {
    ($($name:ident {
        type: $ty:ty,
        implements: [$($bound:path),+ $(,)?],
    })+) => {
        $(#[test]
        fn $name() {
            fn assert_contract<T: $($bound)+*>() {}
            assert_contract::<$ty>();
        })+
    };
}

pub(crate) use type_contracts;
