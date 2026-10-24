"""Skip expensive CI only for a known documentation-only change set."""

import argparse
import os
import subprocess
from pathlib import Path


DOC_FILES = frozenset({"AGENTS.md", "ATTRIBUTIONS.md", "CONTRIBUTING.md"})


def needs_rust(paths):
    """Unknown inputs and empty diffs receive full coverage; README is compiled."""
    return not paths or any(
        path not in DOC_FILES
        and not (path.startswith("docs/") and path.endswith(".md"))
        for path in paths
    )


def changed_paths(base, head, *, pull_request=False):
    """Disable rename detection so both old and new paths affect coverage."""
    comparison = [f"{base}...{head}"] if pull_request else [base, head]
    result = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", "-z", *comparison, "--"],
        check=True,
        capture_output=True,
    )
    return os.fsdecode(result.stdout).rstrip("\0").split("\0") if result.stdout else []


def select_rust(event, base, head):
    """Missing history and non-change events conservatively select all checks."""
    if event not in {"push", "pull_request"} or not base or set(base) == {"0"}:
        return True
    try:
        return needs_rust(changed_paths(base, head, pull_request=event == "pull_request"))
    except subprocess.CalledProcessError:
        return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--base", default="")
    parser.add_argument("--head", default="HEAD")
    args = parser.parse_args()
    selected = select_rust(args.event, args.base, args.head)
    output = f"rust={str(selected).lower()}\n"
    print(output, end="")
    if destination := os.environ.get("GITHUB_OUTPUT"):
        with Path(destination).open("a", encoding="utf-8") as handle:
            handle.write(output)


if __name__ == "__main__":
    main()
