"""The Unity package is deterministic, closed over its locked assemblies, and replaceable."""

import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import unity_package as package

NUSPEC = """<package xmlns="http://schemas.microsoft.com/packaging/2013/05/nuspec.xsd">
<metadata><license type="expression">{license}</license>
<copyright>© Example</copyright>{repository}</metadata></package>"""
REPOSITORY = '<repository type="git" url="https://example.test/repo" commit="abc" />'


class Fixture(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.closure = self.root / "closure"
        self.closure.mkdir()
        for name in package.ASSEMBLIES:
            (self.closure / f"{name}.dll").write_bytes(name.encode())
        self.packages = self.root / "nuget"
        lock = json.loads(
            (package.PACKAGE / "Dependencies/packages.lock.json").read_text()
        )
        for framework in lock["dependencies"].values():
            for name, entry in framework.items():
                if "resolved" in entry and name in package.ASSEMBLIES:
                    self.nuspec(name, entry["resolved"])
        self.archive = self.root / "package.tgz"

    def nuspec(self, name, version, license="MIT", repository=REPOSITORY, notice=True):
        directory = self.packages / name.lower() / version
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{name.lower()}.nuspec").write_text(
            NUSPEC.format(license=license, repository=repository), encoding="utf-8"
        )
        if notice:
            (directory / "THIRD-PARTY-NOTICES.TXT").write_text("shared notice")

    def build(self, **options):
        package.build(self.closure, self.archive, packages=self.packages, **options)

    def rewrite(self, edit):
        """Rebuild the archive after changing its member map."""
        with tarfile.open(self.archive, "r:gz") as source:
            files = {m.name: source.extractfile(m).read() for m in source.getmembers()}
        edit(files)
        with tarfile.open(self.archive, "w:gz") as output:
            for name, data in files.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                output.addfile(info, io.BytesIO(data))

    def project(self):
        project = self.root / "game"
        (project / "Assets").mkdir(parents=True)
        (project / "Packages").mkdir()
        (project / "Packages/manifest.json").write_text(
            json.dumps({"dependencies": {"com.example.game": "1.0.0"}})
        )
        return project


class BuildTests(Fixture):
    def test_archive_is_deterministic_and_verifiable(self):
        self.build()
        first = self.archive.read_bytes()
        self.archive.unlink()
        self.build()
        self.assertEqual(first, self.archive.read_bytes())
        self.assertEqual(package.verify(self.archive)["name"], package.NAME)
        with tarfile.open(self.archive) as source:
            names = source.getnames()
        self.assertIn("package/Plugins/System.Text.Json.dll.meta", names)
        self.assertIn("package/Runtime.meta", names)
        self.assertFalse([name for name in names if "Acceptance" in name])

    def test_notices_cover_each_shipped_package_and_license_text(self):
        self.build()
        with tarfile.open(self.archive) as source:
            text = source.extractfile("package/THIRD-PARTY-NOTICES.md").read().decode()
        self.assertIn("## System.Text.Json", text)
        self.assertIn("Permission is hereby granted", text)
        self.assertEqual(text.count("## Notices shipped with"), 1)

    def test_version_must_match_the_workspace_unless_overridden(self):
        with patch.object(package, "project_version", return_value="9.9.9"):
            with self.assertRaisesRegex(ValueError, "must match Cargo"):
                self.build()
        self.build(version="0.0.1-previous")
        self.assertEqual(package.verify(self.archive)["version"], "0.0.1-previous")
        with self.assertRaisesRegex(ValueError, "unsupported package version"):
            self.build(version="latest")

    def test_notices_reject_unlocked_or_unlicensed_packages(self):
        lock = self.root / "lock.json"
        lock.write_text('{"dependencies": {"f": {}}}')
        with self.assertRaisesRegex(ValueError, "missing from the locked"):
            package.notices(lock, self.packages)
        lock.write_text(
            json.dumps(
                {
                    "dependencies": {
                        "f": {
                            n.lower(): {"resolved": "1.0.0"} for n in package.ASSEMBLIES
                        }
                    }
                }
            )
        )
        for name in package.ASSEMBLIES[1:]:
            self.nuspec(name, "1.0.0")
        self.nuspec("System.Text.Json", "1.0.0", license="Proprietary")
        with self.assertRaisesRegex(ValueError, "not an MIT package"):
            package.notices(lock, self.packages)
        self.nuspec("System.Text.Json", "1.0.0", repository="")
        with self.assertRaisesRegex(ValueError, "not an MIT package"):
            package.notices(lock, self.packages)

    def test_default_nuget_cache_honours_the_environment(self):
        with patch.dict("os.environ", {"NUGET_PACKAGES": "/cache"}):
            self.assertEqual(package.nuget_packages(), Path("/cache"))
        with patch.dict("os.environ", {}, clear=True):
            self.assertEqual(package.nuget_packages(), Path.home() / ".nuget/packages")


class VerifyTests(Fixture):
    def reject(self, edit, message):
        self.build()
        self.rewrite(edit)
        with self.assertRaisesRegex(ValueError, message):
            package.verify(self.archive)

    def test_missing_and_unshipped_files_are_rejected(self):
        self.reject(lambda f: f.pop("package/LICENSE-MIT"), "missing: LICENSE-MIT")
        self.reject(
            lambda f: f.update({"package/Harbour.Acceptance.dll": b"x"}),
            "differ from the locked closure",
        )
        self.reject(
            lambda f: f.update(
                {"package/Notes.txt": b"x", "package/Notes.txt.meta": b""}
            ),
            "unshipped files: Notes.txt",
        )

    def test_every_asset_needs_meta_and_identity(self):
        self.reject(lambda f: f.pop("package/Runtime.meta"), "lacks .meta: Runtime/")
        self.reject(lambda f: f.pop("package/README.md.meta"), "lacks .meta: README.md")
        self.reject(
            lambda f: f.update({"package/package.json": b'{"name": "other"}'}),
            "unsupported package identity",
        )

    def test_entries_must_be_regular_files_inside_package(self):
        self.reject(lambda f: f.update({"elsewhere/file": b"x"}), "outside package/")
        self.reject(lambda f: f.update({"package/../escape": b"x"}), "outside package/")
        with tarfile.open(self.archive, "w:gz") as output:
            link = tarfile.TarInfo("package/link")
            link.type = tarfile.SYMTYPE
            output.addfile(link)
        with self.assertRaisesRegex(ValueError, "regular files"):
            package.verify(self.archive)


class InstallTests(Fixture):
    def test_install_adds_the_reference_without_touching_game_content(self):
        self.build()
        project = self.project()
        (project / "Assets/Game.cs").write_text("game")
        package.install(self.archive, project)
        manifest = json.loads((project / "Packages/manifest.json").read_text())
        self.assertEqual(
            manifest["dependencies"][package.NAME], "file:leitmotif-harbour.tgz"
        )
        self.assertEqual(manifest["dependencies"]["com.example.game"], "1.0.0")
        self.assertEqual((project / "Assets/Game.cs").read_text(), "game")
        self.assertEqual(
            (project / "Packages/leitmotif-harbour.tgz").read_bytes(),
            self.archive.read_bytes(),
        )

    def test_installed_files_keep_the_project_manifest_mode(self):
        self.build()
        project = self.project()
        (project / "Packages/manifest.json").chmod(0o664)
        package.install(self.archive, project)
        self.assertEqual(
            (project / "Packages/manifest.json").stat().st_mode & 0o777, 0o664
        )
        self.assertEqual(
            (project / "Packages/leitmotif-harbour.tgz").stat().st_mode & 0o777, 0o644
        )
        self.assertEqual(
            sorted(p.name for p in (project / "Packages").iterdir()),
            ["leitmotif-harbour.tgz", "manifest.json"],
        )

    def test_upgrade_replaces_only_the_owned_tarball(self):
        self.build(version="0.0.1-previous")
        project = self.project()
        package.install(self.archive, project)
        manifest = (project / "Packages/manifest.json").read_text()
        self.build(version="0.0.2")
        package.install(self.archive, project)
        installed = project / "Packages/leitmotif-harbour.tgz"
        self.assertEqual(package.verify(installed)["version"], "0.0.2")
        self.assertEqual((project / "Packages/manifest.json").read_text(), manifest)

    def test_every_conflicting_game_assembly_is_named(self):
        self.build()
        project = self.project()
        plugins = project / "Assets/Plugins"
        plugins.mkdir()
        (plugins / "system.text.json.dll").write_bytes(b"game copy")
        (plugins / "Unrelated.dll").write_bytes(b"fine")
        (plugins / "Mine.asmdef").write_text(json.dumps({"name": "LEITMOTIF.harbour"}))
        (plugins / "Broken.asmdef").write_text("{")
        (project / "Packages" / package.NAME).mkdir()
        with self.assertRaises(ValueError) as caught:
            package.install(self.archive, project)
        message = str(caught.exception)
        for path in ("system.text.json.dll", "Mine.asmdef", f"Packages/{package.NAME}"):
            self.assertIn(path, message)
        self.assertNotIn("Unrelated", message)
        self.assertFalse((project / "Packages/leitmotif-harbour.tgz").exists())

    def test_foreign_installs_projects_and_links_are_refused(self):
        self.build()
        project = self.project()
        manifest = project / "Packages/manifest.json"
        manifest.write_text(json.dumps({"dependencies": {package.NAME: "1.0.0"}}))
        with self.assertRaisesRegex(ValueError, "already installed from 1.0.0"):
            package.install(self.archive, project)
        manifest.write_text("{}")
        (project / "Packages/leitmotif-harbour.tgz").symlink_to(self.root / "target")
        with self.assertRaisesRegex(ValueError, "linked package tarball"):
            package.install(self.archive, project)
        with self.assertRaisesRegex(ValueError, "not a Unity project"):
            package.install(self.archive, self.root / "empty")


class CommandTests(Fixture):
    def run_main(self, *arguments):
        with patch("sys.argv", ["unity_package", *arguments]):
            package.main()

    def test_commands_dispatch(self):
        self.build()
        project = self.project()
        with patch("builtins.print") as shown:
            self.run_main("verify", str(self.archive))
        shown.assert_called_once()
        self.run_main("install", str(self.archive), str(project))
        self.assertTrue((project / "Packages/leitmotif-harbour.tgz").is_file())
        with patch.object(package, "build") as built:
            self.run_main(
                "build", "--closure", "c", "--archive", "a", "--version", "1.2.3"
            )
        built.assert_called_once_with(Path("c"), Path("a"), "1.2.3")


if __name__ == "__main__":
    unittest.main()
