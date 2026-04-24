run:
    cargo run

ok:
    cargo check --all-targets
    cargo fmt --check
    cargo clippy

fmt:
    cargo fmt

# Run a specific example by name, e.g. `just example hello_usagi`.
example name:
    cargo run -- examples/{{ name }}.lua

examples:
    just example hello_usagi
    just example input
