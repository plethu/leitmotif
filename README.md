# Leitmotif

Leitmotif is a Rust library for keeping track of what can be true, and what can
happen next, in an interactive story.

I'm building it for a game about a town full of people with their own lives,
relationships, and versions of events. I'd like to be able to follow those
threads further than I can comfortably hold in my head: which choices make an
ending possible, whether someone could have learned the thing they're about to
say, and where two otherwise reasonable storylines stop making sense together.

The model lives in ordinary Rust. You define facts, possible events, and the
rules they need to respect; Leitmotif explores the resulting states and shows a
sequence of events when something goes wrong. Lies, conflicting testimony, and
unanswered questions belong in the story. The tooling needs to leave room for
them.

It's a sibling to [Recite](https://github.com/plethu/recite), which owns dialogue
authoring and traversal. Your game owns its live state. The first Recite bridge
is [planned work](https://github.com/plethu/leitmotif/milestones), and isn't
implemented yet.

I also want this to fit games made in Godot, Unity, and Rust engines such as
Bevy. Those integrations are planned; the core has no engine dependency.

This is an early prototype (`0.1.0-alpha.0`). You'll need to build from source;
the API and report format are still changing.

## A small model

```rust
use leitmotif::{Fact, ModelBuilder, ReachabilityConclusion, SearchLimits, Transition};

struct LearnedSecret;
impl Fact for LearnedSecret {
    type Value = bool;
    const KEY: &'static str = "mara.learned_secret";
}

fn main() -> Result<(), leitmotif::Error> {
    let mut builder = ModelBuilder::new("town.chapter_one")?;
    let learned = builder.fact::<LearnedSecret>()?;
    let model = builder
        .with_transition(Transition::new("mara.learn").with_effect(learned.set(true)))
        .build()?;
    let initial = model.state([learned.set(false)])?;
    let report = model.reachability(
        &initial,
        &learned.is(true),
        SearchLimits { max_states: 64, max_depth: 8 },
    )?;
    assert!(matches!(report.conclusion, ReachabilityConclusion::Reached { .. }));
    Ok(())
}
```

`fact::<F>()` works with booleans and with your own finite enums. Guards and
effects are inspectable values, so the same model can be checked and explored
without running arbitrary game code.

## What it can check

The current implementation explores finite states, checks invariants, and
answers hypotheses against explicitly supplied premises. It can show a route to
an ending, an early disclosure that violates a rule, or two assignments that
leave a mystery open. The fixtures include both mistakes and deliberate
ambiguity.

Every result belongs to the model you supplied. If exploration runs out of its
state or depth budget, the report says so; failing to find a route is only proof
that it doesn't exist when the search was complete. Hypothesis checks consider
assignments allowed by their premises, which may include states no sequence of
events can reach.

There is more to do before this can check a whole game's interconnected stories:
scoped claims and beliefs, chronology, progression traps, Recite integration,
and evidence about practical scale. The
[milestones](https://github.com/plethu/leitmotif/milestones)
keep that work separate from what already runs.

## Try it

With Rust 1.96.0 installed, run these from a checkout:

```sh
cargo run --locked -p leitmotif --example check -- town
cargo run --locked -p leitmotif --example check -- disclosure
cargo run --locked -p leitmotif --example check -- mystery --json
```

`disclosure` deliberately fails. These are Rust-authored examples; there is no
separate story language to learn. The
[fixture guide](https://github.com/plethu/leitmotif/blob/main/docs/guide.md)
covers the other scenarios, reports, and exit codes.

For development, install the pinned tools with [mise](https://mise.jdx.dev/):

```sh
mise install
mise exec -- just check
```

`just check` runs the complete gate; `mise run verify` does the same. Run `just`
to list commands, or `just quality` for formatting and repository checks. The
[contribution guide](https://github.com/plethu/leitmotif/blob/main/CONTRIBUTING.md)
explains focused checks and the issue workflow.

I'd love to hear about the stories you'd want to use this for, especially where
the model doesn't yet fit. Questions and design feedback are welcome in
[issues](https://github.com/plethu/leitmotif/issues).

[Research and prior art](https://github.com/plethu/leitmotif/blob/main/ATTRIBUTIONS.md)

Licensed under [MIT OR Apache-2.0](https://github.com/plethu/leitmotif/blob/main/LICENSE),
at your option.
