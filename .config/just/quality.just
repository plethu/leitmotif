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

check: fmt-check lint spelling workflows ci-policy
