# Leitmotif native harbour package

This experimental Linux x86_64 package targets Godot .NET 4.6.3 and .NET SDK
10.0.203, using godot-rust 0.5.5. Install those exact tools first. NuGet restore
needs network access. The SDK selector disables roll-forward.

Unzip the archive into a new directory. From that directory, install the extension
into the included example and run its existing audit:

```sh
python3 install.py install . example/engine-contracts/Godot
dotnet restore example/engine-contracts/Godot/Harbour.Godot.csproj --locked-mode
dotnet build example/engine-contracts/Godot/Harbour.Godot.csproj --no-restore
godot-dotnet --headless --editor --path example/engine-contracts/Godot \
  --quit-after 60 --frame-delay 10
godot-dotnet --headless --path example/engine-contracts/Godot res://Native.tscn
```

`godot-dotnet` denotes the official Godot .NET executable. The expected completion
is `Native harbour: 30 cases; both endings; full Recite trace replay.` followed by
`12 controlled assets; 8 lifetimes passed.` Calls run synchronously on Godot's main
thread. Hot reload is disabled; exported games and other platforms are unverified.

For a subsequent package, unzip into a new directory and run its `install.py`
against the existing project. The installer reserves the entire marked
`native/extension` directory for this package and removes obsolete files inside
it. Keep authored dialogue, scenes and assets outside that directory. Unmarked
collisions are refused. The example's compiled dialogue and test cases live under
`native`, outside the extension directory. Installation verifies shipped extension
hashes before replacing it and restores the previous directory if replacement fails.

`manifest.json` records source revision, native build profile, target, engine and
binding versions, and all artifact hashes. Release builds use godot-rust's default
balanced safeguards; development builds use strict safeguards. The archive's
repeatability covers packaging the same built inputs, not compiler reproducibility.

The extension includes MIT and Apache-2.0 license texts, research attributions, and
`THIRD-PARTY-NOTICES.json` with dependency license texts and exact source-archive
URLs. The MPL binding sources are available through those pinned source URLs.
The package includes no engine binary or restored managed dependencies.
