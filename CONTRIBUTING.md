# Contributing

Leitmotif is an early experimental library, available from source on GitHub. There is no
crates.io release yet. Prefer a small executable example that exposes a semantic problem
over a new abstraction. APIs may change while the representative town and mystery
fixtures establish their requirements.

Use the pinned tools with `mise install` and run `mise exec -- just check`.
`mise run verify` runs the same aggregate gate. `mise exec -- just fmt` applies
formatting. With mise activated, the focused groups are `just quality check`,
`just core-check`, `just supply-chain`, and `just coverage`; run `just` or
`just quality` to list commands. `just core-check` needs no game engine. The private
symbolic-town experiment has a separate locked workspace; `just symbolic check` applies
its format, strict Clippy, test, dependency, and production coverage gates. The
aggregate check and selected Rust CI lane include it. The complete gate also runs the
optional-tool engine conformance lane; install its exact tools with
`mise exec -- just engine setup`. Contributions are under MIT OR Apache-2.0.

Coverage uses the separately pinned nightly compiler to measure branches. The gate
requires 90% line and 95% branch coverage across library and example code; run it at
integration checkpoints, using focused tests during development. The quality lane also
measures all project Python with pinned coverage.py, including subprocesses, and
enforces the same line and branch thresholds with `just quality python-coverage`. Tests
are not included in either language's production coverage totals.

CI always runs the lightweight quality lane: Rust-independent formatting, Python and
Markdown linting, TOML validation, spelling, workflow linting, and CI
policy checks. On pushes and pull requests, the Rust and dependency lane is skipped only
when every changed path is allowlisted as documentation-only by the repository's CI
scope policy. Unknown paths require the full checks. Scheduled and manually dispatched
runs always run the full checks. The engine lane executes the same harbour projection
and action traces in a .NET console and headless Godot .NET on Linux x86_64.

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

The root `global.json` selects the same exact SDK as mise and disables SDK roll-forward.
For focused engine work, run `mise exec -- just engine check`. Its locked NuGet restore,
SDK analyzer build, native Godot resource formatting, console checks, normal Godot
exits, and authored C# coverage are required together. CSharpier formats C# and project
XML; `just engine fmt` applies formatting. Godot resources are round-tripped with the
pinned engine's own serializers. Coverage uses a separate test scene that waits for the
actual production Node to complete its normal quit request, then flushes the pinned
Coverlet trackers before Godot completes its ordinary shutdown. Ordinary shutdown is
checked separately; the production Node never force-exits. The optional installed-Editor
Unity lane is `just engine unity-check`; see the
[Unity example](examples/unity-harbour/README.md) for exact versions, backends, and
platform limits. It stays outside the aggregate gate and hosted CI. Bevy integration is
not established.
