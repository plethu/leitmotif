use leitmotif::{Domain, Error, Fact, FactKind, Model, ModelBuilder, Predicate, Premise, Value};

#[derive(Clone, Copy)]
enum Entrant {
    Ada,
    Bo,
}
impl Domain for Entrant {
    fn values() -> Vec<Self> {
        vec![Self::Ada, Self::Bo]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Ada => "ada",
                Self::Bo => "bo",
            }
            .to_owned(),
        )
    }
}

#[derive(Clone, Copy)]
enum Method {
    Key,
    Passage,
}
impl Domain for Method {
    fn values() -> Vec<Self> {
        vec![Self::Key, Self::Passage]
    }
    fn encode(self) -> Value {
        Value::Symbol(
            match self {
                Self::Key => "key",
                Self::Passage => "passage",
            }
            .to_owned(),
        )
    }
}

struct StudyEntrant;
impl Fact for StudyEntrant {
    type Value = Entrant;
    const KEY: &'static str = "study.entrant";
    const KIND: FactKind = FactKind::Setting;
}
struct EntryMethod;
impl Fact for EntryMethod {
    type Value = Method;
    const KEY: &'static str = "study.entry_method";
    const KIND: FactKind = FactKind::Setting;
}

#[derive(Clone, Copy)]
/// Evidence available at different authored reveal points.
pub enum MysteryEvidence {
    /// A passage remains possible, preserving two possible entrants.
    Partial,
    /// Absence of a passage excludes Bo.
    Complete,
    /// The final premise conflicts with the earlier evidence.
    Contradictory,
}

/// A finite scenario and exactly the evidence its observer can use.
pub struct Mystery {
    pub(super) model: Model,
    pub(super) hypothesis: Predicate,
    pub(super) premises: Vec<Premise>,
}

/// Constructs one evidence-scoped mystery query.
///
/// # Errors
/// Returns a model validation error if the fixture declarations are invalid.
pub fn mystery(evidence: MysteryEvidence) -> Result<Mystery, Error> {
    let mut builder = ModelBuilder::new("study.one_scenario")?;
    let entrant = builder.fact::<StudyEntrant>()?;
    let method = builder.fact::<EntryMethod>()?;
    let mut premises = vec![Premise::new(
        "only_ada_had_key",
        method.is(Method::Key).implies(entrant.is(Entrant::Ada)),
    )];
    match evidence {
        MysteryEvidence::Partial => {}
        MysteryEvidence::Complete => {
            premises.push(Premise::new("no_passage", method.is(Method::Key)));
        }
        MysteryEvidence::Contradictory => {
            premises.push(Premise::new("no_passage", method.is(Method::Key)));
            premises.push(Premise::new("ada_did_not_enter", entrant.is(Entrant::Bo)));
        }
    }
    Ok(Mystery {
        model: builder.build()?,
        hypothesis: entrant.is(Entrant::Bo),
        premises,
    })
}
