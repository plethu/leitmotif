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
and test drivers are omitted; missing instrumentation and unknown sources fail.

These checks currently cover Linux x86_64, Godot .NET 4.6.3, and SDK 10.0.203. The .NET
console is not a Unity Editor, Mono, or IL2CPP integration. No GDExtension or Bevy
support is established by this fixture.

The Rust-only Recite prerequisite is runnable with:

```sh
mise exec -- cargo run --locked -p leitmotif-recite --example harbour-dialogue
```

It drives the same authored harbour through two ordinary endings and a captured beacon
choice whose oil has already been spent on the ferry. The later host guard rejects the
beacon request, and the dialogue continues only after a failed acknowledgement. Full
runtime boundaries are replayed against a fresh session. These are three bounded
scripted interactions, not an exhaustive combined dialogue/host search or a native Godot
integration.
