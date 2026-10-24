# Harbour engine contracts

Run `mise exec -- just engine setup`, then `mise exec -- just engine check` from the
repository root. The gate runs the same six-fact harbour through independent Rust,
C# console, and actual Godot .NET hosts. It checks all five actions at each of six
reachable states, including terminal rejections, then replays both three-action endings.
The existing Rust composition owns the authored model. C# owns its live world and
implements the five actions independently; report snapshots are comparison evidence.

The executable request is an experimental versioned projection of six facts. It is
neither an authored model nor a save format. Model fingerprints identify semantics;
`source_revision` is a caller/build declaration kept separately. A mismatched live
initial projection, identity, schema, or limit is rejected before host actions. Invalid
values and unknown actions are explicit errors. Inconclusive and unreachable reports
retain distinct status codes rather than becoming successful reachability claims.

For an individual report, use the Rust `engine-contracts` example to emit a request and
check it, then run the C# console with the request/report paths:

```sh
mise exec -- cargo run --locked -p leitmotif --example engine-contracts -- \
  --source-revision local request > /tmp/harbour.request.json
mise exec -- cargo run --locked -p leitmotif --example engine-contracts -- \
  --source-revision local check /tmp/harbour.request.json > /tmp/harbour.check.json
mise exec -- dotnet run \
  --project examples/engine-contracts/Console/Harbour.Console.csproj \
  -- /tmp/harbour.request.json /tmp/harbour.check.json
```

For the actual Godot host, build `Godot/Harbour.Godot.csproj`, then invoke:

```sh
mise exec -- godot-dotnet --headless --path examples/engine-contracts/Godot -- \
  /tmp/harbour.request.json /tmp/harbour.check.json
```

Statuses are 0 reached, 1 unreachable or host divergence, and 2 rejected, unsupported,
or inconclusive. Host actions accepted before a divergence are not rolled back. The
audit resets from its own captured initial state for each prefix and never hydrates a
model successor into live state.

Godot coverage uses a test-only scene after ordinary Node completion. Its managed exit
flushes Coverlet but emits native cleanup warnings, which the gate preserves. Separate
normal-exit checks must pass. Coverage includes every authored production C# file,
including the console entrypoint and Godot Node. Only identified generated build output
and test drivers are omitted. The empty compiler marker is checked against its exact
declaration before classifying executable sources; added behavior fails. Missing
instrumentation and unknown sources fail.

These checks currently cover Linux x86_64, Godot .NET 4.6.3, and SDK 10.0.203. The .NET
console is not a Unity Editor, Mono, or IL2CPP integration. No GDExtension or Bevy
support is established by this fixture.

The Shared library is compiled once for .NET Standard 2.1 with the pinned modern
.NET SDK; Console and Godot consume that same target. The acceptance runner checks
the loaded library's target before replaying evidence. Modern C# source is retained:
this is a precompiled library route, not a promise that Unity can compile the source.

A precompiled deployment needs `Harbour.dll` and the runtime assets selected for its
target. Build Shared with `-p:CopyLocalLockFileAssemblies=true` to materialize that
candidate set: System.Text.Json, System.Text.Encodings.Web, System.IO.Pipelines,
Microsoft.Bcl.AsyncInterfaces, and System.Runtime.CompilerServices.Unsafe. Package
versions are locked. The .NET 10 consumers select their own framework-compatible
assets; their tests do not execute that candidate closure under Unity. Editor import,
assembly conflicts, Mono, IL2CPP, stripping, and player distribution remain untested.
