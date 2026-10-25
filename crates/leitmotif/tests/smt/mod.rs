use leitmotif::{
    Fact, FactHandle, Model, ModelBuilder, Predicate, Premise, Snapshot, State, Value,
    smt::{Conclusion, Error as SmtError, Options},
};
use std::{error::Error as StdError, time::Duration};

mod contracts;
mod generated;
#[cfg(unix)]
mod responses;

struct Bit<const INDEX: usize>;
impl<const INDEX: usize> Fact for Bit<INDEX> {
    type Value = bool;
    const KEY: &'static str = match INDEX {
        0 => "a",
        1 => "b",
        _ => "c",
    };
}
fn options() -> Result<Options, SmtError> {
    Options::new("z3", Duration::from_secs(1))
}
struct Facts {
    a: FactHandle<Bit<0>>,
    b: FactHandle<Bit<1>>,
    c: FactHandle<Bit<2>>,
}
impl Facts {
    fn declare(builder: &mut ModelBuilder) -> Result<Self, leitmotif::Error> {
        Ok(Self {
            a: builder.fact()?,
            b: builder.fact()?,
            c: builder.fact()?,
        })
    }
    fn state(&self, model: &Model, snapshot: &Snapshot) -> Result<State, Box<dyn StdError>> {
        Ok(model.state([
            self.a.set(boolean(snapshot, "a")?),
            self.b.set(boolean(snapshot, "b")?),
            self.c.set(boolean(snapshot, "c")?),
        ])?)
    }
    fn validate(
        &self,
        model: &Model,
        snapshot: &Snapshot,
        hypothesis: &Predicate,
        premises: &[Premise],
        supports: bool,
    ) -> Result<(), Box<dyn StdError>> {
        let state = self.state(model, snapshot)?;
        if snapshot.values.len() != model.facts().len() {
            return Err("incomplete returned snapshot".into());
        }
        // Finite enumeration of exactly this assignment independently checks all premises.
        let assignment_premises = [
            self.a.is(boolean(snapshot, "a")?),
            self.b.is(boolean(snapshot, "b")?),
            self.c.is(boolean(snapshot, "c")?),
        ];
        let mut constrained = premises.to_vec();
        for (index, premise) in assignment_premises.into_iter().enumerate() {
            constrained.push(Premise::new(format!("returned.{index}"), premise));
        }
        let checked = model.classify(
            &Predicate::constant(true),
            &constrained,
            leitmotif::ClassificationLimits { max_candidates: 8 },
        )?;
        if checked.conclusion != leitmotif::HypothesisConclusion::Required
            || model.evaluate(&state, hypothesis)? != supports
        {
            return Err("ordinary finite checks rejected returned assignment".into());
        }
        Ok(())
    }
}
fn boolean(snapshot: &Snapshot, name: &str) -> Result<bool, Box<dyn StdError>> {
    match snapshot.values.get(name) {
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(format!("missing Boolean value for {name}").into()),
    }
}
const fn verdict(conclusion: &Conclusion) -> Option<leitmotif::HypothesisConclusion> {
    use leitmotif::HypothesisConclusion;
    match conclusion {
        Conclusion::Required { .. } => Some(HypothesisConclusion::Required),
        Conclusion::RuledOut { .. } => Some(HypothesisConclusion::RuledOut),
        Conclusion::Possible { .. } => Some(HypothesisConclusion::Possible),
        Conclusion::Inconsistent { .. } => Some(HypothesisConclusion::Inconsistent),
        Conclusion::Incomplete { .. } => None,
    }
}
