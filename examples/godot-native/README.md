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

This is an experimental Linux x86_64 consumer tested with Godot .NET 4.6.3, Godot API
4.6 and godot-rust 0.5.5. Development and coverage builds use godot-rust’s default
strict runtime safeguards; release packages use its default balanced safeguards. Calls
are synchronous on Godot's main thread; the extension retains no engine objects after a
call and starts no worker threads. Hot reload is disabled. The host check exercises
editor loading, repeated object creation/freeing and ordinary process shutdown. Other
Godot versions, platforms and exported games have not been tested.

The package copies the bounded engine-contracts source tree and SDK selector,
dereferencing authored links and excluding build caches. It includes the `.so`,
`.gdextension`, compiled dialogue and controlled test assets. Editor UI and
cross-platform delivery remain separate work. This native leaf is excluded
from the core Cargo workspace; explicit native recipes enforce its formatting,
strict lints, locked tests, dependency policy and coverage. Its complete lint
table is checked against the root table, with only the narrowly scoped native
entry-point unsafe-code policy differing. Core crates retain `forbid`.

Run `mise exec -- just native package` to build and verify the Linux x86_64 release
archive. It contains the extension, licenses, prior-art attributions, dependency
notices, a versioned SHA-256 inventory, and the self-contained managed harbour example
with the exact SDK and NuGet locks. The host gate packages the same built inputs twice
and compares archive bytes; this checks archive determinism, not compiler
reproducibility. It extracts the archive, builds the locked example, and runs the
existing editor and runtime audit before and after extension replacement.

Extract with `python -m scripts.native_package extract ARCHIVE DESTINATION`, then
install with `python -m scripts.native_package install DESTINATION PROJECT`, where
`PROJECT` is the example’s `example/engine-contracts/Godot` directory or an existing
Godot project. Installation owns only `native/extension`; it refuses unmarked
collisions and replaces that marked directory, including obsolete files. Keep
authored dialogue, scenes and compiled assets outside it. The example’s compiled
fixtures live under `native` alongside that directory. This package establishes
Linux headless editor/runtime integration only; exported games and other platforms
remain unverified.

`THIRD-PARTY-NOTICES.json` retains cargo-about’s selected license texts and exact
crate versions, source identities and source-archive URLs, including the MPL
binding sources. The pinned generator uses the existing `cargo deny` policy for
license choice; `cargo deny` remains the dependency policy gate. No engine binary
or managed dependencies are redistributed; the example restores its NuGet locks.
