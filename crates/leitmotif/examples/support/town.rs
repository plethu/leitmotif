use leitmotif::{
    Domain, Error, Fact, Invariant, Model, ModelBuilder, Predicate, State, Transition, Value,
};

#[derive(Clone, Copy)]
enum Claim {
    None,
    Truthful,
    Pretended,
}

impl Domain for Claim {
    fn values() -> Vec<Self> {
        vec![Self::None, Self::Truthful, Self::Pretended]
    }

    fn encode(self) -> Value {
        let name = match self {
            Self::None => "none",
            Self::Truthful => "truthful",
            Self::Pretended => "pretended",
        };
        Value::Symbol(name.to_owned())
    }
}

#[derive(Clone, Copy)]
enum Ending {
    Undecided,
    Saved,
    Evacuated,
}

impl Domain for Ending {
    fn values() -> Vec<Self> {
        vec![Self::Undecided, Self::Saved, Self::Evacuated]
    }

    fn encode(self) -> Value {
        let name = match self {
            Self::Undecided => "undecided",
            Self::Saved => "saved",
            Self::Evacuated => "evacuated",
        };
        Value::Symbol(name.to_owned())
    }
}

struct MaraLearned;
impl Fact for MaraLearned {
    type Value = bool;
    const KEY: &'static str = "mara.learned";
}

struct MaraClaim;
impl Fact for MaraClaim {
    type Value = Claim;
    const KEY: &'static str = "mara.claim";
}

struct EvidenceAccepted;
impl Fact for EvidenceAccepted {
    type Value = bool;
    const KEY: &'static str = "ivo.evidence_accepted";
}

struct CouncilOpen;
impl Fact for CouncilOpen {
    type Value = bool;
    const KEY: &'static str = "council.open";
}

struct TransportSecured;
impl Fact for TransportSecured {
    type Value = bool;
    const KEY: &'static str = "transport.secured";
}

struct TownEnding;
impl Fact for TownEnding {
    type Value = Ending;
    const KEY: &'static str = "town.ending";
}

#[derive(Clone, Copy)]
/// Authored variations that exercise distinct narrative checks.
pub enum TownVariant {
    /// Both endings can be reached and disclosure is guarded.
    Valid,
    /// Accepting evidence closes the council, preventing ending A.
    EvidenceClosesCouncil,
    /// A deliberately faulty truthful disclosure lacks its learning guard.
    UnguardedDisclosure,
}

/// A composed town fixture and the properties its consumer checks.
pub struct Town {
    pub(super) model: Model,
    pub(super) initial: State,
    pub(super) ending_a: Predicate,
    pub(super) ending_d: Predicate,
    pub(super) ending_gate: Predicate,
}

/// Constructs the selected town model.
///
/// # Errors
/// Returns a model validation error if the fixture definitions are invalid.
pub fn town(variant: TownVariant) -> Result<Town, Error> {
    let mut builder = ModelBuilder::new("town")?;
    let learned = builder.fact::<MaraLearned>()?;
    let claim = builder.fact::<MaraClaim>()?;
    let evidence = builder.fact::<EvidenceAccepted>()?;
    let council = builder.fact::<CouncilOpen>()?;
    let transport = builder.fact::<TransportSecured>()?;
    let ending = builder.fact::<TownEnding>()?;
    let active = ending.is(Ending::Undecided);
    let ending_gate = evidence
        .is(true)
        .and(claim.is(Claim::Truthful))
        .and(council.is(true));
    let testimony_guard = match variant {
        TownVariant::UnguardedDisclosure => active.clone().and(council.is(true)),
        TownVariant::Valid | TownVariant::EvidenceClosesCouncil => {
            active.clone().and(council.is(true)).and(learned.is(true))
        }
    };
    let accept = Transition::new("ivo.accept_evidence")
        .with_guard(active.clone().and(council.is(true)))
        .with_effect(evidence.set(true));
    let accept = match variant {
        TownVariant::EvidenceClosesCouncil => accept.with_effect(council.set(false)),
        TownVariant::Valid | TownVariant::UnguardedDisclosure => accept,
    };
    let model = builder
        .with_transition(
            Transition::new("mara.learn")
                .with_guard(active.clone())
                .with_effect(learned.set(true)),
        )
        .with_transition(
            Transition::new("mara.testify")
                .with_guard(testimony_guard)
                .with_effect(claim.set(Claim::Truthful)),
        )
        .with_transition(
            Transition::new("mara.pretend")
                .with_guard(active.clone())
                .with_effect(claim.set(Claim::Pretended)),
        )
        .with_transition(accept)
        .with_transition(
            Transition::new("council.close")
                .with_guard(active.clone())
                .with_effect(council.set(false)),
        )
        .with_transition(
            Transition::new("transport.secure")
                .with_guard(active.clone())
                .with_effect(transport.set(true)),
        )
        .with_transition(
            Transition::new("town.save")
                .with_guard(active.clone().and(ending_gate.clone()))
                .with_effect(ending.set(Ending::Saved))
                .with_effect(council.set(false)),
        )
        .with_transition(
            Transition::new("town.evacuate")
                .with_guard(active.and(transport.is(true)))
                .with_effect(ending.set(Ending::Evacuated))
                .with_effect(council.set(false)),
        )
        .with_invariant(Invariant::new(
            "mara.truthful_requires_learning",
            claim.is(Claim::Truthful).implies(learned.is(true)),
        ))
        .build()?;
    let initial = model.state([
        learned.set(false),
        claim.set(Claim::None),
        evidence.set(false),
        council.set(true),
        transport.set(false),
        ending.set(Ending::Undecided),
    ])?;
    Ok(Town {
        model,
        initial,
        ending_a: ending.is(Ending::Saved),
        ending_d: ending.is(Ending::Evacuated),
        ending_gate,
    })
}
