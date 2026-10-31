# Unity harbour consumer

This optional Linux x86_64 example follows Recite's Unity version policy: Unity
6.7+ is the primary target; Unity 2022.3 and Mono receive best-effort compatibility.
The installed verification pins are 6000.7.0b2 and 2022.3.62f3. A beta Editor test
establishes behavior for that exact build, not future releases.

| Editor version | Linux x86_64 player | Verified stripping | Result |
| --- | --- | --- | --- |
| 6000.7.0b2 | IL2CPP (primary) | High | Editor and player suites passed |
| 6000.7.0b2 | CoreCLR (experimental) | Minimal | Editor and player suites passed |
| 2022.3.62f3 | Mono (best effort) | High | Editor and player suites passed |

Run from the repository root with an installed, licensed Editor and its Linux
IL2CPP build support. Unity's Linux compiler/linker also require compatible
`libncurses.so.6` and `libxml2.so.2`; if the host only supplies newer library names,
expose installed compatibility libraries through `LD_LIBRARY_PATH` for this command:

```sh
UNITY_EDITOR="$HOME/Unity/Hub/Editor/6000.7.0b2/Editor/Unity" \
  mise exec -- just engine unity-check
```

The command creates a clean temporary Unity project, generates four reports with the
Rust example, restores locked managed dependencies, imports the .NET Standard 2.1
artifacts, executes the Editor checks, and builds and runs a Linux standalone player
with high managed stripping for IL2CPP/Mono and minimal stripping for the CoreCLR
preview. Each component owns an independent `Consumer`; report
snapshots are comparison evidence and never set live host state. All 30 state/action
pairs (85 action attempts per audit), both endings, unreachable and inconclusive
outcomes, and 62 adversarial cases run through the component. The player invokes the
same suite from its scene's ordinary `Start` lifecycle and quits normally. The temporary
project and player are removed on exit; Editor/build/player logs remain in
`target/unity-IL2CPP`. Set `HARBOUR_UNITY_OUTPUT` to retain logs elsewhere.

For the other installed lanes, select an explicit backend:

```sh
HARBOUR_UNITY_BACKEND=Mono2x \
  UNITY_EDITOR="$HOME/Unity/Hub/Editor/2022.3.62f3/Editor/Unity" \
  mise exec -- just engine unity-check
HARBOUR_UNITY_BACKEND=CoreCLR \
  UNITY_EDITOR="$HOME/Unity/Hub/Editor/6000.7.0b2/Editor/Unity" \
  mise exec -- just engine unity-check
```

CoreCLR is a separate experimental player probe; it does not establish CoreCLR Editor
support. High stripping on this preview aborted in `System.Type` initialization before
the scene ran (`FieldAccessException`); the explicit CoreCLR lane uses minimal stripping
and passes. High stripping remains required for the IL2CPP and Mono lanes. Unavailable
backends and failed builds fail the check. Other operating systems, architectures,
WebGL, mobile players, and existing projects with conflicting JSON dependencies are
untested.

For an interactive walkthrough, copy this project to a writable directory. Build
`examples/engine-contracts/tests/UnityManaged/Harbour.Acceptance.csproj` in Release
and copy its `bin/Release/netstandard2.1/*.dll` into `Assets/Plugins`. Generate the
four request/check pairs as in `scripts/unity-check.sh` and put them in
`Assets/StreamingAssets`. Open the project in the selected Editor. Create a GameObject
with `HarbourRunner` and `HarbourAcceptance` attached, then enter Play Mode. Inspect
Console messages for conformance, rejected evidence, and divergence diagnostics
with the original Rust source paths. The acceptance component is test-only; a game
uses `HarbourRunner.Replay` with its request/report paths and handles status 0
(reached), 1 (unreachable or divergence), or 2 (rejected or inconclusive).

The agreed finite contract uses offline Rust analysis and precompiled managed
validation/replay. This keeps the analyser, Rust native binaries, ABI ownership, and
platform-specific allocation/deallocation rules outside the player. A native C ABI would
only be justified by an in-process analysis requirement; it would need separate handle,
buffer, callback, and distribution acceptance. The current scene is an executable
contract fixture, not a general Unity package or authoring interface. It has no UPM
release and is intentionally outside ordinary core builds and hosted CI. Unity's
[managed plug-in guidance](https://docs.unity3d.com/6000.0/Documentation/Manual/plug-ins-managed.html)
and
[stripping preservation guidance](https://docs.unity3d.com/6000.0/Documentation/Manual/managed-code-stripping-preserving.html)
apply when adapting the artifacts to a game's deployment.
