run:
    cargo run

ok:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt

# Run a specific example by name. Works for flat files (`just example hello_usagi`)
# and directory examples (`just example spr` → examples/spr/main.lua).
example name:
    cargo run -- examples/{{ name }}

examples:
    just example hello_usagi
    just example input
    just example spr
