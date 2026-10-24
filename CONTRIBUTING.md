# Contributing

Leitmotif is an early experimental library, available from source on GitHub. There is no
crates.io release yet. Prefer a small executable example that exposes a semantic problem
over a new abstraction. APIs may change while the representative town and mystery
fixtures establish their requirements.

Use the pinned tools with `mise install` and run `mise exec -- just check`.
`mise run verify` runs the same aggregate gate. `mise exec -- just fmt` applies
formatting. With mise activated, the focused groups are `just quality check`,
`just core-check`, `just supply-chain`, and `just coverage`; run `just` or
`just quality` to list commands. No test services or game engine are required.
Contributions are under MIT OR Apache-2.0.

Coverage uses the separately pinned nightly compiler to measure branches. The gate
requires 90% line and 95% branch coverage across library and example code; run it at
integration checkpoints, using focused tests during development.

CI always runs the lightweight quality lane: Rust-independent formatting, Python and
Markdown linting, TOML validation, spelling, workflow linting, and CI
policy checks. On pushes and pull requests, the Rust and dependency lane is skipped only
when every changed path is allowlisted as documentation-only by the repository's CI
scope policy. Unknown paths require the full checks. Scheduled and manually dispatched
runs always run the full checks.

Changes to `main` go through pull requests, with `Check complete` required. It succeeds
only when every selected lane succeeds; an intentional documentation-only skip is
accepted, but failures and cancellations are not.

Changes to analysis need executable acceptance cases and counterexamples through the
public API. State exactly what the tests establish; incomplete or unsupported analysis
must not be presented as a proof.

Issues and design feedback are welcome. Please discuss substantial changes before coding
so we can agree on their scope. GitHub issues track active work; small issues ready for
implementation should state acceptance criteria, checks, and dependencies. The roadmap's
points are relative sizing, not time estimates.

Use purpose-first branches such as `feat/recite-guards` or `test/ending-availability`,
and sign your commits. Link the relevant issue in the PR body with `Refs #N`.
