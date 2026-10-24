# Running the examples

The example CLI runs models defined in Rust. It accepts a scenario and optional
search limits; it does not load a separate authoring format.

```sh
mise exec -- just demo town
mise exec -- just demo blocked-ending
mise exec -- just demo disclosure
mise exec -- just demo mystery --json
mise exec -- just demo revealed-mystery
mise exec -- just demo contradiction
mise exec -- just demo town --max-states 1
```

Without mise, use `cargo run --locked -p leitmotif --example check --` followed
by the same arguments. `--help` lists the available limits. The toolchain is
pinned in `.mise.toml`.

## Scenarios

| Scenario | What to look for |
| --- | --- |
| `town` | Two reachable endings and a learning-before-disclosure invariant |
| `blocked-ending` | An ending gate that is satisfiable as an assignment but unreachable through events; another ending survives |
| `disclosure` | A missing learning guard produces a replayable invariant violation |
| `mystery` | The supplied evidence leaves more than one explanation possible |
| `revealed-mystery` | Another premise rules out a previously possible explanation |
| `contradiction` | Incompatible premises admit no model; they do not establish an answer |

The town also permits pretending to know something. The rule applies to truthful
disclosure; a claim and its truth are represented separately.

## Reading the result

Reachability and invariant checks explore transitions from one explicit initial
state. Hypothesis classification enumerates assignments admitted by its named
premises. Transitions and invariants do not silently become premises for that
second analysis.

A result includes coverage, limits, and evidence. `Possible` establishes an
assignment supporting a hypothesis. If enumeration stopped early, that does not
also establish that the hypothesis is optional: inspect the counterexample and
coverage before inferring non-necessity. `Required`, `RuledOut`, and
`Inconsistent` need complete enumeration. An unfinished search with no supporting
witness is inconclusive.

JSON reports use the experimental schema `leitmotif.experimental.v2`. They
include the library version, model fingerprint, query, explicit premises, limits,
coverage and evidence. Reachability and safety reports retain the complete
initial assignment, including settings, even when no witness exists.

The SHA-256 model fingerprint covers the context, fact keys, kinds and ordered
finite domains, transitions (IDs, guards and effects), and invariants, preserving
declaration order. It excludes source locations, runtime handle ownership and
run inputs. Compare the query, premises, initial assignment and limits as well as
the fingerprint when comparing runs. A fingerprint identifies a model; it cannot
reconstruct it. Retain the Rust model source revision, Cargo.lock and toolchain
alongside results to reproduce a check.

Compiler source locations remain in `file`. Relative compiler paths also appear
in `portable_file`; use `ModelBuilder::with_source_root` to obtain checkout-relative
aliases for absolute paths under an explicit root. Absolute paths outside that
root retain their originals without a relative alias. Locations do not affect
the model fingerprint, so relocating a checkout does not change model identity.

The report format remains experimental, with no stable persistence promise or
state deserialization API. Inconsistency reports the supplied premise set, not a
minimized unsatisfiable core.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Completed without a reported failure; a ruled-out hypothesis is a valid classification |
| `1` | Unreachable requested ending, invariant violation, inconsistent premises, or application error |
| `2` | Inconclusive analysis or CLI usage error |

Intentional failure fixtures return nonzero. In a combined report, an
inconclusive result takes precedence over failure; inspect the individual
conclusions as well as the process status.

For development commands, see [CONTRIBUTING.md](../CONTRIBUTING.md).
