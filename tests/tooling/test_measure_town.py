"""Measurement records must describe successful, clean child output."""

import argparse
import json
import sys
import unittest

from scripts.measure_town import measure, positive_integer


class MeasurementTests(unittest.TestCase):
    def test_preserves_child_metrics_and_adds_process_elapsed_time(self):
        record = measure(
            [
                sys.executable,
                "-c",
                'print(\'{"coverage":{"exhausted":true},"peak_rss_bytes":4096}\')',
            ]
        )
        self.assertEqual(record["coverage"], {"exhausted": True})
        self.assertEqual(record["peak_rss_bytes"], 4096)
        self.assertGreater(record["process_seconds"], 0)

    def test_failed_child_does_not_become_a_measurement(self):
        with self.assertRaisesRegex(
            RuntimeError, r"measurement failed \(3\): bad fixture"
        ):
            measure(
                [
                    sys.executable,
                    "-c",
                    'import sys; sys.stderr.write("bad fixture"); sys.exit(3)',
                ]
            )

    def test_successful_child_with_stderr_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "unexpected measurement diagnostics"):
            measure(
                [
                    sys.executable,
                    "-c",
                    'import sys; print("{}"); sys.stderr.write("warning")',
                ]
            )

    def test_extra_stdout_is_rejected_instead_of_losing_diagnostics(self):
        with self.assertRaises(json.JSONDecodeError):
            measure([sys.executable, "-c", 'print("{}\\ndiagnostic")'])

    def test_repetitions_must_be_positive(self):
        self.assertEqual(positive_integer("3"), 3)
        with self.assertRaises(ValueError):
            positive_integer("not-a-number")
        with self.assertRaisesRegex(
            argparse.ArgumentTypeError, "repeat must be positive"
        ):
            positive_integer("0")


if __name__ == "__main__":
    unittest.main()
