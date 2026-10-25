"""Dependency notices retain source and license data without local machine paths."""

import json
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import native_notices


class NoticeTests(unittest.TestCase):
    def test_generator_uses_locked_graph_and_existing_license_policy(self):
        report = {
            "licenses": [
                {"id": "MIT", "name": "MIT", "text": "Copyright author", "used_by": []}
            ]
        }

        def generate(command, **options):
            self.assertIn("--locked", command)
            self.assertIn("--fail", command)
            self.assertEqual(options["timeout"], 120)
            self.assertNotIn("RUSTFLAGS", options["env"])
            self.assertNotIn("CARGO_ENCODED_RUSTFLAGS", options["env"])
            config = Path(command[-1]).read_text()
            self.assertIn('"MIT"', config)
            self.assertIn("[godot-core]", config)
            self.assertIn('"MPL-2.0"', config)
            return json.dumps(report)

        with (
            patch.object(
                native_notices.subprocess, "check_output", side_effect=generate
            ),
            patch.dict(
                "os.environ",
                {
                    "RUSTFLAGS": "-Z coverage-options=branch",
                    "CARGO_ENCODED_RUSTFLAGS": "nightly-only",
                },
            ),
        ):
            output = json.loads(native_notices.generate("a" * 40))
        self.assertEqual(output["licenses"][0]["text"], "Copyright author")

    def test_portable_notices_keep_exact_registry_and_git_source_urls(self):
        crates = [
            {
                "name": "godot",
                "version": "0.5.5",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "repository": "https://github.com/godot-rust/gdext",
                "license": "MPL-2.0",
            },
            {
                "name": "recite-core",
                "version": "0.1.0",
                "source": "git+https://github.com/plethu/recite?rev=abc#abc",
                "repository": "https://github.com/plethu/recite",
                "license": "MIT OR Apache-2.0",
            },
        ]
        crates.append(
            {
                "name": "leitmotif",
                "version": "0.1.0",
                "source": None,
                "repository": "https://github.com/plethu/leitmotif",
                "license": "MIT OR Apache-2.0",
            }
        )
        report = {
            "licenses": [
                {
                    "id": "MPL-2.0",
                    "name": "Mozilla",
                    "text": "actual complete license text",
                    "source_path": "/machine/local/license",
                    "used_by": [
                        {"crate": crate, "path": "/machine/local/crate"}
                        for crate in crates
                    ],
                }
            ]
        }
        first = native_notices.portable(report, "a" * 40)
        report["licenses"][0]["used_by"].reverse()
        self.assertEqual(first, native_notices.portable(report, "a" * 40))
        self.assertNotIn(b"/machine/local", first)
        data = json.loads(first)["licenses"][0]
        self.assertEqual(data["text"], "actual complete license text")
        self.assertEqual(
            data["used_by"][0]["source_archive"],
            "https://crates.io/api/v1/crates/godot/0.5.5/download",
        )
        self.assertEqual(
            data["used_by"][2]["source_archive"],
            "https://github.com/plethu/recite/archive/abc.tar.gz",
        )
        with self.assertRaisesRegex(ValueError, "no native dependency notices"):
            native_notices.portable({"licenses": []}, "a" * 40)
