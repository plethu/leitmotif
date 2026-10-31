# Leitmotif harbour consumer for Unity

Offline consumer for Leitmotif's harbour example. Rust analysis produces a request and
report; `Harbour.Consumer` validates the evidence and replays it against managed state
the game owns. Reports never set live state. No Rust or native code is in the player.

Install from a game project's repository checkout:

```sh
python -m scripts.unity_package install leitmotif-harbour-<version>.tgz <unity-project>
```

This copies the tarball to `Packages/leitmotif-harbour.tgz` and adds
`com.plethu.leitmotif.harbour` to `Packages/manifest.json`. Rerun it with a newer
tarball to upgrade; nothing under `Assets/` is touched. It refuses to install while
`Assets/` or embedded packages contain an assembly or asmdef named `Harbour`,
`Leitmotif.Harbour`, `System.Text.Json`, `System.Text.Encodings.Web`,
`System.IO.Pipelines`, `System.Runtime.CompilerServices.Unsafe`, or
`Microsoft.Bcl.AsyncInterfaces`, because Unity cannot load duplicate precompiled
assemblies. Remove or rename the game's copy, or point its code at this package's.

Add `HarbourRunner` to a GameObject and call `Replay(paths)` with a request and report
path. It returns 0 (reached), 1 (unreachable or divergence), or 2 (rejected or
inconclusive). Third-party licenses are in `THIRD-PARTY-NOTICES.md`.
