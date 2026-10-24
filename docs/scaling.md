# Measuring synthetic towns

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
