"""Exercise the native harbour extension inside a clean headless Godot project."""

import argparse
import shutil
import tempfile
from pathlib import Path

from scripts.engine_check import CONSUMERS, ROOT, run

NATIVE = ROOT / "examples/godot-native"
COMPLETION = "Native harbour: 30 cases; both endings; full Recite trace replay.\n12 controlled assets; 8 lifetimes passed."


def execute(library, asset, output):
    """Copy built host artifacts, load the actual extension, then execute its host audit."""
    if not library.is_file():
        raise ValueError(f"native library does not exist: {library}")
    if not asset.is_file():
        raise ValueError(f"compiled dialogue asset does not exist: {asset}")
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="native-project-", dir=output) as temporary:
        consumer = Path(temporary) / "engine-contracts"
        shutil.copytree(
            CONSUMERS, consumer, ignore=shutil.ignore_patterns(".godot", "bin", "obj")
        )
        shutil.copy2(ROOT / "global.json", Path(temporary) / "global.json")
        project = consumer / "Godot"
        run(["dotnet", "restore", project / "Harbour.Godot.csproj", "--locked-mode"])
        run(["dotnet", "build", project / "Harbour.Godot.csproj", "--no-restore"])
        native = project / "native"
        native.mkdir()
        shutil.copy2(library, native / "libleitmotif_godot_native.so")
        shutil.copy2(NATIVE / "harbour.gdextension", native)
        shutil.copy2(asset, native / "harbour.recitec")
        shutil.copytree(asset.parent / "cases", native / "cases")
        base = ["godot-dotnet", "--headless", "--path", project]
        run(
            base + ["--editor", "--quit-after", "60", "--frame-delay", "10"],
            output=output / "editor.log",
            normal_godot=True,
        )
        run(
            base + ["res://Native.tscn"],
            output=output / "runtime.log",
            completion=COMPLETION,
            normal_godot=True,
        )
        print(COMPLETION)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--library", type=Path, required=True, help="Built native GDExtension library"
    )
    parser.add_argument(
        "--asset",
        type=Path,
        required=True,
        help="Compiled harbour asset with controlled assets in the adjacent cases directory",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Directory for logs and disposable clean-project builds",
    )
    args = parser.parse_args()
    execute(args.library.resolve(), args.asset.resolve(), args.output.resolve())


if __name__ == "__main__":
    main()
