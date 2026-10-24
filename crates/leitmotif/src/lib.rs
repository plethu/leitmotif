//! Typed, explicitly finite facts and deterministic bounded analysis.
//!
//! Every state must assign every fact. Settings may vary between initial states,
//! but transitions cannot write them. Analysis is relative to one named context;
//! hypothesis classification uses only its explicitly supplied premises.

//! # Example
//!
//! ```rust
//! use leitmotif::{Fact, ModelBuilder, ReachabilityConclusion, SearchLimits, Transition};
//!
//! struct LearnedSecret;
//! impl Fact for LearnedSecret {
//!     type Value = bool;
//!     const KEY: &'static str = "mara.learned_secret";
//! }
//!
//! fn main() -> Result<(), leitmotif::Error> {
//!     let mut builder = ModelBuilder::new("town.chapter_one")?;
//!     let learned = builder.fact::<LearnedSecret>()?;
//!     let model = builder
//!         .with_transition(Transition::new("mara.learn").with_effect(learned.set(true)))
//!         .build()?;
//!     let initial = model.state([learned.set(false)])?;
//!     let report = model.reachability(
//!         &initial,
//!         &learned.is(true),
//!         SearchLimits { max_states: 64, max_depth: 8 },
//!     )?;
//!     assert!(matches!(report.conclusion, ReachabilityConclusion::Reached { .. }));
//!     Ok(())
//! }
//! ```

mod finite;

#[cfg(feature = "smt")]
pub use finite::smt;

pub use finite::{
    Assignment, ClassificationCoverage, ClassificationLimits, Domain, EXPERIMENTAL_SCHEMA_VERSION,
    Error, ErrorKind, Fact, FactHandle, FactInfo, FactKind, FactValue, HypothesisConclusion,
    HypothesisReport, Invariant, InvariantConclusion, InvariantReport, Model, ModelBuilder,
    NamedPredicate, PermittedEnding, Predicate, PredicateDescription, PredicateExpression,
    PredicateKind, Premise, ProgressionConclusion, ProgressionQuery, ProgressionReport,
    ProjectedCoverage, ReachabilityConclusion, ReachabilityReport, SearchCoverage, SearchLimits,
    SlicedReachabilityReport, Snapshot, Source, State, Trace, TraceStep, Transition,
    TransitionInfo, Value,
};
