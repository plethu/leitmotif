#!/usr/bin/env python
"""Controlled solver responses for testing evidence validation, not a real solver."""

import sys
from pathlib import Path


def commands():
    """Collect the generated multiline commands used by this controlled fixture."""
    parts = []
    balance = 0
    for line in sys.stdin:
        parts.append(line.strip())
        balance += line.count("(") - line.count(")")
        if balance == 0:
            yield " ".join(parts)
            parts.clear()


def info_response(case: str, flag: str) -> str:
    """Return complete, invalid or escaped responses for one requested info field."""
    return {
        "unsupported": "unsupported",
        "error": '(error "info unavailable")',
        "atom": "unrelated",
        "extra": f'({flag} "fixture" extra)',
        "trailing": f'({flag} "fixture") extra',
        "unclosed": f'({flag} "fixture)',
        "malformed-escape": f'({flag} "fixture"")',
        "unsupported-extra": "unsupported extra",
        "wrong-flag": '(:other "fixture")',
        "unquoted": f"({flag} fixture)",
        "escaped": f'({flag} "fixture ""quoted"" ) λ")',
        "unicode-atom": "λλ",
        "unicode-unquoted": f"({flag} λ)",
        "unicode-header": '(:λ "fixture")',
        "unicode-prefix": f'λ({flag} "fixture")',
        "unicode-trailer": f'({flag} "fixture") λ',
        "unicode-extra": f'({flag} "fixture" λ)',
        "unicode-error-header": '(errorλ "fixture")',
        "unicode-unsupported": "unsupportedλ",
        "unicode-error": '(error "δ unavailable")',
    }[case]


def main() -> None:
    """Reply to the small command sequence exercised by the SMT adapter."""
    mode = Path(sys.argv[0]).name
    checks = 0
    for command in commands():
        if command == "(exit)":
            return
        if mode == "unsupported-config" and command.startswith("(set-option"):
            response = "unsupported"
        elif command.startswith("(get-info :version"):
            response = (
                info_response(mode.removeprefix("version-"), ":version")
                if mode.startswith("version-")
                else '(:version "fixture")'
            )
        elif command.startswith("(get-info :reason-unknown"):
            if mode.startswith("reason-"):
                response = info_response(
                    mode.removeprefix("reason-"), ":reason-unknown"
                )
            else:
                reason = "timeout" if mode == "timeout" else "incomplete fixture"
                response = (
                    "not-a-reason"
                    if mode == "bad-reason"
                    else f'(:reason-unknown "{reason}")'
                )
        elif command == "(check-sat)":
            checks += 1
            if (
                mode.startswith("reason-")
                or mode in {"timeout", "unknown", "bad-reason"}
                or (mode in {"second-support", "second-oppose"} and checks == 2)
            ):
                response = "unknown"
            elif mode == "timeout-token":
                response = "timeout"
            elif mode == "unsupported":
                response = "unsupported"
            elif mode == "malformed":
                response = "maybe"
            elif mode in {"bad-core", "duplicate-core"} or (
                mode in {"shuffled", "version-escaped"} and checks == 2
            ):
                response = "unsat"
            else:
                response = "sat"
        elif command.startswith("(get-value"):
            responses = {
                "second-support": "((f0 1))",
                "shuffled": "((f1 0) (f0 1))",
                "prefix": "((f0 1junk))",
                "negative": "((f0 (- 1)))",
                "overflow": "((f0 184467440737095516160000))",
                "fraction": "((f0 1.0))",
                "out-of-domain": "((f0 2))",
                "missing": "()",
                "duplicate": "((f0 0) (f0 0))",
                "foreign": "((foreign 0))",
            }
            response = responses.get(mode, "((f0 0))")
        elif command == "(get-unsat-core)":
            response = {
                "bad-core": "(p999)",
                "duplicate-core": "(p0 p0)",
            }.get(mode, "(h)")
        else:
            response = "success"
        print(response, flush=True)


if __name__ == "__main__":
    main()
