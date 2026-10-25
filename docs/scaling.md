# Measuring towns

Run from a checkout on Linux with `/proc` mounted:

```sh
mise exec -- just town-scaling --threads 8 --local-domain 3 --repeat 3
```

The command builds the release example once, then runs each repetition in a
fresh process. Each stdout line is a JSON record. Redirect stdout to keep the
measurements; build messages go to stderr. The default query asks for an ending
that is closed by an immutable setting, forcing the explorer to search the
reachable graph or exhaust its budget.

```sh
mise exec -- just town-scaling --threads 8 --local-domain 3 > town-results.jsonl
mise exec -- just town-scaling --threads 8 --local-domain 3 --query goal
mise exec -- just town-scaling --threads 12 --local-domain 3 --max-states 100000
mise exec -- just town-scaling --threads 12 --local-domain 3 --coupling 11
mise exec -- just town-scaling --threads 12 --local-domain 3 --resource single-permit
```

Each thread cycles from idle (`0`) through its local phases and back to idle.
`--threads` accepts 1–16, and `--local-domain` accepts 2–8. These are genuinely
finite domains, chosen from an authored set of typed declarations; the fixture
does not change the library's fact API. A thread's `--coupling` following
neighbors must all be idle before it leaves idle. Coupling accepts zero through
one less than the thread count. `single-permit` additionally lets only one
thread be active at a time; the permit returns when that thread cycles to idle.

With no coupling and unlimited resources, the reachable graph contains
`local_domain ^ threads` states. Full coupling or a single permit reduces it to
`1 + threads * (local_domain - 1)`. The `goal` query requires every thread in its
final phase simultaneously. It has a shortest witness of
`threads * (local_domain - 1)` transitions in the independent case, and is
unreachable for multiple threads under full coupling or a single permit.

Records include the parameters, requested bounds, discovered and evaluated
states, deepest evaluated distance, conclusion, exhaustion and limit flags,
and witness length when reached. `analysis_seconds` covers the reachability
call; `process_seconds` also covers process startup, fixture construction and
JSON output. `peak_rss_bytes` comes from the executable's Linux `VmHWM` after
analysis, before JSON rendering. It includes fixture construction and search
memory, and excludes the Python launcher and build processes. Machine,
Rust/Cargo/Python versions, release build flags and source revision accompany
every repetition; `source.dirty` identifies an uncommitted checkout.

An exhausted graph and an inconclusive run answer different questions. A
state or depth limit can leave reachable states unexplored; use the limit flags
and `exhausted`, rather than interpreting a missing witness as unreachability.
A reached goal provides a witness even when exploration is incomplete. The
small CI cases check exact state counts, permit reuse, cycles and replayed
witnesses, including an exact 729-state admission/evaluation budget. CI does not
assert wall-clock timing.

These measurements describe this explicit-state explorer on a synthetic family
and the recorded machine. They do not predict the size of an authored game from
its play time, or establish performance for arbitrary predicates and models.
Compare repeated runs with matching parameters, bounds, toolchain and build
flags. The command reports observations and imposes no runtime or memory
ceiling, so choose bounds suitable for the machine.

To measure the original harbour stories, use the same runner:

```sh
mise exec -- just town-scaling --fixture composed --query unreachable --repeat 3
mise exec -- just town-scaling --fixture composed --query goal
mise exec -- just town-scaling --fixture composed --query alternative
```

The keeper shares a storm warning, enabling the ferryman to prepare a departure.
The last oil can fuel the ferry or light the keeper's beacon. Spending it removes
the other story's action. Shelter and departure are exclusive endings. The
character modules receive shared typed inputs from one composition root; they
register local facts and author their events independently. The existing
`ModelBuilder` validates their combined declarations, including conflicting keys.
A duplicate diagnostic retains both the original and offending author locations.

For this fixture, `goal` means shelter and `alternative` means departure.
`unreachable` asks whether the immutable storm setting has changed, forcing full
graph exploration. Explicit synthetic dimension arguments are rejected for the composed
fixture; composed records have `parameters: null`. `authored_events` counts the
named transition declarations (five here), while `coverage.discovered_states`
counts complete reachable world assignments (six here). These are different
graphs: a short list of authored events can induce many reachable combinations.
The event count does not count playable hours or reachable paths. The composed
fixture's fixed cost regression exhausts exactly six states at depth three;
both ending witnesses take three events.

## Optional symbolic comparison

This `publish = false` experiment runs from this checkout. Its workspace pins only
`oxidd-derive` to a release-compatible diagnostics backport in
[our OxiDD fork](https://github.com/plethu/oxidd/commit/c211b11c3beae9c1e35fa16b7b4871992134404a);
the remaining OxiDD packages use registry release 0.13.0. Cargo does not inherit
a dependency workspace's patch, so this is not a supported downstream library
feature. Remove the patch when a compatible upstream release passes the dependency
policy and focused semantic and resource checks. Reassess if the patch expands
beyond derive diagnostics. Production backend adoption remains deferred.

Build the private OxiDD experiment explicitly:

```sh
mise exec -- cargo build --release --locked -p leitmotif \
  --features symbolic-experiment --example symbolic_town
mise exec -- python scripts/measure_symbolic.py \
  --binary target/release/examples/symbolic_town \
  --backend oxidd --family sparse --threads 4 --local-domain 3 --query unreachable
mise exec -- python scripts/measure_symbolic.py \
  --binary target/release/examples/symbolic_town \
  --backend explicit --family coupled --threads 4 --local-domain 3 --query goal
```

Use `--order paired` or `--order banked` with the same fixture and query to
compare current/next variable placement. Each finite fact uses the minimum
number of binary bits needed for its declared domain. Both banks exclude unused
codes, including the fourth code of a three-value domain. Paired order places
each current bit beside its next bit; banked order places all current bits
before all next bits. Transition relations preserve every unaffected fact,
including immutable settings. Invariants are observations over reached states,
never assumptions that remove violating states. Every recovered witness is
replayed through `Model.apply` and checked against complete reference snapshots.

`--node-capacity` bounds the index manager's inner-node capacity; exhaustion returns an
allocation error. This is separate from process memory: the wrapper's `--memory-mib`
bounds Linux address space, and `--timeout` bounds wall time and CPU seconds. The
wrapper uses an 8 MiB OxiDD worker stack instead of its upstream 1 GiB default;
`OXIDD_STACK_SIZE` overrides this in bytes and the effective value is recorded.
Resource-limit setup errors produce a separate `setup_failure` record, including
when an inherited hard limit prevents the requested cap. Timeout,
signal, allocation failure and inconclusive depth bounds never establish unreachability.
The wrapper starts a fresh worker for each repetition, records the binary and lockfile
hashes, source revision, toolchain, process time and whole-child peak RSS.
`analysis_seconds` includes the BDD manager and variable construction for OxiDD, or the
reference reachability call for explicit exploration. Function node counts include
terminals; the separate manager count includes allocated inner nodes shared across
functions. These counts are different from complete explicit state assignments.

The [comparison recorded in issue #17](https://github.com/plethu/leitmotif/issues/17)
agreed with explicit exploration across 180 earlier bounded measurements.
Symbolic representation helped the independent family; explicit exploration was
faster on the tightly coupled family. Those historical runs included an earlier
comparison adapter. They are evidence for retaining this optional experiment,
not a new benchmark of this implementation or a reason to adopt a production
BDD backend. The small executable contract cases check both variable orders,
nonbinary domains, fixed-point exhaustion, truncation, allocation failure and
reference witness replay. No conclusion about arbitrary authored games follows
from compression on these synthetic families.
