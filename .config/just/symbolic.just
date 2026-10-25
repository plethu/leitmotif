set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

fmt:
    cargo fmt --manifest-path examples/symbolic-town/Cargo.toml

fmt-check:
    cargo fmt --manifest-path examples/symbolic-town/Cargo.toml -- --check

core-check: fmt-check
    cargo clippy --manifest-path examples/symbolic-town/Cargo.toml --all-targets --all-features --locked -- -D warnings
    cargo nextest run --manifest-path examples/symbolic-town/Cargo.toml --all-features --locked
    cargo build --manifest-path examples/symbolic-town/Cargo.toml --all-features --locked
    RUSTDOCFLAGS=-Dwarnings cargo doc --manifest-path examples/symbolic-town/Cargo.toml --all-features --locked --no-deps

supply-chain:
    cargo deny --manifest-path examples/symbolic-town/Cargo.toml --config examples/symbolic-town/deny.toml --locked check advisories bans licenses sources
    cargo machete examples/symbolic-town

coverage:
    cargo +nightly-2026-06-10 llvm-cov nextest --manifest-path examples/symbolic-town/Cargo.toml --all-features --locked --branch --no-default-ignore-filename-regex --ignore-filename-regex '(^|/)(tests|target|rustc|\.cargo|\.rustup)/' --json --output-path "${CARGO_TARGET_DIR:-examples/symbolic-town/target}/symbolic-coverage.json"
    python scripts/coverage_gate.py "${CARGO_TARGET_DIR:-examples/symbolic-town/target}/symbolic-coverage.json"

check: core-check supply-chain coverage
