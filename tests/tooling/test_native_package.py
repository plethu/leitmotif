"""The shipped archive is deterministic, bounded, verifiable and replaceable."""

import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

from scripts import native_install
from scripts import native_package as package


class PackageTests(unittest.TestCase):
    def setUp(self):
        notices = patch("scripts.native_notices.generate", return_value=b"notices")
        notices.start()
        self.addCleanup(notices.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.library = self.root / "native.so"
        self.library.write_bytes(b"locked native library")
        self.asset = self.root / "harbour.recitec"
        self.asset.write_bytes(b"compiled dialogue")
        (self.root / "cases").mkdir()
        (self.root / "cases/case.recitec").write_bytes(b"case bytes")
        self.archive = self.root / "package.zip"
        self.bundle = self.root / "bundle"
        self.revision = "a" * 40

    def build(self):
        package.build(self.library, self.asset, self.archive, self.revision)

    def unpack(self):
        self.build()
        package.extract(self.archive, self.bundle)

    def rewrite(self, files):
        with zipfile.ZipFile(self.archive, "w") as archive:
            for name, data in files:
                archive.writestr(name, data)

    def test_archive_determinism_and_self_contained_example(self):
        self.build()
        first = self.archive.read_bytes()
        self.library.touch()
        self.build()
        self.assertEqual(first, self.archive.read_bytes())
        package.extract(self.archive, self.bundle)
        self.assertEqual(
            (self.bundle / "extension/libleitmotif_godot_native.so").read_bytes(),
            self.library.read_bytes(),
        )
        self.assertTrue((self.bundle / "example/global.json").is_file())
        self.assertTrue(
            (
                self.bundle / "example/engine-contracts/Godot/packages.lock.json"
            ).is_file()
        )
        driver = self.bundle / "example/engine-contracts/Godot/NativeDriver.cs"
        self.assertFalse(driver.is_symlink())
        self.assertEqual(
            driver.read_bytes(),
            (package.CONSUMERS / "tests/NativeDriver.cs").read_bytes(),
        )
        self.assertFalse(
            any(
                path.name in {"bin", "obj", ".godot"} for path in self.bundle.rglob("*")
            )
        )
        with self.assertRaisesRegex(ValueError, "must not exist"):
            package.extract(self.archive, self.bundle)

    def test_install_replaces_only_owned_files_and_rejects_collisions(self):
        self.unpack()
        project = self.root / "project"
        owned = project / native_install.OWNED
        owned.mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, "unmarked"):
            package.install(self.bundle, project)
        owned.rmdir()
        package.install(self.bundle, project)
        (owned / "obsolete.gd").write_bytes(b"obsolete")
        authored = project / "native/authored.recite"
        authored.write_bytes(b"original authored bytes")
        package.install(self.bundle, project)
        self.assertFalse((owned / "obsolete.gd").exists())
        self.assertEqual(authored.read_bytes(), b"original authored bytes")
        (self.bundle / "extension/LICENSE-MIT").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            package.install(self.bundle, project)
        self.assertEqual(authored.read_bytes(), b"original authored bytes")

    def test_symlink_install_paths_are_rejected(self):
        self.unpack()
        project = self.root / "project"
        project.mkdir()
        (project / "native").symlink_to(self.bundle, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "traverse links"):
            package.install(self.bundle, project)
        (project / "native").unlink()
        (project / "native").mkdir()
        (project / native_install.OWNED).symlink_to(
            self.bundle, target_is_directory=True
        )
        with self.assertRaisesRegex(ValueError, "traverse links"):
            package.install(self.bundle, project)

    def test_extraction_rejects_missing_extra_duplicate_and_changed_artifacts(self):
        self.build()
        with zipfile.ZipFile(self.archive) as archive:
            files = [(name, archive.read(name)) for name in archive.namelist()]
        variants = [
            files[:-1],
            files + [("../escape", b"escape")],
            [
                (name, b"changed" if name.startswith("extension/") else data)
                for name, data in files
            ],
        ]
        for changed in variants:
            with self.subTest(changed=changed[0][0]):
                self.rewrite(changed)
                with self.assertRaises(ValueError):
                    package.extract(self.archive, self.bundle)
                self.assertFalse(self.bundle.exists())

        with self.assertWarns(UserWarning):
            self.rewrite(files + [files[0]])
        with self.assertRaisesRegex(ValueError, "inventory"):
            package.extract(self.archive, self.bundle)
        self.assertFalse(self.bundle.exists())

    def test_source_links_are_bounded_and_generated_trees_excluded(self):
        sources = self.root / "sources"
        sources.mkdir()
        for name in ("bin", "obj", ".godot"):
            (sources / name).mkdir()
            (sources / name / "generated").write_text("generated")
        (sources / "author.cs").write_text("authored")
        (sources / "link.cs").symlink_to(sources / "author.cs")
        self.assertEqual(
            [str(name) for name, _ in package.source_files(sources)],
            ["author.cs", "link.cs"],
        )
        (sources / "escape.cs").symlink_to(self.library)
        with self.assertRaisesRegex(ValueError, "escapes source root"):
            list(package.source_files(sources))

    def test_manifest_rejects_unsupported_identity_and_unsafe_inventory(self):
        self.build()
        with zipfile.ZipFile(self.archive) as archive:
            value = json.loads(archive.read(package.MANIFEST))
        invalid = [
            None,
            [],
            {},
            {**value, "format": 2},
            {**value, "profile": "fast"},
            {**value, "source_revision": 10},
            {**value, "source_revision": "dirty"},
            {**value, "artifacts": []},
            {**value, "artifacts": {}},
        ]
        invalid += [
            {**value, "artifacts": {**value["artifacts"], name: checksum}}
            for name, checksum in [
                ("/absolute", "a" * 64),
                ("extension/../escape", "a" * 64),
                ("extension//alias", "a" * 64),
                ("elsewhere/file", "a" * 64),
                ("extension/file", 12),
                ("extension/file", "bad"),
            ]
        ]
        for data in invalid:
            with self.subTest(value=data), self.assertRaises(ValueError):
                package.Manifest.decode(json.dumps(data))
        manifest = package.Manifest.decode(json.dumps({**value, "profile": "release"}))
        self.assertEqual(json.loads(manifest.encode())["profile"], "release")

    def test_required_native_artifacts_are_validated_before_replacement(self):
        self.unpack()
        project = self.root / "project"
        package.install(self.bundle, project)
        previous = (
            project / native_install.OWNED / "libleitmotif_godot_native.so"
        ).read_bytes()
        original = json.loads((self.bundle / package.MANIFEST).read_bytes())
        for missing in (
            "extension/libleitmotif_godot_native.so",
            "extension/harbour.gdextension",
        ):
            value = {
                **original,
                "artifacts": {
                    name: checksum
                    for name, checksum in original["artifacts"].items()
                    if name != missing
                },
            }
            (self.bundle / package.MANIFEST).write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "unsupported"):
                package.install(self.bundle, project)
            self.assertEqual(
                (
                    project / native_install.OWNED / "libleitmotif_godot_native.so"
                ).read_bytes(),
                previous,
            )

    def test_failed_replacement_restores_previous_owned_install(self):
        self.unpack()
        project = self.root / "project"
        package.install(self.bundle, project)
        previous = project / native_install.OWNED / "previous-owned.txt"
        previous.write_bytes(b"old working installation")
        rename = Path.rename

        def fail_stage(path, destination):
            if path.name == "extension" and path.parent.name.startswith(
                "native-install-"
            ):
                raise OSError("injected rename failure")
            return rename(path, destination)

        with (
            patch.object(Path, "rename", fail_stage),
            self.assertRaisesRegex(OSError, "injected"),
        ):
            package.install(self.bundle, project)
        self.assertEqual(previous.read_bytes(), b"old working installation")
        self.assertEqual(
            list((project / "native").iterdir()), [project / native_install.OWNED]
        )

    def test_fixed_package_sources_are_each_read_once(self):
        reads = []
        original = Path.read_bytes

        def counted(path):
            reads.append(path)
            return original(path)

        consumers = self.root / "fixed-consumers"
        consumers.mkdir()
        (consumers / "project.godot").write_bytes(b"fixed project")
        (consumers / "Host.cs").write_bytes(b"fixed host")
        with (
            patch.object(Path, "read_bytes", counted),
            patch.object(package, "CONSUMERS", consumers),
        ):
            self.build()
        self.assertEqual(reads.count(self.library), 1)
        self.assertEqual(reads.count(self.asset), 1)
        self.assertEqual(len(reads), 12)
        with zipfile.ZipFile(self.archive) as archive:
            self.assertEqual(
                archive.read("example/engine-contracts/Host.cs"), b"fixed host"
            )
        self.assertEqual(len(reads), len(set(reads)))

    def test_cli_routes_build_extract_and_install(self):
        for command, args, target in [
            (
                "build",
                [
                    "--library",
                    str(self.library),
                    "--asset",
                    str(self.asset),
                    "--archive",
                    str(self.archive),
                ],
                "build",
            ),
            ("extract", [str(self.archive), str(self.bundle)], "extract"),
            ("install", [str(self.bundle), str(self.root / "project")], "install"),
        ]:
            with (
                patch("sys.argv", ["native_package", command, *args]),
                patch.object(package, target) as called,
            ):
                package.main()
                called.assert_called_once()
            if command != "build":
                with (
                    patch("sys.argv", ["native_install", command, *args]),
                    patch.object(native_install, target) as called,
                ):
                    native_install.main()
                    called.assert_called_once()


if __name__ == "__main__":
    unittest.main()
