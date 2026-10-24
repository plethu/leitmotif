set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

# Formatting, spelling, workflows, and CI policy.
mod quality '.config/just/quality.just'

default:
    @just --list

# Deliberately format Rust and TOML sources.
fmt:
    cargo fmt --all
    just quality fmt

fmt-check:
    cargo fmt --all -- --check
    just quality fmt-check

clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test *args:
    cargo nextest run --workspace --all-features --locked "$@"

test-doc *args:
    cargo test --workspace --all-features --locked --doc "$@"

# Library semantics, examples, and public API documentation.
core-check:
    cargo fmt --all -- --check
    just clippy
    just test
    just test-doc
    RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --all-features --locked --no-deps

# Dependency advisories, licenses, sources, and unused dependencies.
supply-chain:
    cargo deny check advisories bans licenses sources
    cargo machete

# Complete acceptance gate; CI invokes the same constituent lanes.
check:
    just quality check
    just core-check
    just supply-chain

verify: check

# Run the Rust-authored demonstration fixtures.
demo *args:
    cargo run --locked -p leitmotif --example check -- "$@"
