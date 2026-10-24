use serde::Serialize;

use super::{NamedPredicate, PredicateDescription, SearchCoverage, Snapshot, Trace};

/// Route availability for reachable nonterminal states in a complete queried graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressionConclusion {
    /// Every reachable nonterminal has a route to a permitted terminal ending.
    /// Terminal states themselves are excluded, even when not permitted. This
    /// guarantees neither an eventual completion nor a winning strategy.
    NoTraps,
    /// Complete exploration found nonterminal states with no permitted route.
    Trap {
        /// Shortest entry to the first trapped state in deterministic BFS order.
        entry: Trace,
        /// Number of reachable nonterminal states without a permitted route.
        trapped_states: usize,
    },
    /// At least one distinct reachable state remained beyond the search bounds.
    Inconclusive,
}

/// Progression analysis relative to explicit terminal stopping boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProgressionReport {
    /// Full supplied assignment, including settings.
    pub initial: Snapshot,
    /// Experimental report format identifier.
    pub schema_version: String,
    /// Library version that produced the result.
    pub tool_version: String,
    /// Semantic model identity, independent of query and run inputs.
    pub model_id: String,
    /// Named model context.
    pub context: String,
    /// States where this query stops traversal, regardless of model successors.
    pub terminal: PredicateDescription,
    /// Exactly the allowed named completions, including intentional failures.
    pub permitted_endings: Vec<NamedPredicate>,
    /// Coverage of the queried graph; terminal states are evaluated but not expanded.
    pub coverage: SearchCoverage,
    /// A verdict is conclusive only when the queried graph was fully explored.
    pub conclusion: ProgressionConclusion,
}
