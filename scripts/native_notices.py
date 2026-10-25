"""Generate portable native dependency notices with pinned cargo-about."""

import json
import os
import subprocess
import tempfile
import tomllib
from pathlib import Path

from scripts.engine_check import ROOT


def generate(revision):
    """Use the existing deny policy for license choice, retaining exact source identities."""
    policy = tomllib.loads((ROOT / "deny.toml").read_text())["licenses"]
    config = [
        "accepted = " + json.dumps(policy["allow"]),
        'targets = ["x86_64-unknown-linux-gnu"]',
        "ignore-dev-dependencies = true",
    ]
    for exception in policy["exceptions"]:
        config.extend(
            [
                f"[{exception['name']}]",
                "accepted = " + json.dumps(exception["allow"]),
            ]
        )
    with tempfile.TemporaryDirectory(prefix="native-notices-") as temporary:
        path = Path(temporary) / "about.toml"
        path.write_text("\n".join(config), encoding="utf-8")
        # License collection queries metadata; inherited coverage flags may require
        # a different compiler and must not affect this non-compiling operation.
        environment = {
            key: value
            for key, value in os.environ.items()
            if key not in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}
        }
        output = subprocess.check_output(
            [
                "cargo",
                "about",
                "generate",
                "--manifest-path",
                str(ROOT / "examples/godot-native/Cargo.toml"),
                "--locked",
                "--fail",
                "--format",
                "json",
                "--config",
                str(path),
            ],
            cwd=ROOT,
            text=True,
            timeout=120,
            env=environment,
        )
    return portable(json.loads(output), revision)


def portable(report, revision):
    """Keep license texts and source coordinates while discarding machine-local paths."""
    licenses = []
    for license_record in report["licenses"]:
        crates = []
        for used in license_record["used_by"]:
            crate = used["crate"]
            identity = {
                key: crate[key]
                for key in (
                    "name",
                    "version",
                    "source",
                    "repository",
                    "license",
                )
            }
            if crate["source"] is None:
                identity["source"] = (
                    f"git+https://github.com/plethu/leitmotif#{revision}"
                )
                identity["source_archive"] = (
                    f"https://github.com/plethu/leitmotif/archive/{revision}.tar.gz"
                )
            elif crate["source"].startswith("registry+"):
                identity["source_archive"] = (
                    f"https://crates.io/api/v1/crates/{crate['name']}/{crate['version']}/download"
                )
            else:
                repository, git_revision = (
                    crate["source"].removeprefix("git+").split("#")
                )
                identity["source_archive"] = (
                    repository.split("?")[0] + f"/archive/{git_revision}.tar.gz"
                )
            crates.append(identity)
        licenses.append(
            {
                "id": license_record["id"],
                "name": license_record["name"],
                "text": license_record["text"],
                "used_by": sorted(
                    crates, key=lambda crate: (crate["name"], crate["version"])
                ),
            }
        )
    if not licenses:
        raise ValueError("cargo-about produced no native dependency notices")
    return (
        json.dumps(
            {
                "generator": "cargo-about 0.9.2",
                "licenses": sorted(
                    licenses, key=lambda record: (record["id"], record["text"])
                ),
            },
            sort_keys=True,
            indent=2,
        )
        + "\n"
    ).encode()
