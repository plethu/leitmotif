"""Require 90% line and 95% branch coverage from measured instrumentation."""

import argparse
import json
from pathlib import Path


def threshold_failures(lines, branches):
    """Use integer counts so rounding cannot turn a shortfall into a pass."""
    errors = []
    for metric, threshold, (covered, count) in [
        ("lines", 90, lines),
        ("branches", 95, branches),
    ]:
        if (
            type(covered) is not int
            or type(count) is not int
            or not 0 <= covered <= count
            or count == 0
        ):
            errors.append(f"{metric}: missing or invalid instrumentation counts")
        elif covered * 100 < count * threshold:
            errors.append(f"{metric}: {covered}/{count} is below {threshold}%")
    return errors


def failures(report):
    """Read LLVM's workspace summary, retaining the existing gate contract."""
    try:
        data = report["data"]
        if len(data) != 1:
            return ["expected one workspace coverage report"]
        totals = data[0]["totals"]
        return threshold_failures(
            (totals["lines"]["covered"], totals["lines"]["count"]),
            (totals["branches"]["covered"], totals["branches"]["count"]),
        )
    except KeyError, TypeError, IndexError:
        return ["missing LLVM workspace coverage totals"]


def python_failures(report):
    """Require actual branch instrumentation and coverage.py's raw counts."""
    try:
        if report["meta"]["branch_coverage"] is not True:
            return ["Python coverage requires branch instrumentation"]
        totals = report["totals"]
        return threshold_failures(
            (totals["covered_lines"], totals["num_statements"]),
            (totals["covered_branches"], totals["num_branches"]),
        )
    except KeyError, TypeError:
        return ["missing Python coverage totals"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--format", choices=["llvm", "python"], default="llvm")
    args = parser.parse_args()
    check = python_failures if args.format == "python" else failures
    errors = check(json.loads(args.report.read_text(encoding="utf-8")))
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("Coverage passed: at least 90% lines and 95% branches")


if __name__ == "__main__":
    main()
