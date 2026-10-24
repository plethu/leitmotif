//! Typed, explicitly finite facts and deterministic bounded analysis.
//!
//! Every state must assign every fact. Settings may vary between initial states,
//! but transitions cannot write them. Analysis is relative to one named context;
//! hypothesis classification uses only its explicitly supplied premises.

#![doc = include_str!("../../../README.md")]

mod finite;

pub use finite::{
    Assignment, ClassificationCoverage, ClassificationLimits, Domain, EXPERIMENTAL_SCHEMA_VERSION,
    Error, ErrorKind, Fact, FactHandle, FactInfo, FactKind, FactValue, HypothesisConclusion,
    HypothesisReport, Invariant, InvariantConclusion, InvariantReport, Model, ModelBuilder,
    NamedPredicate, Predicate, PredicateDescription, PredicateExpression, PredicateKind, Premise,
    ReachabilityConclusion, ReachabilityReport, SearchCoverage, SearchLimits, Snapshot, Source,
    State, Trace, TraceStep, Transition, TransitionInfo, Value,
};
