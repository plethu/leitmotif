# Headless Bevy harbour

Run `mise exec -- cargo run --locked -p bevy-harbour`. Run the headless
conformance tests with `mise exec -- cargo test --locked -p bevy-harbour`.

The executable reuses the composed harbour model from the Rust examples. Bevy
owns the live harbour component, entity identity, intent resource and `Update`
system. Stable authored action and fact keys cross the model boundary; entity
IDs do not. Each command advances one explicit update. The system executes its
own guards and effects; model successors and witness snapshots only serve as
comparison oracles. Branch checks reconstruct hosts from captured game state.

Analysis runs explicitly during authoring or CI, outside the update schedule.
The finite fixture checks all five actions in all six reachable states, including
rejected actions and both terminal states. It replays both three-action endings,
distinguishes unreachable from inconclusive analysis, rejects stale captured
state, and demonstrates a source-linked failure when lighting omits oil use.
The fixed fixture has at most six admitted states and 30 audited action attempts.
The tests assert those counts as well as the resulting states.

This is a direct in-process library experiment with no plugin wrapper, renderer,
window or save format. The game retains save and scheduling ownership. It does
not establish rendered gameplay, frame timing or interactive input behavior.

Minimal Bevy app/ECS dependencies and the existing derive_more upstream change
are pinned to exact Git revisions in Cargo. The source policy permits their
repositories; cargo-deny does not enforce URL revision queries. Exact-version
Zlib license exceptions cover foldhash 0.2.0 and slotmap 1.1.1. Rust 1.97.1 is
required by the selected Bevy revision; the core library retains its own MSRV.

Clippy's Bevy-local configuration permits only `syn` and `rustc-hash` duplicates
reported through inactive optional reflection metadata. Every other root Clippy
setting is retained. The dependency lane checks configuration parity and runs
cargo-deny against real duplicate normal dependencies to keep that separate
policy enforced. The workspace cargo-deny duplicate ban has no exemptions.
