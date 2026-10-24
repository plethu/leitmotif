set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

fmt:
    taplo fmt

fmt-check:
    taplo fmt --check
    taplo lint

spelling:
    typos

workflows:
    actionlint

# Exercise routing against real Git histories, including deletions and renames.
ci-policy:
    python -m unittest discover -s tests/ci -v

check: fmt-check spelling workflows ci-policy
