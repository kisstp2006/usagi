run:
    cargo run

ok:
    cargo build --all-targets
    cargo fmt --check
    cargo clippy

fmt:
    cargo fmt
