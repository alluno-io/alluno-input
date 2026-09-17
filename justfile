# alluno-input workspace tasks. CI mirrors `check` on all three platforms.

default:
    @just --list

# Format the whole workspace.
fmt:
    cargo fmt --all

# Format, then apply clippy's machine-applicable fixes.
fix:
    cargo fmt --all
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged

# The gate: format check, clippy denying warnings, tests.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Run the test suite.
test:
    cargo test --workspace

# Ask this machine what it can emulate.
probe:
    cargo run -q --bin alluno-input -- probe
