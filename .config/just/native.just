set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

fmt:
    cargo fmt --manifest-path examples/godot-native/Cargo.toml

fmt-check:
    cargo fmt --manifest-path examples/godot-native/Cargo.toml -- --check

core-check: fmt-check
    cargo clippy --manifest-path examples/godot-native/Cargo.toml --all-targets --all-features --locked -- -D warnings
    cargo nextest run --manifest-path examples/godot-native/Cargo.toml --all-features --locked
    cargo test --manifest-path examples/godot-native/Cargo.toml --all-features --locked --doc
    RUSTDOCFLAGS=-Dwarnings cargo doc --manifest-path examples/godot-native/Cargo.toml --all-features --locked --no-deps

supply-chain:
    cargo deny --manifest-path examples/godot-native/Cargo.toml --config deny.toml --locked check advisories bans licenses sources
    cargo machete examples/godot-native

host-check:
    cargo build --manifest-path examples/godot-native/Cargo.toml --locked --release
    mkdir -p "${CARGO_TARGET_DIR:-target}/native-host"
    cargo run --locked -p leitmotif-recite --example harbour-dialogue -- --compiled-asset "${CARGO_TARGET_DIR:-target}/native-host/harbour.recitec"
    cargo run --locked -p leitmotif-recite --example harbour-native-cases -- "${CARGO_TARGET_DIR:-target}/native-host/cases"
    python -m scripts.native_check --library "${CARGO_TARGET_DIR:-examples/godot-native/target}/release/libleitmotif_godot_native.so" --profile release --asset "${CARGO_TARGET_DIR:-target}/native-host/harbour.recitec" --output "${CARGO_TARGET_DIR:-target}/native-host"

# Ship only the verified Linux release package; compilation remains locked.
package: supply-chain host-check
    python -m scripts.native_package build --profile release --library "${CARGO_TARGET_DIR:-examples/godot-native/target}/release/libleitmotif_godot_native.so" --asset "${CARGO_TARGET_DIR:-target}/native-host/harbour.recitec" --archive "${CARGO_TARGET_DIR:-target}/leitmotif-godot-linux-x86_64.zip"

coverage: host-check
    #!/usr/bin/env bash
    set -euo pipefail
    asset="${CARGO_TARGET_DIR:-target}/native-host/harbour.recitec"
    asset="$(realpath "$asset")"
    manifest="$PWD/examples/godot-native/Cargo.toml"
    export CARGO_LLVM_COV_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}/native-coverage"
    export CARGO_TARGET_DIR="$CARGO_LLVM_COV_TARGET_DIR"
    cargo +nightly-2026-06-10 llvm-cov clean --manifest-path "$manifest" --workspace
    cargo +nightly-2026-06-10 llvm-cov nextest --manifest-path "$manifest" --all-features --locked --branch --no-report
    pushd examples/godot-native >/dev/null
    eval "$(cargo +nightly-2026-06-10 llvm-cov show-env --sh)"
    export RUSTFLAGS="${RUSTFLAGS:-} -Z coverage-options=branch"
    cargo +nightly-2026-06-10 build --locked
    popd >/dev/null
    python -m scripts.native_check --library "$CARGO_TARGET_DIR/debug/libleitmotif_godot_native.so" --asset "$asset" --output "$CARGO_TARGET_DIR/native-host"
    cargo +nightly-2026-06-10 llvm-cov report --manifest-path "$manifest" --no-default-ignore-filename-regex --ignore-filename-regex '(^|/)(tests|target|rustc|\.cargo|\.rustup)/' --json --output-path "$CARGO_TARGET_DIR/native-coverage.json"
    python scripts/coverage_gate.py "$CARGO_TARGET_DIR/native-coverage.json"

check: core-check supply-chain host-check coverage
