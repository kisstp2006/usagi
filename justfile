run:
    cargo run

ok:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt

# Run a specific example by name, e.g. `just example hello_usagi`.
example name:
    cargo run -- examples/{{ name }}.lua

examples:
    just example hello_usagi
    just example input
