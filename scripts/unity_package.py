"""Build, verify and install the Unity harbour consumer as a versioned UPM tarball."""

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import tarfile
import tempfile
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "examples/unity-harbour/Package"
NAME = "com.plethu.leitmotif.harbour"
INSTALLED = "leitmotif-harbour.tgz"
# The closure Unity cannot supply; Unity provides the netstandard platform assemblies.
ASSEMBLIES = (
    "Harbour",
    "Microsoft.Bcl.AsyncInterfaces",
    "System.IO.Pipelines",
    "System.Runtime.CompilerServices.Unsafe",
    "System.Text.Encodings.Web",
    "System.Text.Json",
)
OWNED_ASSEMBLIES = {name.lower() for name in (*ASSEMBLIES, "Leitmotif.Harbour")}
REQUIRED = {
    "package.json",
    "README.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "THIRD-PARTY-NOTICES.md",
    "Runtime/HarbourRunner.cs",
    "Runtime/Leitmotif.Harbour.asmdef",
    *(f"Plugins/{name}.dll" for name in ASSEMBLIES),
}
VERSION = re.compile(r"\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?")
MIT = (
    "Permission is hereby granted, free of charge, to any person obtaining a copy of "
    'this software and associated documentation files (the "Software"), to deal in '
    "the Software without restriction, including without limitation the rights to "
    "use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of "
    "the Software, and to permit persons to whom the Software is furnished to do so, "
    "subject to the following conditions:\n\n"
    "The above copyright notice and this permission notice shall be included in all "
    "copies or substantial portions of the Software.\n\n"
    'THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR '
    "IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS "
    "FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR "
    "COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER "
    "IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN "
    "CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\n"
)


def meta(path, is_folder=False):
    """Deterministic Unity .meta so the immutable package imports without churn."""
    guid = hashlib.md5(f"{NAME}/{path}".encode()).hexdigest()
    header = f"fileFormatVersion: 2\nguid: {guid}\n"
    suffix = Path(path).suffix
    importer = {
        ".cs": (
            "MonoImporter:\n  serializedVersion: 2\n  defaultReferences: []\n"
            "  executionOrder: 0\n  icon: {instanceID: 0}\n"
        ),
        ".asmdef": "AssemblyDefinitionImporter:\n  externalObjects: {}\n",
        ".dll": (
            "PluginImporter:\n  externalObjects: {}\n  serializedVersion: 2\n"
            "  iconMap: {}\n  executionOrder: {}\n  defineConstraints: []\n"
            "  isPreloaded: 0\n  isOverridable: 0\n  isExplicitlyReferenced: 0\n"
            "  validateReferences: 1\n  platformData:\n  - first:\n      Any: \n"
            "    second:\n      enabled: 1\n      settings: {}\n"
        ),
    }.get(suffix, "DefaultImporter:\n  externalObjects: {}\n")
    folder = "folderAsset: yes\n" if is_folder else ""
    return (
        header
        + folder
        + importer
        + "  userData: \n  assetBundleName: \n  assetBundleVariant: \n"
    ).encode()


def nuget_packages():
    return Path(os.environ.get("NUGET_PACKAGES") or Path.home() / ".nuget/packages")


def notices(lock, packages):
    """License facts for each shipped NuGet assembly at its locked version."""
    resolved = {
        name.lower(): locked["resolved"]
        for framework in json.loads(lock.read_text()).get("dependencies", {}).values()
        for name, locked in framework.items()
        if "resolved" in locked
    }
    sections, third_party = [], {}
    for name in ASSEMBLIES:
        if name == "Harbour":
            continue
        version = resolved.get(name.lower())
        if version is None:
            raise ValueError(f"{name} is missing from the locked dependency closure")
        directory = packages / name.lower() / version
        spec = ET.parse(directory / f"{name.lower()}.nuspec").getroot()
        namespace = {"n": spec.tag.partition("}")[0].removeprefix("{")}
        metadata = spec.find("n:metadata", namespace)
        license_ = metadata.find("n:license", namespace)
        repository = metadata.find("n:repository", namespace)
        copyright_ = metadata.findtext("n:copyright", "", namespace)
        if license_ is None or license_.text != "MIT" or repository is None:
            raise ValueError(f"{name} {version} is not an MIT package with a source")
        sections.append(
            f"## {name} {version}\n\nLicense: MIT. {copyright_}\n"
            f"Source: {repository.get('url')} at {repository.get('commit')}\n"
        )
        notice = directory / "THIRD-PARTY-NOTICES.TXT"
        if notice.is_file():
            third_party.setdefault(notice.read_text(encoding="utf-8"), []).append(name)
    text = (
        "# Third-party notices\n\nThis package redistributes these NuGet assemblies "
        "unmodified, each under the MIT license below.\n\n" + "\n".join(sections)
    )
    text += f"\n## MIT license\n\n{MIT}"
    for body, owners in third_party.items():
        text += f"\n## Notices shipped with {', '.join(owners)}\n\n{body}\n"
    return text.encode()


def project_version():
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
    return workspace["package"]["version"]


def build(closure, archive, version=None, packages=None):
    """Archive already-built assemblies deterministically; compilation is separate."""
    manifest = json.loads((PACKAGE / "package.json").read_text())
    if version is None:
        version = project_version()
        if manifest["version"] != version:
            raise ValueError(
                f"package.json version {manifest['version']} must match Cargo {version}"
            )
    if not VERSION.fullmatch(version):
        raise ValueError(f"unsupported package version: {version}")
    manifest["version"] = version
    files = {
        "package.json": (json.dumps(manifest, indent=2) + "\n").encode(),
        "README.md": (PACKAGE / "README.md").read_bytes(),
        "LICENSE-MIT": (ROOT / "LICENSE-MIT").read_bytes(),
        "LICENSE-APACHE": (ROOT / "LICENSE-APACHE").read_bytes(),
        "THIRD-PARTY-NOTICES.md": notices(
            PACKAGE / "Dependencies/packages.lock.json",
            packages or nuget_packages(),
        ),
        "Runtime/HarbourRunner.cs": (PACKAGE / "Runtime/HarbourRunner.cs").read_bytes(),
        "Runtime/Leitmotif.Harbour.asmdef": (
            PACKAGE / "Runtime/Leitmotif.Harbour.asmdef"
        ).read_bytes(),
    }
    for name in ASSEMBLIES:
        files[f"Plugins/{name}.dll"] = (closure / f"{name}.dll").read_bytes()
    for name in [*files]:
        files[f"{name}.meta"] = meta(name)
        for parent in Path(name).parents[:-1]:
            files[f"{parent.as_posix()}.meta"] = meta(parent.as_posix(), True)
    archive.parent.mkdir(parents=True, exist_ok=True)
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as output:
        for name, data in sorted(files.items()):
            info = tarfile.TarInfo(f"package/{name}")
            info.size, info.mode, info.mtime = len(data), 0o644, 0
            output.addfile(info, io.BytesIO(data))
    with archive.open("wb") as output:
        with gzip.GzipFile("", "wb", fileobj=output, mtime=0) as compressed:
            compressed.write(buffer.getvalue())


def verify(archive):
    """Return the package manifest after proving the archive is the intended artifact."""
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        if any(
            not member.isfile() or member.name.startswith("/") for member in members
        ):
            raise ValueError("package archive may contain only regular files")
        files = {}
        for member in members:
            path = Path(member.name)
            if path.parts[0] != "package" or ".." in path.parts:
                raise ValueError(f"package entry outside package/: {member.name}")
            files[path.relative_to("package").as_posix()] = source.extractfile(
                member
            ).read()
    visible = {name for name in files if not name.endswith(".meta")}
    missing = sorted(REQUIRED - visible)
    if missing:
        raise ValueError(f"package is missing: {', '.join(missing)}")
    assemblies = {name for name in visible if name.endswith(".dll")}
    if assemblies != {f"Plugins/{name}.dll" for name in ASSEMBLIES}:
        raise ValueError("package assemblies differ from the locked closure")
    unexpected = sorted(visible - REQUIRED)
    if unexpected:
        raise ValueError(f"package contains unshipped files: {', '.join(unexpected)}")
    for name in visible:
        # Immutable packages ignore any asset without its .meta.
        parents = [p.as_posix() for p in Path(name).parents[:-1]]
        if any(f"{item}.meta" not in files for item in [name, *parents]):
            raise ValueError(f"package asset lacks .meta: {name}")
    manifest = json.loads(files["package.json"])
    if manifest.get("name") != NAME or not VERSION.fullmatch(
        manifest.get("version", "")
    ):
        raise ValueError("unsupported package identity")
    return manifest


def conflicts(project):
    """Name game-owned content that would shadow or duplicate a shipped assembly."""
    found = []
    for folder in ("Assets", "Packages"):
        for directory, _, names in os.walk(project / folder):
            for name in names:
                path = Path(directory) / name
                stem = path.stem.lower()
                if path.suffix == ".dll" and stem in OWNED_ASSEMBLIES:
                    found.append(path)
                elif path.suffix == ".asmdef":
                    try:
                        owner = json.loads(path.read_text(encoding="utf-8-sig"))["name"]
                    except ValueError, KeyError:
                        continue
                    if owner.lower() in OWNED_ASSEMBLIES:
                        found.append(path)
    embedded = project / "Packages" / NAME
    if embedded.exists():
        found.append(embedded)
    return sorted(set(found))


def replace(path, data):
    """Swap one file whole, keeping its mode; each file is atomic, the pair is not."""
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as staged:
        try:
            staged.write(data)
            staged.flush()
            if path.exists():
                shutil.copymode(path, staged.name)
            else:
                os.chmod(staged.name, 0o644)
            os.replace(staged.name, path)
        finally:
            Path(staged.name).unlink(missing_ok=True)


def install(archive, project):
    """Install or upgrade only the owned tarball and its manifest reference."""
    verify(archive)
    manifest_path = project / "Packages/manifest.json"
    if not manifest_path.is_file():
        raise ValueError(f"not a Unity project: {project}")
    manifest = json.loads(manifest_path.read_text())
    reference = f"file:{INSTALLED}"
    current = manifest.get("dependencies", {}).get(NAME)
    if current not in (None, reference):
        raise ValueError(f"{NAME} is already installed from {current}; remove it first")
    owned = [path.relative_to(project).as_posix() for path in conflicts(project)]
    if owned:
        raise ValueError(
            "game-owned content conflicts with package assemblies; "
            f"remove or rename: {', '.join(owned)}"
        )
    destination = project / "Packages" / INSTALLED
    if destination.is_symlink():
        raise ValueError("refusing to replace a linked package tarball")
    replace(destination, archive.read_bytes())
    if current is None:
        manifest.setdefault("dependencies", {})[NAME] = reference
        replace(manifest_path, (json.dumps(manifest, indent=2) + "\n").encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    package = commands.add_parser("build", help="Package already-built assemblies.")
    package.add_argument(
        "--closure", type=Path, required=True, help="Built DLL folder."
    )
    package.add_argument("--archive", type=Path, required=True, help="Output .tgz.")
    package.add_argument("--version", help="Override the Cargo workspace version.")
    check = commands.add_parser("verify", help="Check a package archive.")
    check.add_argument("archive", type=Path)
    deploy = commands.add_parser("install", help="Install or upgrade in a project.")
    deploy.add_argument("archive", type=Path)
    deploy.add_argument("project", type=Path, help="Unity project directory.")
    args = parser.parse_args()
    if args.command == "build":
        build(args.closure, args.archive, args.version)
    elif args.command == "verify":
        print(verify(args.archive)["version"])
    else:
        install(args.archive, args.project)


if __name__ == "__main__":
    main()
