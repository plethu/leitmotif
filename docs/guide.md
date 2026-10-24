# Running the examples

The example CLI runs models defined in Rust. It accepts a scenario and optional search
limits; it does not load a separate authoring format.

```sh
mise exec -- just demo town
mise exec -- just demo blocked-ending
mise exec -- just demo disclosure
mise exec -- just demo mystery --json
mise exec -- just demo revealed-mystery
mise exec -- just demo contradiction
mise exec -- just demo town --max-states 1
```

Without mise, use `cargo run --locked -p leitmotif --example check --` followed by the
same arguments. `--help` lists the available limits. The toolchain is pinned in
`.mise.toml`.

## Scenarios

| Scenario           | What to look for                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------------------- |
| `town`             | Two reachable endings and a learning-before-disclosure invariant                                            |
| `blocked-ending`   | An ending gate that is satisfiable as an assignment but unreachable through events; another ending survives |
| `disclosure`       | A missing learning guard produces a replayable invariant violation                                          |
| `mystery`          | The supplied evidence leaves more than one explanation possible                                             |
| `revealed-mystery` | Another premise rules out a previously possible explanation                                                 |
| `contradiction`    | Incompatible premises admit no model; they do not establish an answer                                       |

The town also permits pretending to know something. The rule applies to truthful
disclosure; a claim and its truth are represented separately.

## Reading the result

Reachability and invariant checks explore transitions from one explicit initial state.
Hypothesis classification enumerates assignments admitted by its named premises.
Transitions and invariants do not silently become premises for that second analysis.

A result includes coverage, limits, and evidence. `Possible` establishes an assignment
supporting a hypothesis. If enumeration stopped early, that does not also establish that
the hypothesis is optional: inspect the counterexample and coverage before inferring
non-necessity. `Required`, `RuledOut`, and `Inconsistent` need complete enumeration. An
unfinished search with no supporting witness is inconclusive.

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

| Code | Meaning                                                                                        |
| ---- | ---------------------------------------------------------------------------------------------- |
| `0`  | Completed without a reported failure; a ruled-out hypothesis is a valid classification         |
| `1`  | Unreachable requested ending, invariant violation, inconsistent premises, or application error |
| `2`  | Inconclusive analysis or CLI usage error                                                       |

Intentional failure fixtures return nonzero. In a combined report, an inconclusive
result takes precedence over failure; inspect the individual conclusions as well as the
process status.

For state-space measurements, see [Measuring synthetic towns](scaling.md).
For development commands, see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Compare optional SMT classification

The experimental `smt` Cargo feature adds `Model::classify_smt` and the types in
`leitmotif::smt`. Supply an executable and a positive soft timeout with
`Options::new`. The default library and `Model::classify` need no solver.
The pinned Z3 release is installed through mise.

Compare the same authored mystery or a larger prerequisite set:

```sh
mise exec -- just smt-comparison --fixture mystery --engine enumeration
mise exec -- just smt-comparison --fixture mystery --engine smt
mise exec -- just smt-comparison --facts 20 --query inconsistent --engine enumeration
mise exec -- just smt-comparison --facts 20 --query inconsistent --engine smt
```

These commands print JSON measurements with the complete report. Solver reports
record at most two checks instead of enumeration coverage. `Possible` includes
both a supporting assignment and a counterexample; incomplete checks retain
only the assignment already validated, if any. Unsat cores identify supplied
premises, domain restrictions and hypothesis checks separately. They are solver
results, without a minimality or independently checked proof claim.

`--soft-timeout-ms` applies to each solver check. It does not bound startup,
protocol I/O, assignment/core retrieval or synchronous cleanup. Unsupported
commands and deployment failures return distinct errors. The experiment trusts
pinned Z3's ordinary ASCII replies: rsmt2 can panic on malformed non-ASCII check,
model or core responses. General solver deployment remains deferred.
Retain the model source, lockfile, toolchain and reported solver version when
comparing runs. Enumeration remains the reference for small finite cases.

## Progression traps

`Model::progression` checks whether each reachable nonterminal state has a route
to an explicitly permitted ending. Declare the terminal stopping predicate and
named permitted predicates with `ProgressionQuery::new` and
`PermittedEnding::new`. An intentional failure ending can be permitted even when
a preferred ending is no longer reachable.

```rust
let query = ProgressionQuery::new(
    ending_a.clone().or(ending_b.clone()),
    [
        PermittedEnding::new("saved", ending_a),
        PermittedEnding::new("evacuated", ending_b),
    ],
)?;
let report = model.progression(&initial, &query, limits)?;
```

Terminal states stop this query even if the model declares outgoing transitions.
Permitted predicates must identify terminal completions: a match at a visited
nonterminal returns `ErrorKind::NonterminalEnding`, with the ending's declaration
source and a full counterexample assignment. Validation checks the queried
reachable states; it does not enumerate every possible assignment. Named endings
may overlap, while duplicate or blank names are rejected. An empty permitted set
is valid.

On a complete queried graph, `Trap` returns the number of nonterminal states
without any permitted route and a shortest entry trace to the first one in BFS
order. `NoTraps` says every reachable nonterminal has such a route. Terminal
states are excluded from that count, including disallowed terminals, so an
initial terminal can produce `NoTraps` even with no permitted ending. Neither
verdict guarantees eventual completion or a winning strategy: a player can stay
in an escapable cycle.

Any state or depth truncation produces `Inconclusive`; absence of a route in the
partial graph is not proof of a trap. Reports retain the initial assignment,
model identity, terminal predicate, all considered endings, bounds and coverage.
The reverse pass retains admitted edges, including cycles and converging paths;
its memory grows with states and edges, rather than states alone.

## Compiled Recite guards

The `leitmotif-recite` workspace crate binds compiled Recite calls to finite facts.
It pins Recite compiler/runtime revision
`6ddf13bb944dc7b1f0528654a2e70e243e54bb6f`; the core library has no Recite dependency.

Register exact `CompiledConditionCall` values with `Bindings::bind_bool` or
`bind_enum`, passing a typed `FactHandle`. Enum facts must encode their finite
values as symbols matching Recite variants. Function names and typed arguments
both identify a binding: `ready(player)` and `ready("player")` are distinct calls.

Pass a compiled Boolean expression and its containing row's source-map span to
`Bindings::guard`. The returned `Predicate` works with ordinary finite analysis.
For compiled `:match` scrutinees, `Bindings::enum_is` builds a predicate for one
variant. Lowering validates every branch, including branches that evaluation
could skip; missing bindings, wrong result kinds and unsupported trees return
source-linked errors. Compiled expressions and their lowered binary predicates
are each limited to 128 child edges per path. A deeply nested, wide expression
can fit Recite's limit while exceeding the adapter's binary-depth limit; the
adapter rejects it before constructing an unsafe tree.

`Bindings::context(&model, &state)` implements Recite's `DialogueContext` over the
same immutable finite host assignment. The real runtime owns short-circuit
traversal and captures choice availability when it emits a prompt. A later host
state change does not rewrite that pending prompt's captured availability. The
adapter does not parse dialogue or own dialogue control flow.

## Immediate host requests

Create `ImmediateEffects::new(&model)` and register each supported compiled
immediate effect with `bind(&compiled_effect, "host.transition")`. The transition
must already exist in that model. Bindings match the function and exact typed
arguments; runtime request IDs remain opaque identities rather than lookup keys.

After Recite yields `DialogueEvent::Effect(request)`, call
`effects.apply(&request, &host_state)`. The receipt borrows the original request,
including its ID, arguments and full source span, and identifies the declared host
transition. Commit `receipt.into_state()` as the new host state before the next
traversal call. The existing transition rules still enforce simultaneous
assignments and reject setting writes when the model is built.

Emission alone leaves the host unchanged. Application errors leave its input state
untouched and report unmapped requests, non-immediate modes, disabled transitions,
or a typed model failure. The dialogue has already advanced past the immediate
statement; an application failure does not rewind it. Recite session rollback can
restore that position and emit the request again, but cannot undo an external host
write. The registry provides no automatic retry or duplicate-delivery suppression.

## Blocking host requests

`BlockingEffects::new(&model)` binds blocking declarations to host transitions.
After Recite emits a blocking request, call
`effects.apply_pending(&session, &host_state)` to apply the authoritative request
borrowed from the session. The receipt retains its exact identity and span. Commit
`receipt.into_state()` before calling Recite's `acknowledge_effect` with the
matching request ID and an `EffectAck`, then continue traversal separately.
Application alone leaves the conversation pending. Both `Completed` and `Failed`
acknowledgements clear that pending request and resume the same continuation;
the acknowledgement neither changes host facts nor selects a dialogue branch.

Save the host assignment alongside the Recite session. A restored session emits
its pending request once with the same ID, even if the host operation was already
applied before saving. Reconcile the saved host state before applying anything
again; a domain guard such as "item not yet granted" can prevent a repeated write.
The adapter has no delivery ledger or general exactly-once guarantee. Recite's
restore-only delivery marker is absent from session snapshots: byte-equal
snapshots do not imply equal next deliveries.

## Deferred host requests

`DeferredEffects::new(&model)` binds deferred declarations to host transitions.
Recite alone accumulates the reached requests and returns them in
`DialogueEvent::End { deferred_effects }`. Queuing them leaves host facts unchanged;
queries, later immediate requests and captured choice availability still observe
the current host assignment.

For this bridge's host policy, handle `End`, enumerate its delivered list in order,
and call `effects.apply_at(&session, position, &host_state)` for each occurrence.
The method requires an ended session and borrows that position from Recite's
retained queue. Its receipt preserves the original request and declared transition.
Commit each returned state before the next position. If application fails, stop;
prior host commits remain and no suffix is applied automatically. The adapter does
not impose order, retry, or rollback on the host's loop.

Repeated visits to one authored deferred statement can produce identical IDs and
requests at different positions. Record progress using the host's transaction
identity together with the queue position, rather than deduplicating by request ID.
The retained queue is not consumed by application. Later asynchronous scheduling
would need its own explicit consumer contract.

## Run the archive integration fixture

From a source checkout with the pinned tools installed:

```sh
mise exec -- cargo run --locked -p leitmotif-recite --example archive
```

The original conversation queues a visit, exposes an archivist's claim immediately,
then offers an unlock or leave choice. A world event can remove the key while that
prompt is still pending, before the choice is selected. The faulty host transition
trusts the captured availability; the corrected transition checks that the key is
still present. Blocking denial changes host access explicitly before the Failed
acknowledgement. Claim truth and belief are independent settings; exposure changes
neither.

The example explores actual cloned Recite sessions without deduplicating snapshots.
It checks every visited host assignment against `opening_requires_current_key`,
replays the faulty witness, and exhausts all eight initial key/truth/belief
combinations for the corrected model. Full typed boundary records include events,
query results and errors, host assignments, pending work, deferred queues and selected
line IDs. Every completed history is replayed. The terminal shows a source-linked
summary rather than dumping every record.

Exhaustion applies only to the stated finite host protocol: key removal occurs at
most once while the prompt remains pending; each blocking application permits one
restore before application and one after host commitment; restored requests are
redelivered before further host actions. Applied work is acknowledged Completed,
denial Failed. The host applies deferred occurrences in reached order after End.
The example does not explore arbitrary repeated restores, other host schedules or
unbounded histories. Node and depth limits produce an inconclusive result; runtime
failures remain error outcomes. The node budget limits evaluated tree nodes, not
memory usage.

Compilation and execution use the same pinned Recite revision as the adapter. The
conversation is Rust-authored `concat!` source checked by the real compiler, with no
second parser. To reproduce evidence, retain the fixture source, lockfile and
toolchain along with the displayed model identity, complete initial assignments,
property, protocol and limits. The runtime evidence remains experimental.
