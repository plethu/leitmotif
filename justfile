set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

# Formatting, spelling, workflows, and CI policy.
mod quality '.config/just/quality.just'

default:
    @just --list

# Format every authored language with the pinned project tools.
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

# Include executable examples, excluding tests and dependency/toolchain sources.
coverage:
    cargo +nightly-2026-06-10 llvm-cov nextest --workspace --all-features --locked --branch --no-default-ignore-filename-regex --ignore-filename-regex '(^|/)(tests|target|rustc|\.cargo|\.rustup)/' --json --output-path "${CARGO_TARGET_DIR:-target}/coverage.json"
    python scripts/coverage_gate.py "${CARGO_TARGET_DIR:-target}/coverage.json"

# Complete acceptance gate; CI invokes the same constituent lanes.
check:
    just quality check
    just core-check
    just supply-chain
    just coverage
    just report-check

verify: check

# Run the Rust-authored demonstration fixtures.
demo *args:
    cargo run --locked -p leitmotif --example check -- "$@"

# Repeat isolated release-mode town measurements and emit JSON Lines.
town-scaling *args:
    python scripts/measure_town.py "$@"

# Gate the actual archive runtime report with the independent serialized-data consumer.
report-check:
    mkdir -p "${CARGO_TARGET_DIR:-target}/report-consumer"
    cargo run --locked -p leitmotif-recite --example archive -- --json > "${CARGO_TARGET_DIR:-target}/report-consumer/archive.json"
    python scripts/report_consumer.py "${CARGO_TARGET_DIR:-target}/report-consumer/archive.json" --summary "${GITHUB_STEP_SUMMARY:-${CARGO_TARGET_DIR:-target}/report-consumer/summary.md}"
