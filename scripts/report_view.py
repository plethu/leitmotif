"""Validate the small experimental archive display contract at one boundary."""

from dataclasses import dataclass
from enum import StrEnum
from pathlib import PurePosixPath

SCHEMA = "leitmotif.archive-ci.experimental.v1"


class ReportError(ValueError):
    """Input cannot be interpreted as this consumer's supported report view."""


def object_value(value):
    if not isinstance(value, dict):
        raise ReportError("expected an object")
    return value


def text(value):
    if not isinstance(value, str) or not value.strip() or "\0" in value:
        raise ReportError("expected nonempty text")
    return value


def integer(value, minimum=0):
    if type(value) is not int or value < minimum:
        raise ReportError("expected a nonnegative integer")
    return value


def sequence(value):
    if not isinstance(value, list):
        raise ReportError("expected an array")
    return value


def portable_path(value):
    value = text(value)
    if (
        PurePosixPath(value).is_absolute()
        or any(part in {"", ".", ".."} for part in value.split("/"))
        or any(char in value for char in "\\\r\n:")
    ):
        raise ReportError("expected a portable relative source path")
    return value


class Status(StrEnum):
    EXHAUSTIVE = "exhaustive"
    VIOLATION = "violation"
    RUNTIME_FAILED = "runtime_failed"
    INCONCLUSIVE = "inconclusive"


@dataclass(frozen=True)
class Location:
    kind: str
    path: str
    line: int
    column: int

    @classmethod
    def parse(cls, value):
        value = object_value(value)
        kind = value["kind"]
        if kind not in {"repository", "virtual"}:
            raise ReportError("unknown source kind")
        return cls(
            kind,
            portable_path(value["path"]),
            integer(value["line"], 1),
            integer(value["column"], 1),
        )


def snapshot(value):
    values = object_value(object_value(value)["values"])
    result = []
    for key, encoded in values.items():
        key = text(key)
        encoded = object_value(encoded)
        item = encoded["value"]
        match encoded["type"]:
            case "bool" if type(item) is bool:
                pass
            case "symbol":
                item = text(item)
            case _:
                raise ReportError("unsupported snapshot value")
        result.append((key, item))
    if not result:
        raise ReportError("initial or witness assignment is empty")
    return tuple(sorted(result))


@dataclass(frozen=True)
class Step:
    action: str
    outcome: str
    source: Location
    declaration: Location | None
    host: tuple

    @classmethod
    def parse(cls, value):
        value = object_value(value)
        return cls(
            text(value["action"]),
            text(value["outcome"]),
            Location.parse(value["source"]),
            Location.parse(value["declaration"])
            if value["declaration"] is not None
            else None,
            snapshot(value["host"]),
        )


@dataclass(frozen=True)
class Coverage:
    evaluated: int
    generated: int
    peak: int
    completed: int
    max_nodes: int
    max_depth: int

    @classmethod
    def parse(cls, value, limits):
        value, limits = object_value(value), object_value(limits)
        result = cls(
            integer(value["evaluated_nodes"]),
            integer(value["generated_nodes"], 1),
            integer(value["peak_frontier"], 1),
            integer(value["completed_paths"]),
            integer(limits["max_nodes"]),
            integer(limits["max_depth"]),
        )
        if not (
            result.completed <= result.evaluated <= result.generated
            and result.peak <= result.generated
            and result.evaluated <= result.max_nodes
        ):
            raise ReportError("inconsistent runtime coverage counts")
        return result


@dataclass(frozen=True)
class Run:
    model_id: str
    property: str
    protocol: str
    revision: str
    initial: tuple
    coverage: Coverage
    status: Status
    witness: tuple[Step, ...]

    @classmethod
    def parse(cls, value):
        value = object_value(value)
        result = cls(
            text(value["model_id"]),
            text(value["property"]),
            text(value["protocol"]),
            text(value["recite_revision"]),
            snapshot(value["initial"]),
            Coverage.parse(value["coverage"], value["limits"]),
            Status(value["conclusion"]),
            tuple(Step.parse(step) for step in sequence(value["witness"])),
        )
        if len(result.witness) > result.coverage.max_depth:
            raise ReportError("witness exceeds declared depth bound")
        if result.status in {Status.EXHAUSTIVE, Status.INCONCLUSIVE} and result.witness:
            raise ReportError("unexpected witness for this conclusion")
        if result.status == Status.EXHAUSTIVE and (
            result.coverage.evaluated == 0
            or result.coverage.evaluated != result.coverage.generated
        ):
            raise ReportError("exhaustive tree must evaluate every generated node")
        return result


@dataclass(frozen=True)
class VirtualSource:
    path: str
    content: str


@dataclass(frozen=True)
class Analysis:
    tool_version: str
    source: VirtualSource
    runs: tuple[Run, ...]


@dataclass(frozen=True)
class Unsupported:
    tool_version: str
    source: VirtualSource
    message: str
    location: Location


def parse_report(value):
    """Ignore additive fields; never guess unknown versions, results, or sources."""
    try:
        value = object_value(value)
        if value["schema_version"] != SCHEMA:
            raise ReportError("unsupported archive CI schema version")
        version = text(value["tool_version"])
        virtual = object_value(value["virtual_source"])
        source = VirtualSource(portable_path(virtual["path"]), text(virtual["content"]))
        if value["kind"] == "unsupported":
            return Unsupported(
                version, source, text(value["message"]), Location.parse(value["source"])
            )
        if value["kind"] != "analysed":
            raise ReportError("unknown report result")
        runs = tuple(Run.parse(run) for run in sequence(value["runs"]))
        if len(runs) != 8 or len({run.initial for run in runs}) != 8:
            raise ReportError(
                "archive scope requires eight distinct initial assignments"
            )
        if (
            len(
                {
                    (run.model_id, run.property, run.protocol, run.revision)
                    for run in runs
                }
            )
            != 1
        ):
            raise ReportError("archive runs disagree on identity or scope")
        return Analysis(version, source, runs)
    except (KeyError, TypeError, ValueError) as error:
        if isinstance(error, ReportError):
            raise
        raise ReportError(f"malformed archive report: {error}") from error
