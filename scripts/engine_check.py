"""Run the shared finite harbour contract through actual .NET and Godot hosts."""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

from scripts.engine_coverage import check

ROOT = Path(__file__).resolve().parents[1]
CONSUMERS = ROOT / "examples/engine-contracts"


def run(command, expected=0, *, output=None, completion=None, normal_godot=False):
    """Bound every tool/engine process and retain visible stderr, including exit warnings."""
    result = subprocess.run(
        [str(part) for part in command],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
        timeout=120,
    )
    print(result.stderr, end="", file=sys.stderr)
    if normal_godot and "ERROR:" in result.stderr:
        raise ValueError("unexpected Godot error during normal lifecycle")
    if completion and completion not in result.stdout:
        raise ValueError("missing host conformance completion marker")
    if output is not None:
        output.write_text(result.stdout, encoding="utf-8")
    else:
        print(result.stdout, end="")
    if result.returncode != expected:
        raise ValueError(
            f"{command[0]} exited {result.returncode}, expected {expected}"
        )
    return result.stdout


def authored_sources():
    """Include new/untracked production sources too; compare full paths, never basenames."""
    files = subprocess.check_output(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            str(CONSUMERS),
        ],
        cwd=ROOT,
        text=True,
    ).splitlines()
    return {
        (ROOT / name).resolve()
        for name in files
        if name.endswith(".cs")
        and not (ROOT / name).resolve().is_relative_to(CONSUMERS / "tests")
    }


def execute(binary, output):
    output.mkdir(parents=True, exist_ok=True)
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    for name, query, depth, status in [
        ("sheltered", "sheltered", 3, 0),
        ("departed", "departed", 3, 0),
        ("unreachable", "storm-passed", 3, 1),
        ("inconclusive", "sheltered", 0, 2),
    ]:
        request = output / f"{name}.request.json"
        result = output / f"{name}.check.json"
        run(
            [
                binary,
                "--source-revision",
                revision,
                "request",
                "--query",
                query,
                "--max-depth",
                depth,
            ],
            output=request,
        )
        run(
            [binary, "--source-revision", revision, "check", request],
            status,
            output=result,
        )
        data = json.loads(result.read_text(encoding="utf-8"))
        if len(data["conformance"]["cases"]) != 30:
            raise ValueError("fixed harbour audit must contain exactly 30 cases")
    for name in [
        "Shared/Harbour",
        "Console/Harbour.Console",
        "tests/Harbour.Tests",
        "Godot/Harbour.Godot",
    ]:
        project = CONSUMERS / f"{name}.csproj"
        run(["dotnet", "restore", project, "--locked-mode"])
        run(["dotnet", "build", project, "--no-restore", "--configuration", "Debug"])
    godot = Path(shutil.which("godot-dotnet") or "godot-dotnet")
    godot_root = CONSUMERS / "Godot"
    run([godot, "--headless", "--editor", "--path", godot_root, "--import"])
    run(
        [godot, "--headless", "--path", godot_root, "res://Format.tscn"],
        normal_godot=True,
    )
    console = CONSUMERS / "Console/bin/Debug/net10.0/Harbour.Console.dll"
    for name, status in [
        ("sheltered", 0),
        ("departed", 0),
        ("unreachable", 1),
        ("inconclusive", 2),
    ]:
        args = [output / f"{name}.request.json", output / f"{name}.check.json"]
        run(
            ["dotnet", console, *args],
            status,
            completion="Harbour conformance:",
            output=output / f"normal-console-{name}.log",
        )
        run(
            [godot, "--headless", "--path", godot_root, "--", *args],
            status,
            completion="Harbour conformance:",
            normal_godot=True,
            output=output / f"normal-godot-{name}.log",
        )
    run(["dotnet", CONSUMERS / "tests/bin/Debug/net10.0/Harbour.Tests.dll", output])
    args = [output / "sheltered.request.json", output / "sheltered.check.json"]
    for mode in ["abort-node", "fail-after-node"]:
        run(
            [
                "env",
                f"HARBOUR_TEST_MODE={mode}",
                godot,
                "--headless",
                "--path",
                godot_root,
                "res://Coverage.tscn",
                "--",
                *args,
            ],
            3,
            output=output / f"{mode}.log",
        )
    common = ["--include", "[Harbour*]*", "--include-test-assembly", "--format", "json"]
    run(
        [
            "coverlet",
            console,
            "--target",
            "dotnet",
            "--targetargs",
            subprocess.list2cmdline(list(map(str, [console, *args]))),
            *common,
            "--output",
            output / "console-coverage.json",
        ]
    )
    godot_dll = godot_root / ".godot/mono/temp/bin/Debug/Harbour.Godot.dll"
    run(
        [
            "coverlet",
            godot_dll,
            "--target",
            godot,
            "--targetargs",
            subprocess.list2cmdline(
                list(
                    map(
                        str,
                        [
                            "--headless",
                            "--path",
                            godot_root,
                            "res://Coverage.tscn",
                            "--",
                            *args,
                        ],
                    )
                )
            ),
            *common,
            "--output",
            output / "godot-coverage.json",
        ],
        completion="Coverage harness complete",
    )
    print(
        check(
            [output / "console-coverage.json", output / "godot-coverage.json"],
            authored_sources(),
            [
                CONSUMERS / "Godot/.godot/mono/temp/obj",
                CONSUMERS / "Shared/obj",
                CONSUMERS / "Console/obj",
            ],
            CONSUMERS / "tests",
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(os.environ.get("CARGO_TARGET_DIR", "target")) / "engine-contracts",
    )
    args = parser.parse_args()
    execute(args.binary.resolve(), args.output.resolve())


if __name__ == "__main__":
    main()
