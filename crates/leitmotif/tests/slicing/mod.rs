use std::error::Error as StdError;

use leitmotif::{
    Fact, FactHandle, Model, ModelBuilder, Predicate, ReachabilityConclusion, SearchLimits, State,
    Trace,
};

mod adversarial;
mod bounds;
mod finite_domains;
mod generated;

struct Bit<const INDEX: usize>;
impl<const INDEX: usize> Fact for Bit<INDEX> {
    type Value = bool;
    const KEY: &'static str = match INDEX {
        0 => "goal",
        1 => "gate",
        2 => "permit",
        _ => "unrelated",
    };
}
type Handle<const INDEX: usize> = FactHandle<Bit<INDEX>>;
const LIMITS: SearchLimits = SearchLimits {
    max_states: 32,
    max_depth: 32,
};

fn facts() -> Result<(ModelBuilder, Handle<0>, Handle<1>, Handle<2>, Handle<3>), leitmotif::Error> {
    let mut builder = ModelBuilder::new("slicing")?;
    let a = builder.fact::<Bit<0>>()?;
    let b = builder.fact::<Bit<1>>()?;
    let c = builder.fact::<Bit<2>>()?;
    let d = builder.fact::<Bit<3>>()?;
    Ok((builder, a, b, c, d))
}

fn replay(
    model: &Model,
    initial: &State,
    target: &Predicate,
    witness: &Trace,
) -> Result<(), Box<dyn StdError>> {
    let mut state = initial.clone();
    if model.snapshot(&state)? != witness.initial {
        return Err("witness initial differs from supplied full state".into());
    }
    for step in &witness.steps {
        state = model
            .apply(&state, &step.transition.id)?
            .ok_or("disabled full-model witness edge")?;
        if model.snapshot(&state)? != step.state {
            return Err(format!("full snapshot differs after {}", step.transition.id).into());
        }
    }
    if !model.evaluate(&state, target)? {
        return Err("witness does not reach the original target".into());
    }
    Ok(())
}

fn compare(model: &Model, initial: &State, target: &Predicate) -> Result<(), Box<dyn StdError>> {
    let full = model.reachability(initial, target, LIMITS)?;
    let sliced = model.sliced_reachability(initial, target, LIMITS)?;
    match (&full.conclusion, &sliced.conclusion) {
        (
            ReachabilityConclusion::Reached { witness: full },
            ReachabilityConclusion::Reached { witness },
        ) => {
            if full.steps.len() != witness.steps.len() {
                return Err("minimum witness lengths differ".into());
            }
            replay(model, initial, target, witness)?;
        }
        (ReachabilityConclusion::Unreachable, ReachabilityConclusion::Unreachable) => {}
        conclusions => return Err(format!("exhaustive outcomes differ: {conclusions:?}").into()),
    }
    if sliced.initial != full.initial
        || sliced.model_id != full.model_id
        || sliced.target != full.target
    {
        return Err("report lost original model, initial or target".into());
    }
    Ok(())
}
