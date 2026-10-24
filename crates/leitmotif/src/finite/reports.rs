use std::collections::BTreeMap;

use serde::Serialize;

mod progression;
pub use progression::{ProgressionConclusion, ProgressionReport};

use super::{Source, Value};

/// Experimental report format; no stable persistence contract is promised yet.
pub const EXPERIMENTAL_SCHEMA_VERSION: &str = "leitmotif.experimental.v2";

/// Deterministic BFS bounds. Zero states permits no state inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SearchLimits {
    /// Maximum distinct states admitted, including the initial state.
    pub max_states: usize,
    /// Maximum number of transitions from the initial state.
    pub max_depth: usize,
}

/// Finite enumeration budget, independent of reachable states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ClassificationLimits {
    /// Maximum complete candidate assignments evaluated, including rejected ones.
    pub max_candidates: usize,
}

/// What BFS actually covered within its stated bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SearchCoverage {
    /// Requested deterministic bounds.
    pub limits: SearchLimits,
    /// Distinct states admitted to the search.
    pub discovered_states: usize,
    /// States whose query or invariants were evaluated.
    pub evaluated_states: usize,
    /// Deepest evaluated distance, absent if no state was evaluated.
    pub deepest_evaluated: Option<usize>,
    /// True only when the query's reachable state space is exhausted.
    pub exhausted: bool,
    /// A distinct successor could not be admitted within the state budget.
    pub state_limit_hit: bool,
    /// A distinct successor remained beyond the depth bound.
    pub depth_limit_hit: bool,
}

/// What explicit-premise enumeration actually covered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ClassificationCoverage {
    /// Requested candidate budget.
    pub limits: ClassificationLimits,
    /// Complete assignments evaluated, whether or not premises held.
    pub evaluated_candidates: usize,
    /// Evaluated candidates satisfying every supplied premise.
    pub satisfying_candidates: usize,
    /// True only when the finite Cartesian product was exhausted.
    pub exhausted: bool,
}

/// Serializable symbolic predicate including authoring locations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PredicateDescription {
    /// Operation with stable fact keys.
    pub expression: PredicateExpression,
    /// Source of this operation.
    pub provenance: Source,
}

/// Inspectable operations in report predicates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "snake_case")]
pub enum PredicateExpression {
    /// Literal truth value.
    Constant(bool),
    /// Equality with a symbolic fact value.
    Eq(FactValue),
    /// Conjunction.
    And(Box<PredicateDescription>, Box<PredicateDescription>),
    /// Disjunction.
    Or(Box<PredicateDescription>, Box<PredicateDescription>),
    /// Negation.
    Not(Box<PredicateDescription>),
}

/// Stable fact key, concrete value and authoring location of an equality/effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FactValue {
    /// Stable fact key.
    pub fact: String,
    /// Concrete finite value.
    pub value: Value,
    /// Equality or assignment source.
    pub provenance: Source,
}

/// A named premise or invariant in a report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamedPredicate {
    /// Stable declaration ID.
    pub id: String,
    /// Declared property.
    pub predicate: PredicateDescription,
    /// Declaration source.
    pub provenance: Source,
}

/// Source-linked transition definition included in replayable traces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TransitionInfo {
    /// Stable transition ID.
    pub id: String,
    /// Guard evaluated on the pre-state.
    pub guard: PredicateDescription,
    /// Simultaneous assignments.
    pub effects: Vec<FactValue>,
    /// Transition declaration source.
    pub provenance: Source,
}

/// A full assignment keyed by stable fact IDs, in deterministic order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Every fact, including settings.
    pub values: BTreeMap<String, Value>,
}

/// One transition and the complete state after its effects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TraceStep {
    /// Source-linked transition evaluated against the preceding snapshot.
    pub transition: TransitionInfo,
    /// Full post-state.
    pub state: Snapshot,
}

/// A shortest transition-count witness within admitted exploration.
///
/// Global shortest-path claims require coverage without state-budget truncation.
/// A witness remains valid when coverage is incomplete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Trace {
    /// Full supplied initial assignment.
    pub initial: Snapshot,
    /// Transitions in execution order.
    pub steps: Vec<TraceStep>,
}

/// Reachability distinguishes evidence, exhaustion, and incomplete exploration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReachabilityConclusion {
    /// A shortest witness within admitted exploration reaches the target.
    Reached {
        /// Replayable witness.
        witness: Trace,
    },
    /// Exhaustive BFS found no target state.
    Unreachable,
    /// The bounds left at least one distinct reachable state unexplored.
    Inconclusive,
}

/// Reachability result with query, provenance and deterministic coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReachabilityReport {
    /// Complete supplied assignment, including settings, even without a witness.
    pub initial: Snapshot,
    /// Experimental report format identifier.
    pub schema_version: String,
    /// Library version that produced the result.
    pub tool_version: String,
    /// SHA-256 of the ordered semantic model; excludes provenance and run inputs.
    pub model_id: String,
    /// Named model context.
    pub context: String,
    /// Explicit requested predicate.
    pub target: PredicateDescription,
    /// Actual search coverage and requested budget.
    pub coverage: SearchCoverage,
    /// Evidence-sensitive conclusion.
    pub conclusion: ReachabilityConclusion,
}

/// Reachable invariant verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InvariantConclusion {
    /// Every declared invariant holds over all reachable states.
    Holds,
    /// A shortest trace within admitted exploration reaches a violated invariant.
    Violation {
        /// Source-linked violated declaration.
        invariant: Box<NamedPredicate>,
        /// Replayable witness; consult coverage before claiming global shortestness.
        witness: Trace,
    },
    /// No violation was seen, but exploration was incomplete.
    Inconclusive,
}

/// Invariant verification report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvariantReport {
    /// Complete supplied assignment, including settings, even without a witness.
    pub initial: Snapshot,
    /// Experimental report format identifier.
    pub schema_version: String,
    /// Library version that produced the result.
    pub tool_version: String,
    /// SHA-256 of the ordered semantic model; excludes provenance and run inputs.
    pub model_id: String,
    /// Named model context.
    pub context: String,
    /// All explicitly declared invariants, in authoring order.
    pub invariants: Vec<NamedPredicate>,
    /// Actual coverage and requested budget.
    pub coverage: SearchCoverage,
    /// Evidence-sensitive conclusion.
    pub conclusion: InvariantConclusion,
}

/// Classification relative only to the explicit premises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisConclusion {
    /// Premises admit both hypothesis and counterexample, or a hypothesis
    /// witness exists while incomplete enumeration prevents proving necessity.
    Possible,
    /// Premises are consistent and every satisfying assignment supports it.
    Required,
    /// Premises are consistent and every satisfying assignment refutes it.
    RuledOut,
    /// No assignment satisfies all premises; no vacuous necessity is reported.
    Inconsistent,
    /// Incomplete enumeration found no supporting hypothesis witness.
    Inconclusive,
}

/// Finite hypothesis classification, independent of transitions and invariants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HypothesisReport {
    /// Experimental report format identifier.
    pub schema_version: String,
    /// Library version that produced the result.
    pub tool_version: String,
    /// SHA-256 of the ordered semantic model; excludes provenance and run inputs.
    pub model_id: String,
    /// Named model context.
    pub context: String,
    /// Explicit hypothesis.
    pub hypothesis: PredicateDescription,
    /// Exactly the premises supplied for this call.
    pub premises: Vec<NamedPredicate>,
    /// Enumeration coverage and requested budget.
    pub coverage: ClassificationCoverage,
    /// Classification after checking premise consistency.
    pub conclusion: HypothesisConclusion,
    /// First satisfying assignment supporting the hypothesis, if found.
    pub witness: Option<Snapshot>,
    /// First satisfying assignment refuting the hypothesis, if found.
    pub counterexample: Option<Snapshot>,
}
