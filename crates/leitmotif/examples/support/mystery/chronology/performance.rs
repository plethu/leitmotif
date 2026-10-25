//! Learning and audience revelation concern historical occurrences independently.
use leitmotif::{
    Error, Fact, FactHandle, Invariant, Model, ModelBuilder, SearchLimits, State, Transition,
};

pub(in super::super) const LIMITS: SearchLimits = SearchLimits {
    max_states: 128,
    max_depth: 7,
};

struct Conversation;
impl Fact for Conversation {
    type Value = bool;
    const KEY: &'static str = "execution.conversation";
}
struct OrinKnowsA;
impl Fact for OrinKnowsA {
    type Value = bool;
    const KEY: &'static str = "orin.knows.lock_a";
}
struct OrinKnowsB;
impl Fact for OrinKnowsB {
    type Value = bool;
    const KEY: &'static str = "orin.knows.lock_b";
}
struct AudienceSawA;
impl Fact for AudienceSawA {
    type Value = bool;
    const KEY: &'static str = "audience.saw.lock_a";
}
struct AudienceSawB;
impl Fact for AudienceSawB {
    type Value = bool;
    const KEY: &'static str = "audience.saw.lock_b";
}
struct DisclosedA;
impl Fact for DisclosedA {
    type Value = bool;
    const KEY: &'static str = "orin.disclosed.lock_a";
}
struct DisclosedB;
impl Fact for DisclosedB {
    type Value = bool;
    const KEY: &'static str = "orin.disclosed.lock_b";
}
#[derive(Clone, Copy)]
pub(in super::super) enum DisclosureRule {
    Unchecked,
    RequiresAcquisition,
}
pub(in super::super) struct Performance {
    pub(in super::super) model: Model,
    pub(in super::super) initial: State,
}
impl Performance {
    pub(in super::super) fn new(rule: DisclosureRule) -> Result<Self, Error> {
        let mut builder = ModelBuilder::new("tide_plate.chronology")?;
        let conversation = builder.fact::<Conversation>()?;
        let known_a = builder.fact::<OrinKnowsA>()?;
        let known_b = builder.fact::<OrinKnowsB>()?;
        let seen_a = builder.fact::<AudienceSawA>()?;
        let seen_b = builder.fact::<AudienceSawB>()?;
        let disclosed_a = builder.fact::<DisclosedA>()?;
        let disclosed_b = builder.fact::<DisclosedB>()?;
        builder = builder.with_transition(
            Transition::new("conversation")
                .with_guard(conversation.is(false))
                .with_effect(conversation.set(true)),
        );
        builder = one_shot(builder, "acquire_lock_a", &conversation, &known_a);
        builder = one_shot(builder, "acquire_lock_b", &conversation, &known_b);
        builder = one_shot(builder, "flashback_lock_a", &conversation, &seen_a);
        builder = one_shot(builder, "flashback_lock_b", &conversation, &seen_b);
        builder = disclose(
            builder,
            "disclose_lock_a",
            &conversation,
            &known_a,
            &disclosed_a,
            rule,
        );
        builder = disclose(
            builder,
            "disclose_lock_b",
            &conversation,
            &known_b,
            &disclosed_b,
            rule,
        );
        // This is an authored license for these disclosures, not general epistemic inference.
        let model = builder
            .with_invariant(Invariant::new(
                "lock_a_disclosure_requires_acquisition",
                disclosed_a.is(true).implies(known_a.is(true)),
            ))
            .with_invariant(Invariant::new(
                "lock_b_disclosure_requires_acquisition",
                disclosed_b.is(true).implies(known_b.is(true)),
            ))
            .build()?;
        let initial = model.state([
            conversation.set(false),
            known_a.set(false),
            known_b.set(false),
            seen_a.set(false),
            seen_b.set(false),
            disclosed_a.set(false),
            disclosed_b.set(false),
        ])?;
        Ok(Self { model, initial })
    }
}
// All four acquisition/reveal actions are one-shot changes of their own fact only.
#[track_caller]
fn one_shot<F: Fact<Value = bool>>(
    builder: ModelBuilder,
    id: &str,
    conversation: &FactHandle<Conversation>,
    fact: &FactHandle<F>,
) -> ModelBuilder {
    builder.with_transition(
        Transition::new(id)
            .with_guard(conversation.is(true).and(fact.is(false)))
            .with_effect(fact.set(true)),
    )
}
#[track_caller]
fn disclose<K: Fact<Value = bool>, D: Fact<Value = bool>>(
    builder: ModelBuilder,
    id: &str,
    conversation: &FactHandle<Conversation>,
    known: &FactHandle<K>,
    disclosed: &FactHandle<D>,
    rule: DisclosureRule,
) -> ModelBuilder {
    let mut guard = conversation.is(true).and(disclosed.is(false));
    if matches!(rule, DisclosureRule::RequiresAcquisition) {
        guard = guard.and(known.is(true));
    }
    builder.with_transition(
        Transition::new(id)
            .with_guard(guard)
            .with_effect(disclosed.set(true)),
    )
}
