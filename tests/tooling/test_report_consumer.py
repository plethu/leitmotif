"""Serialized reports are data, not workflow or Markdown instructions."""

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.report_consumer import command_text, render, validate_locations
from scripts.report_view import ReportError, parse_report


def report_data():
    runs = []
    for number in range(8):
        runs.append(
            {
                "model_id": "sha256:fixture",
                "property": "opening requires a key",
                "protocol": "finite fixture protocol",
                "recite_revision": "fixture-revision",
                "initial": {
                    "values": {"case": {"type": "symbol", "value": str(number)}}
                },
                "limits": {"max_nodes": 10, "max_depth": 5},
                "coverage": {
                    "evaluated_nodes": 3,
                    "generated_nodes": 3,
                    "peak_frontier": 2,
                    "completed_paths": 1,
                },
                "conclusion": "exhaustive",
                "witness": [],
            }
        )
    return {
        "schema_version": "leitmotif.archive-ci.experimental.v1",
        "tool_version": "test",
        "virtual_source": {"path": "archive.recite", "content": ":: archive\n  text\n"},
        "kind": "analysed",
        "runs": runs,
    }


def step(kind="virtual", path="archive.recite", line=1):
    return {
        "action": "next",
        "outcome": "captured prompt",
        "source": {"kind": kind, "path": path, "line": line, "column": 1},
        "declaration": None,
        "host": {"values": {"key": {"type": "bool", "value": False}}},
    }


class ReportConsumerTests(unittest.TestCase):
    def test_incompatible_version_is_rejected_before_rendering(self):
        with self.assertRaisesRegex(ReportError, "schema"):
            parse_report({"schema_version": "future.v99"})

    def test_additive_fields_do_not_change_consumed_meaning(self):
        data = report_data()
        before = parse_report(data)
        data["new_diagnostic"] = {"future": True}
        data["runs"][0]["coverage"]["future_counter"] = 4
        self.assertEqual(before, parse_report(data))
        summary, commands, status = render(before)
        self.assertEqual(status, 0)
        self.assertEqual(commands, [])
        self.assertIn("3 evaluated / 3 generated", summary)
        self.assertIn("Limits: 10 nodes, 5 actions deep", summary)
        self.assertIn("finite fixture protocol", summary)

    def test_unknown_or_malformed_required_data_is_never_a_pass(self):
        base = report_data()
        mutations = [
            lambda data: data.pop("tool_version"),
            lambda data: data.update(kind="future_result"),
            lambda data: data.update(runs=[]),
            lambda data: data["runs"].pop(),
            lambda data: data["runs"].__setitem__(0, data["runs"][1]),
            lambda data: data["runs"][0].update(conclusion="future"),
            lambda data: data["runs"][0].update(model_id="different"),
            lambda data: data["runs"][0]["coverage"].update(evaluated_nodes=True),
            lambda data: data["runs"][0]["coverage"].update(evaluated_nodes=-1),
            lambda data: data["runs"][0]["coverage"].update(evaluated_nodes=20),
            lambda data: data["runs"][0]["coverage"].update(completed_paths=4),
            lambda data: data["runs"][0].update(witness=[step()]),
            lambda data: data["runs"][0]["initial"]["values"]["case"].update(
                type="integer"
            ),
            lambda data: data["runs"][0]["initial"]["values"]["case"].update(
                type="bool", value=1
            ),
        ]
        for mutation in mutations:
            data = copy.deepcopy(base)
            mutation(data)
            with self.subTest(mutation=mutation), self.assertRaises(ReportError):
                parse_report(data)

    def test_inconclusive_and_runtime_failures_have_distinct_nonzero_status(self):
        for conclusion, expected in [
            ("inconclusive", 2),
            ("runtime_failed", 1),
            ("violation", 1),
        ]:
            data = report_data()
            data["runs"][0]["conclusion"] = conclusion
            _, commands, status = render(parse_report(data))
            self.assertEqual(status, expected)
            self.assertEqual(len(commands), 1)
        data["runs"][1]["conclusion"] = "inconclusive"
        self.assertEqual(render(parse_report(data))[2], 2)

    def test_report_text_cannot_create_markdown_links_or_workflow_commands(self):
        payload = "</pre>\n::error file=evil.rs::injected\n[click](https://evil.invalid)\n<script>bad</script>"
        data = report_data()
        data["virtual_source"]["content"] = payload
        for run in data["runs"]:
            run["property"] = payload
        data["runs"][0].update(conclusion="violation", witness=[step()])
        data["runs"][0]["witness"][0]["action"] = payload
        summary, commands, status = render(parse_report(data))
        self.assertEqual(status, 1)
        self.assertIn("&lt;/pre&gt;", summary)
        self.assertNotIn("<script>", summary)
        self.assertEqual(len(commands), 1)
        self.assertNotIn("injected", commands[0])
        self.assertTrue(commands[0].startswith("::error::"))
        self.assertEqual(
            command_text("a%,b:\nc\r", property_value=True), "a%25%2Cb%3A%0Ac%0D"
        )

    def test_portable_sources_reject_escaping_paths_and_bad_positions(self):
        for path in [
            "/tmp/file.rs",
            "../file.rs",
            "a/../file.rs",
            "a\\file.rs",
            "https://bad",
            "a\nfile.rs",
            "a//file.rs",
        ]:
            data = report_data()
            data["runs"][0].update(
                conclusion="violation", witness=[step("repository", path)]
            )
            with self.subTest(path=path), self.assertRaises(ReportError):
                parse_report(data)
        for source in [
            dict(kind="unknown", path="a.rs", line=1, column=1),
            dict(kind="virtual", path="archive.recite", line=0, column=1),
        ]:
            data["runs"][0]["witness"][0]["source"] = source
            with self.assertRaises(ReportError):
                parse_report(data)

    def test_source_navigation_checks_file_line_and_symlink_containment(self):
        with (
            tempfile.TemporaryDirectory() as directory,
            tempfile.TemporaryDirectory() as outside,
        ):
            root = Path(directory)
            (root / "host.rs").write_text("host declaration\n", encoding="utf-8")
            (Path(outside) / "outside.rs").write_text("outside\n", encoding="utf-8")
            (root / "escape.rs").symlink_to(Path(outside) / "outside.rs")
            data = report_data()
            data["runs"][0].update(
                conclusion="violation", witness=[step("repository", "host.rs")]
            )
            view = parse_report(data)
            self.assertEqual(len(validate_locations(view, root)), 1)
            with patch.dict(
                "os.environ",
                {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_SHA": "a" * 40},
            ):
                summary, commands, _ = render(view)
            self.assertIn(
                "https://github.com/owner/repo/blob/" + "a" * 40 + "/host.rs#L1",
                summary,
            )
            self.assertIn("file=host.rs,line=1,col=1", commands[0])
            for source in [
                step("repository", "missing.rs"),
                step("repository", "escape.rs"),
                step("repository", "host.rs", 2),
                step("virtual", "unknown.recite"),
                step("virtual", "archive.recite", 3),
            ]:
                data["runs"][0]["witness"] = [source]
                with self.subTest(source=source), self.assertRaises(ReportError):
                    validate_locations(parse_report(data), root)

    def test_unsupported_diagnostic_is_escaped_and_never_passes(self):
        data = report_data()
        data.update(
            kind="unsupported",
            message="bad\n::error::injected%",
            source=step()["source"],
        )
        data.pop("runs")
        view = parse_report(data)
        summary, commands, status = render(view)
        self.assertEqual(status, 2)
        self.assertIn("analysis did not run", summary)
        self.assertEqual(len(commands[0].splitlines()), 1)
        self.assertIn("%0A::error::injected%25", commands[0])

    def test_fixed_witness_source_cost_reads_each_repository_file_once(self):
        # Measured after caching: 40 repeated references require one file read.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "host.rs").write_text("host declaration\n", encoding="utf-8")
            data = report_data()
            for run in data["runs"]:
                run.update(
                    conclusion="violation", witness=[step("repository", "host.rs")] * 5
                )
            view = parse_report(data)
            original = Path.read_text
            with patch.object(
                Path, "read_text", autospec=True, side_effect=original
            ) as read:
                locations = validate_locations(view, root)
            self.assertEqual(len(locations), 40)
            self.assertEqual(read.call_count, 1)
            summary, commands, status = render(view)
            self.assertEqual(status, 1)
            self.assertEqual(len(commands), 8)
            self.assertEqual(summary.count("captured prompt"), 40)


class ReportCliTests(unittest.TestCase):
    def test_required_value_shapes_and_complete_tree_counts_are_validated(self):
        for change in [
            lambda data: data.update(tool_version=""),
            lambda data: data.update(tool_version="bad\0text"),
            lambda data: data.update(virtual_source=[]),
            lambda data: data.update(runs={}),
            lambda data: data["runs"][0]["initial"].update(values={}),
            lambda data: data["runs"][0]["coverage"].update(generated_nodes=100),
            lambda data: data["runs"][0]["coverage"].update(
                evaluated_nodes=0, completed_paths=0
            ),
            lambda data: data["runs"][0].update(
                conclusion="violation", witness=[step()] * 6
            ),
        ]:
            data = report_data()
            change(data)
            with self.subTest(change=change), self.assertRaises(ReportError):
                parse_report(data)
        with self.assertRaises(ReportError):
            parse_report(None)

    def test_real_cli_pass_failure_unsupported_and_invalid_input(self):
        script = Path(__file__).resolve().parents[2] / "scripts/report_consumer.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "host.rs").write_text("host declaration\n", encoding="utf-8")
            path = root / "report.json"
            summary = root / "summary.md"
            data = report_data()
            for case, expected in [
                ("pass", 0),
                ("failure", 1),
                ("unsupported", 2),
                ("invalid", 2),
            ]:
                value = copy.deepcopy(data)
                if case == "failure":
                    action = step()
                    action["declaration"] = step("repository", "host.rs")["source"]
                    value["runs"][0].update(conclusion="violation", witness=[action])
                if case == "unsupported":
                    value.update(
                        kind="unsupported",
                        message="compiled expression unsupported",
                        source=step()["source"],
                    )
                path.write_text(
                    "{" if case == "invalid" else json.dumps(value), encoding="utf-8"
                )
                command = [
                    sys.executable,
                    str(script),
                    str(path),
                    "--repo-root",
                    str(root),
                ]
                if case in {"pass", "failure"}:
                    command += ["--summary", str(summary)]
                result = subprocess.run(
                    command, capture_output=True, text=True, check=False
                )
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertEqual(result.stderr, "")
                if case == "failure":
                    self.assertIn("file=host.rs,line=1,col=1", result.stdout)
                    self.assertIn(
                        "Host declaration:", summary.read_text(encoding="utf-8")
                    )
                if case == "unsupported":
                    self.assertIn("Use --summary", result.stdout)
                    self.assertIn("no exhaustive result", result.stdout)
            missing = subprocess.run(
                [sys.executable, str(script), str(root / "missing.json")],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(missing.returncode, 2)
            self.assertIn("Invalid archive report", missing.stdout)


if __name__ == "__main__":
    unittest.main()
