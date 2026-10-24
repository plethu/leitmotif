"""Coverage thresholds must reject missing instrumentation and rounded passes."""

import importlib.util
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


if __name__ == "__main__":
    unittest.main()
