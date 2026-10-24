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

JSON reports use the experimental schema `leitmotif.experimental.v1`. They
include queries, supplied premises, sources, and witnesses. Neither the format
nor internal state serialization is a stable persistence contract. Keep the
source revision and lockfile alongside results; reports do not yet contain a
model fingerprint. Inconsistency reports the supplied premise set, not a minimized
unsatisfiable core.

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
