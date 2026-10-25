"""Measure one private symbolic-town executable in bounded fresh Linux processes."""

import argparse
import hashlib
import json
import math
import os
import platform
import resource
import subprocess
import sys
import time
from pathlib import Path


def bounded_integer(value):
    """Require a positive resource/repetition argument."""
    result = int(value)
    if result < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return result


def text(value):
    """Timeout diagnostics may be bytes even when the child requested text mode."""
    return (
        value.decode("utf-8", errors="replace") if isinstance(value, bytes) else value
    )


def limits(seconds, memory_mib):
    """Apply limits only in the single-threaded child's pre-exec boundary."""
    cpu = math.ceil(seconds)
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu + 1))
    memory = memory_mib * 1024 * 1024
    resource.setrlimit(resource.RLIMIT_AS, (memory, memory))


def backend_environment():
    """Bound OxiDD's default 1 GiB worker stack while preserving explicit tuning."""
    return {"OXIDD_STACK_SIZE": os.environ.get("OXIDD_STACK_SIZE", "8388608")}


def measure_child(command, seconds, memory_mib):
    """Run once per fresh worker so RUSAGE_CHILDREN describes exactly this child."""
    start = time.perf_counter()
    result = {"command": command, "timeout_seconds": seconds, "memory_mib": memory_mib}
    try:
        child = subprocess.run(
            command,
            check=False,
            text=True,
            capture_output=True,
            timeout=seconds,
            preexec_fn=lambda: limits(seconds, memory_mib),
            env=os.environ | backend_environment(),
        )
    except subprocess.TimeoutExpired as error:
        result.update(
            status="timeout", stdout=text(error.stdout), stderr=text(error.stderr)
        )
    except OSError as error:
        result.update(status="spawn_error", diagnostic=str(error))
    except subprocess.SubprocessError as error:
        result.update(status="setup_failure", diagnostic=str(error))
    else:
        result.update(
            returncode=child.returncode, stdout=child.stdout, stderr=child.stderr
        )
        if child.returncode:
            result["status"] = "signal" if child.returncode < 0 else "process_failure"
        elif child.stderr:
            result["status"] = "unexpected_diagnostics"
        else:
            try:
                data = json.loads(child.stdout)
                conclusion = data["result"]["conclusion"]
                if conclusion not in {"reached", "unreachable", "inconclusive"}:
                    raise ValueError("unknown conclusion")
                search = data["result"]
                coverage = search.get("coverage", {})
                if not isinstance(coverage, dict):
                    raise ValueError("invalid explicit coverage")
                exhausted = search.get("exhausted", coverage.get("exhausted"))
                if conclusion == "unreachable" and exhausted is not True:
                    raise ValueError("unreachable result is not exhausted")
                result.update(status=conclusion, measurement=data)
            except (ValueError, KeyError, TypeError) as error:
                result.update(status="invalid_output", diagnostic=str(error))
    result["process_seconds"] = time.perf_counter() - start
    result["peak_child_rss_bytes"] = (
        resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024
    )
    result["rss_scope"] = "whole child lifetime, including the pre-exec launcher"
    return result


def worker_command(args, fixture):
    """Keep each repetition's peak RSS independent of every previous run."""
    return [
        sys.executable,
        str(Path(__file__).resolve()),
        "--worker",
        "--binary",
        str(args.binary.resolve()),
        "--timeout",
        str(args.timeout),
        "--memory-mib",
        str(args.memory_mib),
        *fixture,
    ]


def main():
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog="Remaining arguments go to symbolic_town; use its --help for backend, family, order and query options.",
    )
    parser.add_argument(
        "--binary",
        type=Path,
        required=True,
        help="Release symbolic_town executable; this command never builds it.",
    )
    parser.add_argument(
        "--repeat",
        type=bounded_integer,
        default=3,
        help="Fresh worker processes per case (default: %(default)s).",
    )
    parser.add_argument(
        "--timeout",
        type=bounded_integer,
        default=30,
        help="Wall seconds per child, also used for its CPU soft limit (default: %(default)s).",
    )
    parser.add_argument(
        "--memory-mib",
        type=bounded_integer,
        default=1024,
        help="Child address-space cap in MiB, not a heap/RSS cap (default: %(default)s).",
    )
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args, fixture = parser.parse_known_args()
    if sys.platform != "linux":
        parser.error("resource limits and peak RSS currently require Linux")
    if args.worker:
        print(
            json.dumps(
                measure_child(
                    [str(args.binary.resolve()), *fixture],
                    args.timeout,
                    args.memory_mib,
                )
            )
        )
        return 0
    root = Path(__file__).resolve().parents[1]
    metadata = {
        "binary_sha256": hashlib.sha256(args.binary.resolve().read_bytes()).hexdigest(),
        "lock_sha256": hashlib.sha256(
            (root / "examples/symbolic-town/Cargo.lock").read_bytes()
        ).hexdigest(),
        "source_dirty": bool(
            subprocess.run(
                ["git", "status", "--porcelain"],
                cwd=root,
                check=True,
                text=True,
                capture_output=True,
            ).stdout
        ),
        "machine": {
            "system": platform.system(),
            "release": platform.release(),
            "architecture": platform.machine(),
            "logical_cpus": os.cpu_count(),
            "physical_memory_bytes": os.sysconf("SC_PHYS_PAGES")
            * os.sysconf("SC_PAGE_SIZE"),
            "processor": next(
                (
                    line.split(":", 1)[1].strip()
                    for line in Path("/proc/cpuinfo").read_text().splitlines()
                    if line.startswith("model name")
                ),
                "unknown",
            ),
        },
        "rustc": subprocess.run(
            ["rustc", "--version", "--verbose"],
            cwd=root,
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip(),
        "source_revision": subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip(),
        "python": platform.python_version(),
        "backend_environment": backend_environment(),
    }
    for repetition in range(1, args.repeat + 1):
        child = subprocess.run(
            worker_command(args, fixture),
            check=True,
            text=True,
            capture_output=True,
            timeout=args.timeout + 10,
        )
        print(
            json.dumps(
                {**metadata, "repetition": repetition, **json.loads(child.stdout)}
            ),
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
