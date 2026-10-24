use super::super::{FactInfo, NamedPredicate, PredicateDescription, Snapshot};
use serde::Serialize;
use std::path::PathBuf;

/// An actual solver check, never an enumeration-coverage claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckPurpose {
    /// Establish whether explicit premises are consistent.
    Premises,
    /// Find a supporting assignment after an opposing premise witness.
    Supporting,
    /// Find an opposing assignment after a supporting premise witness.
    Opposing,
}
/// Why a solver check did not establish satisfiability or unsatisfiability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IncompleteReason {
    /// The solver reported its soft timeout; this is not a hard process deadline.
    Timeout,
    /// Another unknown result, retaining the complete reason response.
    Unknown {
        /// Original solver reason S-expression.
        response: String,
    },
}
/// Result of a single solver check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckResult {
    /// A complete returned assignment passed ordinary finite validation.
    Sat,
    /// The solver reported unsat; this is not an independently checked proof.
    Unsat,
    /// The solver could not decide within its stated conditions.
    Incomplete {
        /// Distinct reason for incompleteness.
        reason: IncompleteReason,
    },
}
/// One check's purpose and evidence-sensitive result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Check {
    /// Premise consistency or the requested hypothesis polarity.
    pub purpose: CheckPurpose,
    /// Validated satisfiability, solver unsat, or an incomplete result.
    pub result: CheckResult,
}
/// Source-mapped dependency named in a solver unsat core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreDependency {
    /// An explicitly supplied named premise.
    Premise {
        /// Original premise with portable source metadata.
        premise: NamedPredicate,
    },
    /// A finite-domain restriction, distinct from authored premises.
    Domain {
        /// Fact declaration owning the finite domain.
        fact: FactInfo,
    },
    /// The additional hypothesis polarity used in the second check.
    Hypothesis {
        /// Whether this check required the hypothesis to hold.
        supports: bool,
    },
}
/// Solver-provided unsatisfiable subset; neither minimality nor uniqueness is claimed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Core {
    /// Source-mapped dependencies in deterministic generated-name order.
    pub dependencies: Vec<CoreDependency>,
}
/// One validated assignment retained when the opposite polarity is undecided.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "assignment", rename_all = "snake_case")]
pub enum KnownAssignment {
    /// All premises hold and the hypothesis is true.
    Supporting(Snapshot),
    /// All premises hold and the hypothesis is false.
    Opposing(Snapshot),
}
/// Classification with evidence required by each exclusive outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Conclusion {
    /// Premises are consistent and the opposite hypothesis is unsatisfiable.
    Required {
        /// Validated supporting assignment.
        witness: Snapshot,
        /// Solver core for the unsatisfiable opposite check.
        core: Core,
    },
    /// Premises are consistent and the hypothesis is unsatisfiable.
    RuledOut {
        /// Validated opposing assignment.
        counterexample: Snapshot,
        /// Solver core for the unsatisfiable hypothesis check.
        core: Core,
    },
    /// Both hypothesis polarities have validated assignments.
    Possible {
        /// Validated supporting assignment.
        witness: Snapshot,
        /// Validated opposing assignment.
        counterexample: Snapshot,
    },
    /// The explicit premises alone are unsatisfiable.
    Inconsistent {
        /// Source-mapped solver core for premise inconsistency.
        core: Core,
    },
    /// No complete classification follows; known evidence remains replayable.
    Incomplete {
        /// Distinct unknown or soft-timeout reason.
        reason: IncompleteReason,
        /// At most one validated assignment.
        known: Option<KnownAssignment>,
    },
}
/// Deployment identity used for a completed or incomplete experiment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SolverIdentity {
    /// Explicit executable; retain its pinned release and checksum for reproduction.
    pub executable: PathBuf,
    /// Complete SMT-LIB version response from the running solver.
    pub version_response: String,
    /// Positive per-check soft timeout in milliseconds.
    pub soft_timeout_ms: u32,
}
/// Experimental SMT classification report, separate from finite enumeration coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Experimental report schema.
    pub schema_version: String,
    /// Leitmotif version.
    pub tool_version: String,
    /// Original finite model identity.
    pub model_id: String,
    /// Original named context.
    pub context: String,
    /// Validated requested hypothesis.
    pub hypothesis: PredicateDescription,
    /// Exactly the named premises supplied for this call.
    pub premises: Vec<NamedPredicate>,
    /// Actual executable, version response and soft limit.
    pub solver: SolverIdentity,
    /// At most two checks, with no assignment-enumeration counts inferred.
    pub checks: Vec<Check>,
    /// Classification and the evidence its outcome requires.
    pub conclusion: Conclusion,
}
