# Working on this example

- Preserve the inward dependency direction. `application` owns `TaskRepository`; `infrastructure` implements it. `main.rs` is the composition root.
- Keep the runnable example small and dependency-free. This repository evaluates architecture checks, not a production task service.
- Keep `pup.ron` as the single source of truth for architecture rules. Do not create a weaker configuration just for fixtures.
- Every important new rule needs a compile-valid negative fixture and an assertion of its named diagnostic.
- Do not silently skip architecture checks when cargo-pup is missing. The explicit architecture test must fail; the ordinary stable test suite intentionally has it marked ignored.
- A passing import lint is not proof of a dependency-free domain. Preserve and explain the fully qualified path characterization test.
- Validate with `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`, and `cargo architecture-test`.
- Open a pull request for changes; do not merge without the owner's approval.
