"""Coverage thresholds must reject missing instrumentation and rounded passes."""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "coverage_gate", ROOT / "scripts/coverage_gate.py"
)
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


def report(lines=(90, 100), branches=(95, 100)):
    return {
        "data": [
            {
                "totals": {
                    name: {"covered": covered, "count": count}
                    for name, (covered, count) in [
                        ("lines", lines),
                        ("branches", branches),
                    ]
                }
            }
        ]
    }


class CoverageTests(unittest.TestCase):
    def test_exact_thresholds_pass(self):
        self.assertEqual(GATE.failures(report()), [])

    def test_each_threshold_is_independently_required(self):
        for data in [report(lines=(8999, 10000)), report(branches=(9499, 10000))]:
            self.assertEqual(len(GATE.failures(data)), 1)

    def test_missing_or_invalid_measurements_fail(self):
        for data in [
            {},
            {"data": []},
            report(branches=(0, 0)),
            report(lines=(101, 100)),
            report(lines=(-1, 100)),
            report(lines=(90.0, 100)),
            report(lines=(True, 100)),
        ]:
            with self.subTest(data=data):
                self.assertTrue(GATE.failures(data))


class PythonCoverageTests(unittest.TestCase):
    def report(self, lines=90, branches=95):
        return {
            "meta": {"branch_coverage": True},
            "totals": {
                "covered_lines": lines,
                "num_statements": 100,
                "covered_branches": branches,
                "num_branches": 100,
            },
        }

    def test_python_requires_each_raw_count_threshold(self):
        self.assertEqual(GATE.python_failures(self.report()), [])
        for report in [self.report(lines=89), self.report(branches=94)]:
            self.assertEqual(len(GATE.python_failures(report)), 1)

    def test_missing_or_line_only_instrumentation_cannot_pass(self):
        for report in [
            {},
            {"meta": {"branch_coverage": False}},
            {"meta": {"branch_coverage": True}, "totals": {}},
            self.report(lines=True),
        ]:
            with self.subTest(report=report):
                self.assertTrue(GATE.python_failures(report))

    def test_cli_reports_both_formats_and_rejects_shortfalls(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "coverage.json"
            for format_name, data in [("python", self.report()), ("llvm", report())]:
                path.write_text(json.dumps(data), encoding="utf-8")
                result = subprocess.run(
                    [
                        sys.executable,
                        str(ROOT / "scripts/coverage_gate.py"),
                        str(path),
                        "--format",
                        format_name,
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                    env=os.environ,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Coverage passed", result.stdout)
            path.write_text(json.dumps(self.report(branches=94)), encoding="utf-8")
            failed = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts/coverage_gate.py"),
                    str(path),
                    "--format",
                    "python",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(failed.returncode, 1)
            self.assertIn("branches: 94/100", failed.stderr)


if __name__ == "__main__":
    unittest.main()
