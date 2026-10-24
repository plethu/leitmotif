"""Measurement records must describe successful, clean child output."""

import argparse
import contextlib
import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.measure_town import (
    build_example,
    command_output,
    main,
    measure,
    positive_integer,
)


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


class MeasurementCommandTests(unittest.TestCase):
    def test_build_selects_the_actual_example_artifact(self):

        artifacts = [
            {"reason": "compiler-message"},
            {"reason": "compiler-artifact", "target": {"name": "library"}},
            {"reason": "compiler-artifact", "target": {"name": "town_scaling"}},
            {
                "reason": "compiler-artifact",
                "target": {"name": "town_scaling"},
                "executable": "/fixture/town",
            },
        ]
        with patch(
            "scripts.measure_town.command_output",
            return_value="\n".join(json.dumps(value) for value in artifacts),
        ) as command:
            self.assertEqual(build_example(Path.cwd()), Path("/fixture/town"))
            self.assertIn("--release", command.call_args.args[0])
        with patch("scripts.measure_town.command_output", return_value=""):
            with self.assertRaisesRegex(RuntimeError, "did not report"):
                build_example(Path.cwd())

    def test_identification_command_captures_real_child_output(self):

        self.assertEqual(
            command_output(
                [sys.executable, "-c", 'print("  tool version  ")'], Path.cwd()
            ),
            "tool version",
        )

    def test_measurement_cli_preserves_metadata_and_repeats_fixed_inputs(self):

        for cpuinfo, expected in [
            ("unrelated\nmodel name : Fixture CPU\n", "Fixture CPU"),
            ("unrelated\n", "unknown"),
        ]:
            output = io.StringIO()
            with (
                patch.object(
                    sys, "argv", ["measure-town", "--repeat", "2", "--threads", "1"]
                ),
                patch.object(sys, "platform", "linux"),
                patch(
                    "scripts.measure_town.build_example",
                    return_value=Path("/fixture/town"),
                ),
                patch(
                    "scripts.measure_town.command_output", return_value="tool identity"
                ),
                patch(
                    "scripts.measure_town.measure",
                    return_value={"coverage": {"exhausted": True}},
                ) as measure_child,
                patch.object(Path, "read_text", return_value=cpuinfo),
                contextlib.redirect_stdout(output),
            ):
                self.assertEqual(main(), 0)
            records = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual([record["repetition"] for record in records], [1, 2])
            self.assertTrue(all(record["coverage"]["exhausted"] for record in records))
            self.assertEqual(records[0]["machine"]["processor"], expected)
            self.assertEqual(records[0]["toolchain"]["cargo"], "tool identity")
            self.assertEqual(measure_child.call_count, 2)
            self.assertEqual(
                measure_child.call_args.args[0], ["/fixture/town", "--threads", "1"]
            )

    def test_cli_rejects_unsupported_platform_and_failed_build(self):

        for platform, expected in [("win32", 2), ("linux", 1)]:
            stderr = io.StringIO()
            with (
                patch.object(sys, "argv", ["measure-town"]),
                patch.object(sys, "platform", platform),
                patch(
                    "scripts.measure_town.build_example",
                    side_effect=RuntimeError("fixture build failed"),
                ),
                contextlib.redirect_stderr(stderr),
            ):
                with self.assertRaises(SystemExit) as result:
                    main()
            self.assertEqual(result.exception.code, expected)
            self.assertIn(
                "require Linux" if expected == 2 else "fixture build failed",
                stderr.getvalue(),
            )


if __name__ == "__main__":
    unittest.main()
