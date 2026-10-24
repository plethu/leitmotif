"""Measure every authored production C# file from unfiltered Coverlet reports."""

import json
from pathlib import Path

from scripts.coverage_gate import threshold_failures


def counts(reports, expected, generated_roots, test_root):
    """Union instrumented points by full source/method identity across host processes."""
    expected = {path.resolve() for path in expected}
    lines = {}
    branches = {}
    measured = set()
    for report in reports:
        for documents in report.values():
            for document, classes in documents.items():
                source = Path(document).resolve()
                if source not in expected:
                    generated = source.name.endswith(".generated.cs") and any(
                        source.is_relative_to(root.resolve())
                        for root in generated_roots
                    )
                    if generated or source.is_relative_to(test_root.resolve()):
                        continue
                    raise ValueError(f"unknown coverage source: {source}")
                measured.add(source)
                for type_name, methods in classes.items():
                    for method_name, method in methods.items():
                        for line, hits in method["Lines"].items():
                            _record(lines, (source, int(line)), hits)
                        for branch in method["Branches"]:
                            key = (
                                source,
                                type_name,
                                method_name,
                                branch["Offset"],
                                branch["EndOffset"],
                                branch["Path"],
                                branch["Ordinal"],
                            )
                            _record(branches, key, branch["Hits"])
    missing = expected - measured
    if missing:
        raise ValueError(f"missing authored source instrumentation: {sorted(missing)}")
    for source in expected:
        if not any(key[0] == source for key in lines):
            raise ValueError(f"no sequence points for authored source: {source}")
    return (sum(lines.values()), len(lines)), (sum(branches.values()), len(branches))


def _record(points, key, hits):
    if type(hits) is not int or hits < 0:
        raise ValueError("invalid coverage hit count")
    points[key] = points.get(key, False) or hits > 0


def check(paths, expected, generated_roots, test_root):
    """Apply the project's shared integer threshold owner without hiding source files."""
    reports = [json.loads(path.read_text(encoding="utf-8")) for path in paths]
    lines, branches = counts(reports, expected, generated_roots, test_root)
    failures = threshold_failures(lines, branches)
    if failures:
        raise ValueError("\n".join(failures))
    return f"Authored C# coverage: lines {lines[0]}/{lines[1]}, branches {branches[0]}/{branches[1]}"
