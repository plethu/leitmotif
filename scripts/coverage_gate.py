"""Require 90% line and 95% branch coverage in an LLVM JSON report."""

import argparse
import json
from pathlib import Path


def failures(report):
    """Use integer counts so rounding cannot turn a shortfall into a pass."""
    try:
        data = report["data"]
        if len(data) != 1:
            return ["expected one workspace coverage report"]
        totals = data[0]["totals"]
        errors = []
        for metric, threshold in [("lines", 90), ("branches", 95)]:
            covered, count = totals[metric]["covered"], totals[metric]["count"]
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
    except KeyError, TypeError, IndexError:
        return ["missing LLVM workspace coverage totals"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    errors = failures(json.loads(args.report.read_text(encoding="utf-8")))
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("Coverage passed: at least 90% lines and 95% branches")


if __name__ == "__main__":
    main()
