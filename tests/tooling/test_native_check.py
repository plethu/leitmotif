"""Native host orchestration requires real artifacts, clean projects and completion."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import native_check


class NativeCheckTests(unittest.TestCase):
    def test_missing_library_is_rejected_before_copy_or_engine_run(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "native library does not exist"):
                native_check.execute(
                    Path(directory) / "missing.so",
                    Path(directory) / "asset",
                    Path(directory),
                )

    def test_missing_asset_is_rejected_before_engine_run(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "fixture.so"
            library.touch()
            with self.assertRaisesRegex(
                ValueError, "compiled dialogue asset does not exist"
            ):
                native_check.execute(library, root / "missing.recitec", root / "out")

    def test_clean_project_has_exact_library_and_descriptor_and_bounded_engine_checks(
        self,
    ):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            consumers = root / "consumers"
            (consumers / "Godot").mkdir(parents=True)
            (consumers / "Godot/Native.tscn").write_text("test scene")
            library = root / "fixture.so"
            library.write_bytes(b"native fixture")
            asset = root / "fixture.recitec"
            asset.write_bytes(b"compiled fixture")
            (root / "cases").mkdir()
            commands = []

            def run(command, **options):
                if command[0] == "dotnet":
                    self.assertEqual(command[2].parent.name, "Godot")
                    self.assertTrue(
                        command[2].parent.parent.parent.joinpath("global.json").exists()
                    )
                    return
                project = command[3]
                self.assertEqual(
                    (project / "native/libleitmotif_godot_native.so").read_bytes(),
                    b"native fixture",
                )
                self.assertEqual(
                    (project / "native/harbour.gdextension").read_bytes(),
                    (native_check.NATIVE / "harbour.gdextension").read_bytes(),
                )
                self.assertTrue(options["normal_godot"])
                commands.append((command, options))

            with (
                patch.object(native_check, "CONSUMERS", consumers),
                patch.object(native_check, "run", side_effect=run),
            ):
                native_check.execute(library, asset, root / "out")
            self.assertEqual(len(commands), 2)
            self.assertIn("--editor", commands[0][0])
            self.assertEqual(commands[1][1]["completion"], native_check.COMPLETION)
            self.assertFalse(commands[0][0][3].exists())

    def test_engine_failure_propagates_and_removes_temporary_project(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "fixture.so"
            library.touch()
            asset = root / "fixture.recitec"
            asset.touch()
            (root / "cases").mkdir()
            consumers = root / "consumers"
            (consumers / "Godot").mkdir(parents=True)
            with (
                patch.object(native_check, "CONSUMERS", consumers),
                patch.object(
                    native_check, "run", side_effect=ValueError("engine failed")
                ),
                self.assertRaisesRegex(ValueError, "engine failed"),
            ):
                native_check.execute(library, asset, root / "out")
            self.assertEqual(list((root / "out").iterdir()), [])

    def test_cli_resolves_artifacts_and_real_help_requires_no_engine(self):
        with (
            patch(
                "sys.argv",
                [
                    "native_check",
                    "--library",
                    "fixture.so",
                    "--asset",
                    "fixture.recitec",
                    "--output",
                    "out",
                ],
            ),
            patch.object(native_check, "execute") as execute,
        ):
            native_check.main()
        self.assertTrue(all(path.is_absolute() for path in execute.call_args.args))
        result = subprocess.run(
            [sys.executable, "-m", "scripts.native_check", "--help"],
            cwd=native_check.ROOT,
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("--library", result.stdout)


if __name__ == "__main__":
    unittest.main()
