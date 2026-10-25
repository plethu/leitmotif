"""Exercise the native harbour extension inside a clean headless Godot project."""

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

from scripts import native_install, native_package
from scripts.engine_check import ROOT, run

NATIVE = ROOT / "examples/godot-native"
COMPLETION = "Native harbour: 30 cases; both endings; full Recite trace replay.\n12 controlled assets; 8 lifetimes passed."


def execute(library, asset, output, profile="dev"):
    """Copy built host artifacts, load the actual extension, then execute its host audit."""
    if not library.is_file():
        raise ValueError(f"native library does not exist: {library}")
    if not asset.is_file():
        raise ValueError(f"compiled dialogue asset does not exist: {asset}")
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="native-project-", dir=output) as temporary:
        temporary = Path(temporary)
        revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        first, second = temporary / "first.zip", temporary / "second.zip"
        notices = native_package.native_notices.generate(revision)
        native_package.build(library, asset, first, revision, profile, notices)
        native_package.build(library, asset, second, revision, profile, notices)
        if first.read_bytes() != second.read_bytes():
            raise ValueError("same built inputs produced different native archives")
        bundle = temporary / "extracted"
        native_package.extract(first, bundle)
        project = bundle / "example/engine-contracts/Godot"
        subprocess.run(
            [
                sys.executable,
                str(bundle / "install.py"),
                "install",
                str(bundle),
                str(project),
            ],
            cwd=bundle,
            check=True,
            timeout=120,
        )
        check_project(project, output, "installed")
        obsolete = project / native_install.OWNED / "obsolete.gd"
        obsolete.write_bytes(b"obsolete owned fixture")
        authored = project / "native/author-owned.txt"
        authored.write_bytes(b"authored dialogue and scene data\n")
        before = {
            path.relative_to(project): path.read_bytes()
            for path in (project / "native").rglob("*")
            if path.is_file()
            and not path.is_relative_to(project / native_install.OWNED)
        }
        subprocess.run(
            [
                sys.executable,
                str(bundle / "install.py"),
                "install",
                str(bundle),
                str(project),
            ],
            cwd=bundle,
            check=True,
            timeout=120,
        )
        if obsolete.exists() or any(
            (project / path).read_bytes() != data for path, data in before.items()
        ):
            raise ValueError("native replacement violated package ownership")
        check_project(project, output, "replaced")
        print(COMPLETION)


def check_project(project, output, label):
    """Build locked managed sources and require the established native host audit."""
    run(["dotnet", "restore", project / "Harbour.Godot.csproj", "--locked-mode"])
    run(["dotnet", "build", project / "Harbour.Godot.csproj", "--no-restore"])
    base = ["godot-dotnet", "--headless", "--path", project]
    run(
        base + ["--editor", "--quit-after", "60", "--frame-delay", "10"],
        output=output / f"{label}-editor.log",
        normal_godot=True,
    )
    run(
        base + ["res://Native.tscn"],
        output=output / f"{label}-runtime.log",
        completion=COMPLETION,
        normal_godot=True,
    )


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
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    args = parser.parse_args()
    execute(
        args.library.resolve(),
        args.asset.resolve(),
        args.output.resolve(),
        args.profile,
    )


if __name__ == "__main__":
    main()
