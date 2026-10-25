"""Keep the Bevy metadata exception local and normal duplicate bans effective."""

import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class DependencyPolicyTests(unittest.TestCase):
    def test_bevy_clippy_only_adds_the_two_approved_metadata_exceptions(self):
        root = tomllib.loads((ROOT / "clippy.toml").read_text())
        consumer = tomllib.loads(
            (ROOT / "examples/bevy-harbour/clippy.toml").read_text()
        )
        self.assertCountEqual(
            consumer.pop("allowed-duplicate-crates"), ["syn", "rustc-hash"]
        )
        self.assertEqual(consumer, root)
        self.assertNotIn("allowed-duplicate-crates", root)

    def test_cargo_deny_rejects_real_normal_duplicates_and_accepts_one_version(self):
        with tempfile.TemporaryDirectory(prefix="leitmotif-dependency-policy-") as path:
            fixture = Path(path)
            (fixture / "src").mkdir()
            (fixture / "src/lib.rs").write_text("// Dependency policy fixture.\n")
            manifest = fixture / "Cargo.toml"
            header = (
                '[package]\nname = "duplicate-policy-probe"\nversion = "0.0.0"\n'
                'edition = "2024"\n[workspace]\n[dependencies]\n'
                'current = { package = "rustc-hash", version = "=2.1.3" }\n'
            )
            for duplicate in [True, False]:
                with self.subTest(duplicate=duplicate):
                    manifest.write_text(
                        header
                        + (
                            'legacy = { package = "rustc-hash", version = "=1.1.0" }\n'
                            if duplicate
                            else ""
                        )
                    )
                    subprocess.run(
                        ["cargo", "generate-lockfile", "--offline"],
                        cwd=fixture,
                        check=True,
                        capture_output=True,
                        text=True,
                    )
                    result = subprocess.run(
                        [
                            "cargo",
                            "deny",
                            "--manifest-path",
                            str(manifest),
                            "--config",
                            str(ROOT / "deny.toml"),
                            "--locked",
                            "--offline",
                            "check",
                            "bans",
                        ],
                        capture_output=True,
                        text=True,
                        check=False,
                    )
                    if duplicate:
                        self.assertNotEqual(result.returncode, 0, result.stderr)
                        self.assertIn("error[duplicate]", result.stderr)
                        self.assertIn("rustc-hash", result.stderr)
                    else:
                        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
