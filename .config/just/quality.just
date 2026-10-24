set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

fmt:
    taplo fmt
    ruff format scripts tests
    dprint fmt
    rumdl fmt .
    just --unstable --fmt
    just --justfile .config/just/quality.just --unstable --fmt

fmt-check:
    taplo fmt --check
    taplo lint
    ruff format --check scripts tests
    dprint check
    rumdl fmt --check .
    just --unstable --fmt --check
    just --justfile .config/just/quality.just --unstable --fmt --check

lint:
    ruff check scripts tests
    rumdl check .

spelling:
    typos

workflows:
    actionlint

# Exercise routing against real Git histories, including deletions and renames.
ci-policy:
    python -m unittest discover -s tests/ci -v

# Check measurement collection and the serialized report consumer boundary.
tooling-tests:
    python -m unittest discover -s tests/tooling -v

# Measure all project Python, including child interpreters, without counting tests.
python-coverage:
    mkdir -p "${CARGO_TARGET_DIR:-target}/python-coverage"
    coverage erase --data-file "${CARGO_TARGET_DIR:-target}/python-coverage/.coverage"
    PYTHONPATH="$PWD" coverage run --data-file "${CARGO_TARGET_DIR:-target}/python-coverage/.coverage" -m unittest discover -s tests/ci -v
    PYTHONPATH="$PWD" coverage run --data-file "${CARGO_TARGET_DIR:-target}/python-coverage/.coverage" -m unittest discover -s tests/tooling -v
    coverage combine --data-file "${CARGO_TARGET_DIR:-target}/python-coverage/.coverage"
    coverage json --data-file "${CARGO_TARGET_DIR:-target}/python-coverage/.coverage" -o "${CARGO_TARGET_DIR:-target}/python-coverage/report.json"
    python scripts/coverage_gate.py --format python "${CARGO_TARGET_DIR:-target}/python-coverage/report.json"

check: fmt-check lint spelling workflows python-coverage
