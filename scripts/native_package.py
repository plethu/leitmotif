"""Package the bounded Linux harbour GDExtension and its self-contained example."""

import argparse
import subprocess
import zipfile
from pathlib import Path

from scripts import native_notices
from scripts.engine_check import CONSUMERS, ROOT
from scripts.native_install import MANIFEST, Manifest, digest, extract, install


def source_files(root):
    """Dereference only links inside the source root and exclude generated builds."""
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if {".godot", "bin", "obj"}.intersection(relative.parts):
            continue
        if not path.resolve().is_relative_to(root.resolve()):
            raise ValueError(f"example link escapes source root: {path}")
        if path.is_file():
            yield relative, path.read_bytes()


def build(library, asset, archive, revision, profile="dev", notices=None):
    """Archive the same built inputs deterministically; compilation is separate."""
    if notices is None:
        notices = native_notices.generate(revision)
    files = {
        "extension/libleitmotif_godot_native.so": library.read_bytes(),
        "extension/harbour.gdextension": (
            ROOT / "examples/godot-native/harbour.gdextension"
        ).read_bytes(),
        "extension/THIRD-PARTY-NOTICES.json": notices,
        "install.py": (ROOT / "scripts/native_install.py").read_bytes(),
        "README.md": (ROOT / "examples/godot-native/PACKAGE.md").read_bytes(),
        "extension/LICENSE-MIT": (ROOT / "LICENSE-MIT").read_bytes(),
        "extension/LICENSE-APACHE": (ROOT / "LICENSE-APACHE").read_bytes(),
        "extension/ATTRIBUTIONS.md": (ROOT / "ATTRIBUTIONS.md").read_bytes(),
        "example/global.json": (ROOT / "global.json").read_bytes(),
        "example/engine-contracts/Godot/native/harbour.recitec": asset.read_bytes(),
    }
    for relative, data in source_files(CONSUMERS):
        files[f"example/engine-contracts/{relative.as_posix()}"] = data
    for relative, data in source_files(asset.parent / "cases"):
        files[f"example/engine-contracts/Godot/native/cases/{relative.as_posix()}"] = (
            data
        )
    manifest = Manifest(
        revision, {name: digest(data) for name, data in files.items()}, profile
    )
    Manifest.decode(manifest.encode())
    files[MANIFEST] = manifest.encode()
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            output.writestr(info, data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    package = commands.add_parser("build")
    package.add_argument("--library", type=Path, required=True)
    package.add_argument("--asset", type=Path, required=True)
    package.add_argument("--archive", type=Path, required=True)
    package.add_argument("--profile", choices=("dev", "release"), default="release")
    unpack = commands.add_parser("extract")
    unpack.add_argument("archive", type=Path)
    unpack.add_argument("destination", type=Path)
    deploy = commands.add_parser("install")
    deploy.add_argument("bundle", type=Path)
    deploy.add_argument("project", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        build(args.library, args.asset, args.archive, revision, args.profile)
    elif args.command == "extract":
        extract(args.archive, args.destination)
    else:
        install(args.bundle, args.project)


if __name__ == "__main__":
    main()
