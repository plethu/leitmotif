"""Render archive report data for CI; incomplete or unsupported analysis never passes."""

import argparse
import html
import json
import os
import re
import sys
from pathlib import Path
from urllib.parse import quote

# Support both the direct script and test imports without altering sys.path.
if __package__:
    from .report_view import Analysis, ReportError, Status, Unsupported, parse_report
else:
    from report_view import Analysis, ReportError, Status, Unsupported, parse_report


def command_text(value, *, property_value=False):
    value = value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    if property_value:
        value = value.replace(":", "%3A").replace(",", "%2C")
    return value


def pre(value):
    return "<pre>" + html.escape(str(value)) + "</pre>\n"


def validate_locations(report, root):
    locations = (
        [report.location]
        if isinstance(report, Unsupported)
        else [
            location
            for run in report.runs
            for step in run.witness
            for location in (step.source, step.declaration)
            if location is not None
        ]
    )
    root = root.resolve()
    line_counts = {}
    virtual_lines = len(report.source.content.splitlines())
    for location in locations:
        if location.kind == "virtual":
            if location.path != report.source.path:
                raise ReportError("unknown virtual source")
            count = virtual_lines
        else:
            path = (root / location.path).resolve()
            if not path.is_relative_to(root) or not path.is_file():
                raise ReportError(
                    "repository source is missing or escapes the checkout"
                )
            if path not in line_counts:
                line_counts[path] = len(path.read_text(encoding="utf-8").splitlines())
            count = line_counts[path]
        if location.line > count:
            raise ReportError("source line is outside the available file")
    return locations


def source_base():
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    revision = os.environ.get("GITHUB_SHA", "")
    if re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) and re.fullmatch(
        r"[a-fA-F0-9]{40}", revision
    ):
        return f"https://github.com/{repository}/blob/{revision}/"
    return "./"


def location_html(location):
    label = f"{location.path}:{location.line}:{location.column}"
    escaped_label = "".join(f"&#{ord(char)};" for char in label)
    if location.kind == "virtual":
        return "Virtual source: <code>" + escaped_label + "</code>"
    url = source_base() + quote(location.path, safe="/") + f"#L{location.line}"
    return '<a href="' + html.escape(url, quote=True) + '">' + escaped_label + "</a>"


def annotation(level, message, location=None):
    properties = ""
    if location and location.kind == "repository":
        properties = (
            " file="
            + command_text(location.path, property_value=True)
            + f",line={location.line},col={location.column}"
        )
    return f"::{level}{properties}::" + command_text(message)


def assignment(values):
    return pre("\n".join(f"{key} = {value}" for key, value in values))


def render(report):
    """Only validated views reach Markdown and workflow command rendering."""
    lines = ["## Leitmotif archive analysis\n", pre(f"Tool: {report.tool_version}")]
    commands = []
    if isinstance(report, Unsupported):
        lines += [
            "### Unsupported — analysis did not run\n",
            pre(report.message),
            location_html(report.location),
        ]
        commands.append(
            annotation(
                "error",
                "Unsupported archive analysis: " + report.message,
                report.location,
            )
        )
        status = 2
    else:
        first = report.runs[0]
        lines += [
            pre(
                f"Property: {first.property}\nModel: {first.model_id}\nRecite: {first.revision}\nFinite protocol:\n{first.protocol}"
            )
        ]
        statuses = {run.status for run in report.runs}
        status = (
            2
            if Status.INCONCLUSIVE in statuses
            else int(bool(statuses - {Status.EXHAUSTIVE}))
        )
        shown_witness = False
        for index, run in enumerate(report.runs, 1):
            coverage = run.coverage
            lines += [
                f"### Initial assignment {index}: {run.status.value}\n",
                "<details><summary>Initial assignment</summary>\n\n"
                + assignment(run.initial)
                + "\n</details>",
            ]
            lines += [
                pre(
                    f"Runtime tree: {coverage.evaluated} evaluated / {coverage.generated} generated; peak frontier {coverage.peak}; completed paths {coverage.completed}\nLimits: {coverage.max_nodes} nodes, {coverage.max_depth} actions deep"
                )
            ]
            if run.status != Status.EXHAUSTIVE:
                location = (
                    (run.witness[-1].declaration or run.witness[-1].source)
                    if run.witness
                    else None
                )
                commands.append(
                    annotation(
                        "error",
                        f"Archive assignment {index}: {run.status.value}",
                        location,
                    )
                )
            if run.witness:
                opened = " open" if not shown_witness else ""
                lines += [
                    f"<details{opened}><summary>Ordered runtime witness ({len(run.witness)} actions)</summary>\n"
                ]
                shown_witness = True
            for position, step in enumerate(run.witness, 1):
                lines += [
                    pre(f"{position}. {step.action} -> {step.outcome}"),
                    location_html(step.source),
                ]
                if step.declaration:
                    lines += ["Host declaration: " + location_html(step.declaration)]
                lines += [
                    "<details><summary>Host assignment after this action</summary>\n\n"
                    + assignment(step.host)
                    + "\n</details>"
                ]
            if run.witness:
                lines += ["</details>\n"]
        lines += ["Exhaustion applies only to the stated finite protocol.\n"]
    lines += [
        "### Attached virtual dialogue source\n",
        "<details><summary>Dialogue content (virtual file)</summary>\n\n"
        + pre(report.source.path + "\n" + report.source.content)
        + "\n</details>",
    ]
    return "\n".join(lines) + "\n", commands, status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--summary", type=Path, default=os.environ.get("GITHUB_STEP_SUMMARY")
    )
    args = parser.parse_args()
    try:
        report = parse_report(json.loads(args.report.read_text(encoding="utf-8")))
        validate_locations(report, args.repo_root)
        summary, commands, status = render(report)
    except (OSError, UnicodeError, json.JSONDecodeError, ReportError) as error:
        print(annotation("error", "Invalid archive report: " + str(error)))
        return 2
    if args.summary:
        with args.summary.open("a", encoding="utf-8") as output:
            output.write(summary)
    if isinstance(report, Analysis):
        evaluated = sum(run.coverage.evaluated for run in report.runs)
        print(
            f"Archive analysis: {'passed' if status == 0 else 'did not pass'}; 8 assignments, {evaluated} runtime nodes evaluated."
        )
    else:
        print("Archive analysis: unsupported; no exhaustive result.")
    if not args.summary:
        print("Use --summary PATH to save the ordered witness, coverage and sources.")
    for command in commands:
        print(command)
    return status


if __name__ == "__main__":
    sys.exit(main())
