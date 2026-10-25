"""Limits and process failures must never masquerade as exhaustive reachability."""

import argparse
import hashlib
import json
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import measure_symbolic


class SymbolicMeasurementTests(unittest.TestCase):
    def completed(self, payload, code=0, stderr=""):
        return subprocess.CompletedProcess([], code, json.dumps(payload), stderr)

    def measure(self, child):
        with patch.object(subprocess, "run", return_value=child):
            return measure_symbolic.measure_child(["fixture"], 1, 256)

    def test_real_fresh_worker_reports_a_process_failure_without_a_conclusion(self):
        # The Python executable is real; a deliberately unknown option fails before code runs.
        child = subprocess.run(
            [
                sys.executable,
                "-m",
                "scripts.measure_symbolic",
                "--worker",
                "--binary",
                sys.executable,
                "--invalid-python-option",
            ],
            check=True,
            text=True,
            capture_output=True,
            timeout=10,
        )
        result = json.loads(child.stdout)
        self.assertEqual(result["status"], "process_failure")
        self.assertGreater(result["peak_child_rss_bytes"], 0)
        self.assertGreaterEqual(result["process_seconds"], 0)
        self.assertNotIn("measurement", result)

    def test_inherited_hard_limit_returns_setup_failure_for_every_repetition(self):
        # Leave enough address space for rustc metadata while exceeding the inherited cap.
        program = (
            "import resource,sys; "
            "from scripts.measure_symbolic import main; "
            "resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824)); "
            "sys.argv=['measure_symbolic','--binary',sys.executable,'--repeat','2',"
            "'--memory-mib','2048','--invalid-python-option']; "
            "raise SystemExit(main())"
        )
        child = subprocess.run(
            [sys.executable, "-c", program],
            check=True,
            text=True,
            capture_output=True,
            timeout=10,
        )
        records = [json.loads(line) for line in child.stdout.splitlines()]
        self.assertEqual(len(records), 2)
        for result in records:
            self.assertEqual(result["status"], "setup_failure")
            self.assertEqual(result["memory_mib"], 2048)
            self.assertIn("preexec_fn", result["diagnostic"])
            self.assertNotIn("measurement", result)
            self.assertNotIn("returncode", result)
        self.assertEqual(child.stderr, "")

    def test_real_child_observes_cpu_and_address_space_limits(self):
        program = (
            "import json,resource; "
            "print(json.dumps({'result':{'conclusion':'inconclusive','exhausted':False},"
            "'cpu':resource.getrlimit(resource.RLIMIT_CPU),"
            "'memory':resource.getrlimit(resource.RLIMIT_AS)}))"
        )
        child = subprocess.run(
            [
                sys.executable,
                "-m",
                "scripts.measure_symbolic",
                "--worker",
                "--binary",
                sys.executable,
                "--timeout",
                "2",
                "--memory-mib",
                "256",
                "-c",
                program,
            ],
            check=True,
            text=True,
            capture_output=True,
            timeout=10,
        )
        result = json.loads(child.stdout)
        self.assertEqual(result["status"], "inconclusive")
        self.assertEqual(result["measurement"]["cpu"], [2, 3])
        self.assertEqual(result["measurement"]["memory"], [256 * 1024 * 1024] * 2)

    def test_conclusions_require_coherent_exhaustion(self):
        for conclusion in ["reached", "unreachable", "inconclusive"]:
            with self.subTest(conclusion=conclusion):
                result = self.measure(
                    self.completed(
                        {
                            "result": {
                                "conclusion": conclusion,
                                "exhausted": conclusion == "unreachable",
                            }
                        }
                    )
                )
                self.assertEqual(result["status"], conclusion)
        explicit = {
            "result": {"conclusion": "unreachable", "coverage": {"exhausted": True}}
        }
        self.assertEqual(
            self.measure(self.completed(explicit))["status"], "unreachable"
        )
        for data in [
            {},
            {"result": {"conclusion": "unreachable"}},
            {"result": {"conclusion": "unknown"}},
            {"result": {"conclusion": "unreachable", "coverage": None}},
            None,
        ]:
            with self.subTest(data=data):
                self.assertEqual(
                    self.measure(self.completed(data))["status"], "invalid_output"
                )

    def test_signals_nonzero_and_diagnostics_remain_distinct(self):
        for code, stderr, expected in [
            (-9, "", "signal"),
            (2, "allocation failed", "process_failure"),
            (0, "warning", "unexpected_diagnostics"),
        ]:
            with self.subTest(code=code, stderr=stderr):
                self.assertEqual(
                    self.measure(self.completed({}, code, stderr))["status"], expected
                )

    def test_timeouts_and_spawn_errors_are_not_proofs(self):
        for error, expected in [
            (
                subprocess.TimeoutExpired(["fixture"], 1, b"partial", b"killed"),
                "timeout",
            ),
            (FileNotFoundError("absent"), "spawn_error"),
        ]:
            with (
                self.subTest(error=error),
                patch.object(subprocess, "run", side_effect=error),
            ):
                result = measure_symbolic.measure_child(["fixture"], 1, 256)
                self.assertEqual(result["status"], expected)
                self.assertNotIn("measurement", result)
        self.assertEqual(measure_symbolic.text("text"), "text")

    def test_backend_stack_default_and_explicit_override_are_recorded(self):
        with patch.dict(measure_symbolic.os.environ, {}, clear=True):
            self.assertEqual(
                measure_symbolic.backend_environment(), {"OXIDD_STACK_SIZE": "8388608"}
            )
        with patch.dict(measure_symbolic.os.environ, {"OXIDD_STACK_SIZE": "16777216"}):
            self.assertEqual(
                measure_symbolic.backend_environment(), {"OXIDD_STACK_SIZE": "16777216"}
            )

    def test_resource_limits_bound_cpu_and_address_space(self):
        with patch.object(measure_symbolic.resource, "setrlimit") as set_limit:
            measure_symbolic.limits(2, 256)
        self.assertEqual(set_limit.call_args_list[0].args[1], (2, 3))
        self.assertEqual(set_limit.call_args_list[1].args[1], (256 * 1024 * 1024,) * 2)
        with self.assertRaises(argparse.ArgumentTypeError):
            measure_symbolic.bounded_integer("0")
        self.assertEqual(measure_symbolic.bounded_integer("3"), 3)

    def test_main_preserves_fixture_arguments_and_separate_worker_per_repetition(self):
        replies = [
            subprocess.CompletedProcess([], 0, "", ""),
            subprocess.CompletedProcess([], 0, "rustc version", ""),
            subprocess.CompletedProcess([], 0, "revision", ""),
            self.completed({"status": "timeout"}),
            self.completed({"status": "signal"}),
        ]
        with (
            patch(
                "sys.argv",
                [
                    "measure_symbolic",
                    "--binary",
                    sys.executable,
                    "--repeat",
                    "2",
                    "--backend",
                    "oxidd",
                ],
            ),
            patch.object(subprocess, "run", side_effect=replies) as run,
            patch("builtins.print") as output,
        ):
            self.assertEqual(measure_symbolic.main(), 0)
        self.assertEqual(output.call_count, 2)
        root = Path(measure_symbolic.__file__).resolve().parents[1]
        experiment_lock = root / "examples/symbolic-town/Cargo.lock"
        expected_hash = hashlib.sha256(experiment_lock.read_bytes()).hexdigest()
        core_hash = hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest()
        self.assertNotEqual(expected_hash, core_hash)
        for call in output.call_args_list:
            self.assertEqual(json.loads(call.args[0])["lock_sha256"], expected_hash)
        root = Path(measure_symbolic.__file__).resolve().parents[1]
        for call in run.call_args_list:
            if call.args[0][0] == "git":
                self.assertEqual(call.kwargs.get("cwd"), root)
        self.assertTrue(
            all("--worker" in call.args[0] for call in run.call_args_list[3:])
        )
        self.assertTrue(
            all(
                call.args[0][-2:] == ["--backend", "oxidd"]
                for call in run.call_args_list[3:]
            )
        )

    def test_unsupported_platform_rejects_before_running_a_backend(self):
        with (
            patch("sys.argv", ["measure_symbolic", "--binary", "fixture"]),
            patch.object(sys, "platform", "win32"),
            self.assertRaises(SystemExit),
        ):
            measure_symbolic.main()
