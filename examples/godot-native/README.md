# Native Godot harbour

Run `mise exec -- just engine setup`, then `mise exec -- just native check` from the
repository root. The native check builds this standalone GDExtension, compiles the
existing harbour dialogue with the pinned Recite compiler, and loads a clean copy
of the Godot .NET project. The aggregate check and engine CI lane run it too.

The existing composed harbour remains the sole model declaration. The Godot-owned
C# host executes its own actions using the existing shared `HarbourState`; the
extension receives complete typed snapshots and compares actual effects against
model predictions. Replay and dialogue reject stale live initial state before
any effects. The audit deliberately resets its dedicated host for each case.
Neither replay nor dialogue resets live state, and divergence retains accepted
host effects with structured source, expected/actual snapshots and progress.

The authoring process emits an official compiled Recite asset:

```sh
mise exec -- cargo run --locked -p leitmotif-recite --example harbour-dialogue -- \
  --compiled-asset /tmp/harbour.recitec
```

The native dialogue method receives those bytes as a Godot `PackedByteArray`.
The extension uses Recite's official decoder and runtime; it contains no dialogue
compiler or second authored source. Empty, truncated and unsupported assets fail
explicitly. The headless fixture checks both endings and the captured beacon
choice after the host has spent the oil on the ferry, including pending request
and failed acknowledgement behavior. Its fixture-specific event projection
records operations, typed query observations, requests, acknowledgements and
provenance, with complete host snapshots and official complete session bytes.
Fresh Godot sessions must reproduce the projected trace exactly. Optional Recite
presentation fields outside this harbour are not an engine integration contract.

This is an experimental Linux x86_64 consumer tested with Godot .NET 4.6.3,
Godot API 4.6 and godot-rust 0.5.5. It keeps strict runtime safeguards enabled.
Calls are synchronous on Godot's main thread; the extension retains no engine
objects after a call and starts no worker threads. Hot reload is disabled.
The host check exercises editor loading, repeated object creation/freeing and
ordinary process shutdown. Other Godot versions, platforms and exported games
have not been tested.

The local fixture copies the bounded engine-contracts source tree and SDK
selector into a temporary directory, dereferencing authored links and excluding
build caches. It performs a locked restore and builds that clean managed host
before copying the `.so`, `.gdextension`, compiled dialogue and controlled test
assets into its Godot project. Shipping an addon, installer, editor UI,
or cross-platform package remains separate work. This native leaf is excluded
from the core Cargo workspace; explicit native recipes enforce its formatting,
strict lints, locked tests, dependency policy and coverage. Its complete lint
table is checked against the root table, with only the narrowly scoped native
entry-point unsafe-code policy differing. Core crates retain `forbid`.
