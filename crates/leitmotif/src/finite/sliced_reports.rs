use serde::Serialize;

use super::{PredicateDescription, ReachabilityConclusion, SearchCoverage, SearchLimits, Snapshot};

/// Coverage of a dependency projection, never a count of full-model states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectedCoverage {
    /// State budget counts distinct retained-fact assignments, including initial.
    /// Depth counts original transitions along the selected concrete execution.
    pub limits: SearchLimits,
    /// Distinct retained-fact assignments admitted.
    pub discovered_projections: usize,
    /// Retained-fact assignments whose target was evaluated.
    pub evaluated_projections: usize,
    /// Deepest evaluated transition distance, absent without an evaluation.
    pub deepest_evaluated: Option<usize>,
    /// All reachable projected assignments were evaluated and expanded.
    pub exhausted: bool,
    /// A distinct projection could not be admitted within the state budget.
    pub state_limit_hit: bool,
    /// A distinct projection remained beyond the depth bound.
    pub depth_limit_hit: bool,
}

impl ProjectedCoverage {
    pub(super) const fn from_search(coverage: &SearchCoverage) -> Self {
        Self {
            limits: coverage.limits,
            discovered_projections: coverage.discovered_states,
            evaluated_projections: coverage.evaluated_states,
            deepest_evaluated: coverage.deepest_evaluated,
            exhausted: coverage.exhausted,
            state_limit_hit: coverage.state_limit_hit,
            depth_limit_hit: coverage.depth_limit_hit,
        }
    }
}

/// Reachability over a query dependency projection of the original model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SlicedReachabilityReport {
    /// Complete supplied assignment, including facts omitted by the projection.
    pub initial: Snapshot,
    /// Experimental report format identifier.
    pub schema_version: String,
    /// Library version that produced the result.
    pub tool_version: String,
    /// Identity of the original full model, not a synthetic projected model.
    pub model_id: String,
    /// Named model context.
    pub context: String,
    /// Explicit requested predicate.
    pub target: PredicateDescription,
    /// Dependency-closed fact keys in original declaration order.
    pub retained_facts: Vec<String>,
    /// Selected transition IDs in original declaration order.
    pub retained_transitions: Vec<String>,
    /// Actual projected coverage and requested limits.
    pub coverage: ProjectedCoverage,
    /// Reachability verdict; any witness contains full-model snapshots and effects.
    pub conclusion: ReachabilityConclusion,
}
