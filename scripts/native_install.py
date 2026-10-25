"""Verify, extract and install the bounded Leitmotif Linux Godot package."""

import argparse
import hashlib
import json
import re
import shutil
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path

OWNED = Path("native/extension")
MANIFEST = "manifest.json"
TARGET = "x86_64-unknown-linux-gnu"


def digest(data):
    """Hash the exact shipped bytes."""
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class Manifest:
    """Validated identity and inventory of one bounded native package."""

    source_revision: str
    artifacts: dict[str, str]
    profile: str = "dev"

    def encode(self):
        return (
            json.dumps(
                {
                    "format": 1,
                    "source_revision": self.source_revision,
                    "target": TARGET,
                    "profile": self.profile,
                    "godot": "4.6.3",
                    "bindings": "0.5.5",
                    "artifacts": self.artifacts,
                },
                sort_keys=True,
                indent=2,
            )
            + "\n"
        ).encode()

    @classmethod
    def decode(cls, data):
        """Reject incompatible identity, ambiguous paths, and malformed hashes."""
        value = json.loads(data)
        expected = {
            "format": 1,
            "target": TARGET,
            "godot": "4.6.3",
            "bindings": "0.5.5",
        }
        if (
            not isinstance(value, dict)
            or set(value) != {*expected, "source_revision", "artifacts", "profile"}
            or value["profile"] not in ("dev", "release")
            or any(value[key] != item for key, item in expected.items())
            or not isinstance(value["source_revision"], str)
            or not re.fullmatch(r"[0-9a-f]{40}", value["source_revision"])
            or not isinstance(value["artifacts"], dict)
            or not value["artifacts"]
            or not {
                "extension/libleitmotif_godot_native.so",
                "extension/harbour.gdextension",
            }.issubset(value["artifacts"])
        ):
            raise ValueError("unsupported native package manifest")
        for name, checksum in value["artifacts"].items():
            path = Path(name)
            if (
                path.is_absolute()
                or ".." in path.parts
                or path.as_posix() != name
                or (
                    not name.startswith(("extension/", "example/"))
                    and name not in {"install.py", "README.md"}
                )
                or not isinstance(checksum, str)
                or not re.fullmatch(r"[0-9a-f]{64}", checksum)
            ):
                raise ValueError("invalid native artifact inventory")
        return cls(value["source_revision"], value["artifacts"], value["profile"])


def extract(archive, destination):
    """Verify the complete archive inventory before writing any extracted files."""
    with zipfile.ZipFile(archive) as source:
        if MANIFEST not in source.namelist():
            raise ValueError("missing native package manifest")
        manifest = Manifest.decode(source.read(MANIFEST))
        if sorted(source.namelist()) != sorted([MANIFEST, *manifest.artifacts]):
            raise ValueError("archive does not match native package inventory")
        files = {name: source.read(name) for name in manifest.artifacts}
        if any(
            digest(data) != manifest.artifacts[name] for name, data in files.items()
        ):
            raise ValueError("native package artifact hash mismatch")
        if destination.exists():
            raise ValueError("extract destination must not exist")
        for name, data in {**files, MANIFEST: manifest.encode()}.items():
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)


def install(bundle, project):
    """Replace only the marked native directory, preserving authored project bytes."""
    manifest = Manifest.decode((bundle / MANIFEST).read_bytes())
    artifacts = {
        name.removeprefix("extension/"): checksum
        for name, checksum in manifest.artifacts.items()
        if name.startswith("extension/")
    }
    for name, checksum in artifacts.items():
        if digest((bundle / "extension" / name).read_bytes()) != checksum:
            raise ValueError("native package artifact hash mismatch")
    destination = project / OWNED
    if destination.is_symlink() or any(
        parent.is_symlink() for parent in destination.parents
    ):
        raise ValueError("native install path must not traverse links")
    if destination.exists():
        if not (destination / MANIFEST).is_file():
            raise ValueError("refusing to replace unmarked native directory")
        Manifest.decode((destination / MANIFEST).read_bytes())
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix="native-install-", dir=destination.parent))
    backup = temporary / "previous"
    try:
        stage = temporary / "extension"
        stage.mkdir()
        for name in artifacts:
            target = stage / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(bundle / "extension" / name, target)
        (stage / MANIFEST).write_bytes(manifest.encode())
        if destination.exists():
            destination.rename(backup)
        try:
            stage.rename(destination)
        except OSError:
            if backup.exists():
                backup.rename(destination)
            raise
        if backup.exists():
            shutil.rmtree(backup)
    finally:
        # A failed rollback retains the previous installation for recovery.
        if not backup.exists():
            shutil.rmtree(temporary)


def add_install_commands(commands):
    """Share the archive install contract between repository and shipped CLIs."""
    unpack = commands.add_parser(
        "extract",
        help="Verify a package ZIP into a new directory.",
        description="Verify the manifest and artifact hashes before extracting a package ZIP.",
    )
    unpack.add_argument("archive", type=Path, help="Package ZIP to verify and extract.")
    unpack.add_argument(
        "destination", type=Path, help="New directory for the verified package files."
    )
    deploy = commands.add_parser(
        "install",
        help="Install or replace the package-owned native/extension directory.",
        description=(
            "Replace only the package-owned native/extension directory in a Godot "
            "project. Refuse unmarked collisions; keep authored content outside it."
        ),
    )
    deploy.add_argument(
        "bundle",
        type=Path,
        help="Extracted package directory containing manifest.json and extension/.",
    )
    deploy.add_argument(
        "project",
        type=Path,
        help="Existing Godot project directory containing project.godot.",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    add_install_commands(commands)
    args = parser.parse_args()
    if args.command == "extract":
        extract(args.archive, args.destination)
    else:
        install(args.bundle, args.project)


if __name__ == "__main__":
    main()
