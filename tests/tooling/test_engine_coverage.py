"""Coverage must account for every authored file without hiding Node branches."""

import json
import tempfile
import unittest
from pathlib import Path

from scripts.engine_coverage import check, counts


def report(path, line_hits=1, branch_hits=1):
    return {
        "host.dll": {
            str(path): {
                "Node": {
                    "Ready()": {
                        "Lines": {"7": line_hits},
                        "Branches": [
                            {
                                "Offset": 1,
                                "EndOffset": 2,
                                "Path": 0,
                                "Ordinal": 0,
                                "Hits": branch_hits,
                            }
                        ],
                    }
                }
            }
        }
    }


class AuthoredCoverageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.node = self.root / "Godot/Node.cs"
        self.generated = self.root / "Godot/obj"
        self.tests = self.root / "tests"

    def measure(self, reports, expected=None):
        return counts(
            reports,
            [self.node] if expected is None else expected,
            [self.generated],
            self.tests,
        )

    def test_exact_source_identity_unions_hits_across_hosts(self):
        self.assertEqual(
            self.measure([report(self.node, 0, 0), report(self.node)]), ((1, 1), (1, 1))
        )

    def test_same_basename_does_not_substitute_for_missing_node(self):
        with self.assertRaisesRegex(ValueError, "unknown coverage source"):
            self.measure([report(self.root / "elsewhere/Node.cs")])

    def test_missing_file_or_instrumentation_fails(self):
        with self.assertRaisesRegex(ValueError, "missing authored"):
            self.measure([])
        empty = {"host.dll": {str(self.node): {}}}
        with self.assertRaisesRegex(ValueError, "no sequence points"):
            self.measure([empty])

    def test_only_identified_generated_outputs_and_test_sources_are_omitted(self):
        self.assertEqual(
            self.measure(
                [
                    report(self.node),
                    report(self.generated / "Node.generated.cs"),
                    report(self.tests / "Driver.cs"),
                ]
            ),
            ((1, 1), (1, 1)),
        )
        for path in [self.root / "Node.generated.cs", self.generated / "Authored.cs"]:
            with (
                self.subTest(path=path),
                self.assertRaisesRegex(ValueError, "unknown coverage"),
            ):
                self.measure([report(self.node), report(path)])

    def test_symlink_to_test_driver_uses_canonical_source(self):
        self.tests.mkdir()
        driver = self.tests / "Driver.cs"
        driver.touch()
        alias = self.root / "Driver.cs"
        alias.symlink_to(driver)
        self.assertEqual(
            self.measure([report(self.node), report(alias)]), ((1, 1), (1, 1))
        )

    def test_bad_hit_counts_are_rejected(self):
        for hits in [-1, True, "1"]:
            with (
                self.subTest(hits=hits),
                self.assertRaisesRegex(ValueError, "invalid coverage"),
            ):
                self.measure([report(self.node, line_hits=hits)])

    def test_shared_thresholds_use_raw_counts(self):
        path = self.root / "coverage.json"
        path.write_text(json.dumps(report(self.node)), encoding="utf-8")
        self.assertIn(
            "lines 1/1", check([path], [self.node], [self.generated], self.tests)
        )
        path.write_text(json.dumps(report(self.node, branch_hits=0)), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "branches: 0/1"):
            check([path], [self.node], [self.generated], self.tests)


if __name__ == "__main__":
    unittest.main()
