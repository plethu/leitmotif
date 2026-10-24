"""Run release-mode town measurements in fresh Linux processes."""

import argparse
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path


def positive_integer(value: str) -> int:
    """Parse a nonempty repetition count."""
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("repeat must be positive")
    return number


def command_output(command: list[str], root: Path) -> str:
    """Capture a short tool or source-identification command."""
    return subprocess.run(
        command, cwd=root, check=True, text=True, stdout=subprocess.PIPE
    ).stdout.strip()


def build_example(root: Path) -> Path:
    """Build once and obtain Cargo's exact executable path."""
    output = command_output(
        [
            "cargo",
            "build",
            "--locked",
            "--release",
            "--package",
            "leitmotif",
            "--example",
            "town_scaling",
            "--message-format=json",
        ],
        root,
    )
    for line in output.splitlines():
        artifact = json.loads(line)
        if (
            artifact.get("reason") == "compiler-artifact"
            and artifact["target"]["name"] == "town_scaling"
            and artifact.get("executable")
        ):
            return Path(artifact["executable"])
    raise RuntimeError("Cargo did not report the town_scaling executable")


def measure(command: list[str]) -> dict:
    """Measure one fresh executable, preserving its analysis and memory metrics."""
    start = time.perf_counter()
    child = subprocess.run(command, text=True, capture_output=True, check=False)
    elapsed = time.perf_counter() - start
    if child.returncode != 0:
        raise RuntimeError(f"measurement failed ({child.returncode}): {child.stderr}")
    if child.stderr:
        raise RuntimeError(f"unexpected measurement diagnostics: {child.stderr}")
    return {**json.loads(child.stdout), "process_seconds": elapsed}


def main() -> int:
    """Emit one JSON record per isolated repetition."""
    parser = argparse.ArgumentParser(
        description=(
            "Measure synthetic towns in release mode; remaining arguments go to "
            "the town_scaling example (threads, local-domain, coupling, resource, "
            "query, max-states, max-depth)."
        )
    )
    parser.add_argument("--repeat", type=positive_integer, default=3)
    args, fixture_args = parser.parse_known_args()
    if sys.platform != "linux":
        parser.error("peak RSS measurements require Linux with /proc mounted")
    root = Path(__file__).resolve().parents[1]
    try:
        executable = build_example(root)
        metadata = {
            "machine": {
                "system": platform.system(),
                "release": platform.release(),
                "architecture": platform.machine(),
                "processor": next(
                    (
                        line.split(":", 1)[1].strip()
                        for line in Path("/proc/cpuinfo").read_text().splitlines()
                        if line.startswith("model name")
                    ),
                    "unknown",
                ),
                "physical_memory_bytes": os.sysconf("SC_PHYS_PAGES")
                * os.sysconf("SC_PAGE_SIZE"),
                "logical_cpus": os.cpu_count(),
            },
            "toolchain": {
                "rustc": command_output(["rustc", "--version", "--verbose"], root),
                "cargo": command_output(["cargo", "--version"], root),
                "python": platform.python_version(),
            },
            "build": {
                "profile": "release",
                "rustflags": os.environ.get("RUSTFLAGS", ""),
                "encoded_rustflags": os.environ.get("CARGO_ENCODED_RUSTFLAGS", ""),
            },
            "source": {
                "revision": command_output(["git", "rev-parse", "HEAD"], root),
                "dirty": bool(command_output(["git", "status", "--porcelain"], root)),
            },
        }
        for repetition in range(1, args.repeat + 1):
            measurement = measure([str(executable), *fixture_args])
            print(
                json.dumps({**metadata, "repetition": repetition, **measurement}),
                flush=True,
            )
    except (OSError, subprocess.CalledProcessError, RuntimeError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
