"""The engine gate preserves process failures, semantic statuses, and source scope."""

import json
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import engine_check


class EngineCheckTests(unittest.TestCase):
    def test_process_timeout_status_and_output_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "run.log"
            with patch.object(
                subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 2, "inconclusive\n", ""),
            ) as call:
                self.assertEqual(
                    engine_check.run(["engine", "request"], 2, output=output),
                    "inconclusive\n",
                )
                self.assertEqual(output.read_text(), "inconclusive\n")
                self.assertEqual(call.call_args.kwargs["timeout"], 120)
                self.assertTrue(call.call_args.kwargs["capture_output"])
            with (
                patch.object(
                    subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 3, "failed\n", ""),
                ),
                self.assertRaisesRegex(ValueError, "exited 3, expected 0"),
            ):
                engine_check.run(["engine"])

    def test_authored_scope_includes_console_and_node_but_not_test_symlink(self):
        files = "\n".join(
            [
                "examples/engine-contracts/Console/Program.cs",
                "examples/engine-contracts/Godot/HarbourNode.cs",
                "examples/engine-contracts/Godot/CoverageDriver.cs",
                "examples/engine-contracts/tests/AcceptanceCases.cs",
                "examples/engine-contracts/Console/Harbour.Console.csproj",
            ]
        )
        with patch.object(subprocess, "check_output", return_value=files):
            paths = engine_check.authored_sources()
        self.assertEqual(
            paths,
            {
                (engine_check.CONSUMERS / "Console/Program.cs").resolve(),
                (engine_check.CONSUMERS / "Godot/HarbourNode.cs").resolve(),
            },
        )

    def test_orchestration_requires_every_normal_status_and_both_coverage_reports(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            commands = []
            options = []

            def tool(command, expected=0, *, output=None, **kwargs):
                commands.append((command, expected))
                options.append(kwargs)
                if output is not None:
                    output.write_text(json.dumps({"conformance": {"cases": [{}] * 30}}))

            with (
                patch.object(engine_check, "run", side_effect=tool),
                patch.object(subprocess, "check_output", return_value="revision\n"),
                patch.object(engine_check.shutil, "which", return_value="godot"),
                patch.object(
                    engine_check, "authored_sources", return_value={Path("Node.cs")}
                ),
                patch.object(
                    engine_check, "check", return_value="coverage passed"
                ) as coverage,
            ):
                engine_check.execute(Path("rust-fixture"), output)
            normal = [
                (command, status)
                for command, status in commands
                if "--" in command and "res://Coverage.tscn" not in command
            ]
            self.assertEqual([status for _, status in normal], [0, 0, 1, 2])
            self.assertEqual(
                coverage.call_args.args[0],
                [output / "console-coverage.json", output / "godot-coverage.json"],
            )
            instrumented = [
                command for command, _ in commands if command[0] == "coverlet"
            ]
            self.assertEqual(len(instrumented), 2)
            self.assertTrue(
                all("--include-test-assembly" in command for command in instrumented)
            )

            godot_coverage = instrumented[-1]
            self.assertEqual(
                godot_coverage[godot_coverage.index("--target") + 1], "env"
            )
            target_args = godot_coverage[godot_coverage.index("--targetargs") + 1]
            self.assertIn("HARBOUR_COVERAGE=1", target_args)
            self.assertTrue(options[-1]["normal_godot"])
            negative = [
                (command, status)
                for command, status in commands
                if any(str(part).startswith("HARBOUR_TEST_MODE=") for part in command)
            ]
            self.assertEqual([status for _, status in negative], [3, 3])

    def test_fixed_cost_regression_rejects_incomplete_fixture(self):
        with tempfile.TemporaryDirectory() as directory:

            def tool(command, expected=0, *, output=None, **kwargs):
                if output is not None:
                    output.write_text('{"conformance":{"cases":[]}}')

            with (
                patch.object(engine_check, "run", side_effect=tool),
                patch.object(subprocess, "check_output", return_value="revision"),
                self.assertRaisesRegex(ValueError, "exactly 30"),
            ):
                engine_check.execute(Path("fixture"), Path(directory))

    def test_normal_engine_errors_and_missing_completion_are_not_success(self):
        for stderr, stdout, options, message in [
            (
                "ERROR: lifecycle failed",
                "",
                {"normal_godot": True},
                "unexpected Godot error",
            ),
            (
                "",
                "ERROR: child lifecycle failed",
                {"normal_godot": True},
                "unexpected Godot error",
            ),
            (
                "",
                "engine banner only",
                {"completion": "Harbour conformance:"},
                "missing host conformance",
            ),
        ]:
            with (
                self.subTest(message=message),
                patch.object(
                    subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 0, stdout, stderr),
                ),
                self.assertRaisesRegex(ValueError, message),
            ):
                engine_check.run(["godot"], **options)

    def test_only_the_known_startup_noise_is_tolerated(self):
        noise = engine_check.GODOT_STARTUP_NOISE[0]
        other = noise.replace("android_sdk_path", "java_sdk_path")
        for stderr, passes in [
            (f"{noise}\n   at: _EDITOR_GET (editor_settings.cpp:1531)\n", True),
            (f"{noise}\nERROR: lifecycle failed\n", False),
            (f"{other}\n", False),
        ]:
            with (
                self.subTest(stderr=stderr),
                patch.object(
                    subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 0, "", stderr),
                ),
            ):
                if passes:
                    engine_check.run(["godot"], normal_godot=True)
                else:
                    with self.assertRaisesRegex(ValueError, "unexpected Godot error"):
                        engine_check.run(["godot"], normal_godot=True)

    def test_sdk_selector_matches_mise_and_disables_roll_forward(self):
        mise = tomllib.loads((engine_check.ROOT / ".mise.toml").read_text())
        sdk = json.loads((engine_check.ROOT / "global.json").read_text())["sdk"]
        self.assertEqual(sdk["version"], mise["tools"]["dotnet"])
        self.assertEqual(sdk["rollForward"], "disable")

    def test_dotnet_tool_installs_serialize_first_use(self):
        mise = tomllib.loads((engine_check.ROOT / ".mise.toml").read_text())
        tools = mise["tools"]
        self.assertEqual(
            tools["dotnet:csharpier"]["depends"], ["dotnet:coverlet.console"]
        )
        self.assertNotIn("jobs", mise["settings"])

    def test_native_leaf_preserves_workspace_lints_except_scoped_unsafe_entry(self):
        root = tomllib.loads((engine_check.ROOT / "Cargo.toml").read_text())
        native = tomllib.loads(
            (engine_check.ROOT / "examples/godot-native/Cargo.toml").read_text()
        )
        self.assertIn("examples/godot-native", root["workspace"]["exclude"])
        self.assertEqual(root["workspace"]["lints"]["rust"]["unsafe_code"], "forbid")
        expected = root["workspace"]["lints"]
        expected["rust"]["unsafe_code"] = "deny"
        self.assertEqual(native["lints"], expected)

    def test_real_help_is_available_without_engine_tools(self):
        result = subprocess.run(
            [sys.executable, "-m", "scripts.engine_check", "--help"],
            cwd=engine_check.ROOT,
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("--binary", result.stdout)

    def test_cli_resolves_bounded_output_paths(self):
        with (
            patch(
                "sys.argv", ["engine_check", "--binary", "fixture", "--output", "out"]
            ),
            patch.object(engine_check, "execute") as execute,
        ):
            engine_check.main()
        self.assertTrue(all(path.is_absolute() for path in execute.call_args.args))


if __name__ == "__main__":
    unittest.main()
