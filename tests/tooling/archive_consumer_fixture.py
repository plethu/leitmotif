"""Run real archive producers through the independent Python consumer process."""

import argparse
import html
import json
import subprocess
import sys
import tempfile
from pathlib import Path


def verify(producer, root):
    with tempfile.TemporaryDirectory() as directory:
        directory = Path(directory)
        for label, arguments, expected, phrase in [
            ("corrected", [], 0, "exhaustive"),
            ("faulty", ["--variant", "faulty"], 1, "violation"),
            ("truncated", ["--max-nodes", "0"], 2, "inconclusive"),
            ("unsupported", ["--variant", "unsupported"], 2, "Unsupported"),
        ]:
            result = subprocess.run(
                [producer, "--json", *arguments],
                capture_output=True,
                text=True,
                check=True,
            )
            if result.stderr:
                raise AssertionError(
                    f"{label}: unexpected producer diagnostics: {result.stderr}"
                )
            report = directory / f"{label}.json"
            report.write_text(result.stdout, encoding="utf-8")
            data = json.loads(result.stdout)
            summary = directory / f"{label}.md"
            command = [
                sys.executable,
                str(root / "scripts/report_consumer.py"),
                str(report),
                "--repo-root",
                str(root),
                "--summary",
                str(summary),
            ]
            rendered = subprocess.run(
                command, capture_output=True, text=True, check=False
            )
            if rendered.returncode != expected:
                raise AssertionError(
                    f"{label}: expected {expected}, got {rendered.returncode}: {rendered.stdout} {rendered.stderr}"
                )
            text = summary.read_text(encoding="utf-8")
            if phrase not in text or "Attached virtual dialogue source" not in text:
                raise AssertionError(f"{label}: summary lost conclusion or source")
            if label == "faulty":
                witness = next(
                    run["witness"]
                    for run in data["runs"]
                    if run["conclusion"] == "violation"
                )
                positions = [
                    text.index(
                        html.escape(f"{index}. {item['action']} -> {item['outcome']}")
                    )
                    for index, item in enumerate(witness, 1)
                ]
                if (
                    positions != sorted(positions)
                    or "Virtual source:" not in text
                    or "file=crates/leitmotif-recite/" not in rendered.stdout
                ):
                    raise AssertionError(
                        "faulty witness lost order or actual source navigation"
                    )
            if label == "corrected" and (
                "evaluated /" not in text or len(data["runs"]) != 8
            ):
                raise AssertionError("corrected scope or coverage missing")
            if label == "corrected":
                data["runs"][0]["coverage"]["generated_nodes"] = 100
                report.write_text(json.dumps(data), encoding="utf-8")
                rejected = subprocess.run(
                    command, capture_output=True, text=True, check=False
                )
                if rejected.returncode != 2 or "exhaustive tree" not in rejected.stdout:
                    raise AssertionError(
                        "unevaluated generated nodes were accepted as exhaustive"
                    )
        print(
            "Archive consumer: corrected, faulty, truncated and unsupported subprocesses passed"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("producer")
    args = parser.parse_args()
    verify(args.producer, Path(__file__).resolve().parents[2])


if __name__ == "__main__":
    main()
