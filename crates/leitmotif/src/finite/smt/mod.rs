//! Experimental optional Z3 classification of the existing finite predicate language.
//!
//! Enable the `smt` feature and supply an explicitly selected Z3 executable.
//! Finite enumeration remains available without the feature or any solver.
//! Unsat results trust the solver; returned assignments are independently checked.
//! Soft per-check timeouts do not bound startup, protocol I/O, model/core retrieval
//! or synchronous cleanup. No hard process deadline or cancellation is promised.
//! This experiment trusts the pinned Z3's ordinary ASCII protocol responses.
//! Upstream rsmt2 can panic on malformed non-ASCII check, model or core replies;
//! general solver deployment remains outside this experiment's supported use.

mod decoder;
mod encoding;
mod error;
mod info;
mod options;
mod reports;
mod session;

pub use error::Error;
pub use options::Options;
pub use reports::{
    Check, CheckPurpose, CheckResult, Conclusion, Core, CoreDependency, IncompleteReason,
    KnownAssignment, Report, SolverIdentity,
};

use super::{EXPERIMENTAL_SCHEMA_VERSION, Model, Predicate, Premise};
use session::{Answer, Session};

impl Model {
    /// Classifies under exactly the supplied premises using the optional Z3 experiment.
    ///
    /// Every fact is restricted to its declared finite domain. Transitions and
    /// invariants are not premises. Every returned assignment is complete and
    /// checked by the ordinary domain, premise and hypothesis evaluators.
    /// At most two solver checks distinguish consistent polarities from inconsistent
    /// premises. Solver unsat cores are source-mapped, not claimed minimal or proven
    /// independently. Solvers may choose different valid assignments or cores.
    ///
    /// # Errors
    /// Fully validates the query before starting the executable. Reports deployment,
    /// unsupported commands, protocol failures and invalid returned evidence distinctly.
    /// Unknown and soft-timeout results retain only the evidence already validated.
    ///
    /// # Panics
    /// Upstream rsmt2 can panic on malformed non-ASCII check, assignment or core
    /// responses. This experiment requires the pinned Z3's ordinary ASCII replies;
    /// its optional feature does not harden the upstream parser.
    #[track_caller]
    pub fn classify_smt(
        &self,
        hypothesis: &Predicate,
        premises: &[Premise],
        options: &Options,
    ) -> Result<Report, Error> {
        let query = self.prepare_classification(hypothesis, premises)?;
        let mut session = Session::new(self, &query, options)?;
        let first = session.check(None)?;
        let mut checks = vec![Check {
            purpose: CheckPurpose::Premises,
            result: first.result(),
        }];
        let conclusion = self.smt_conclusion(&mut session, first, &mut checks)?;
        session.finish()?;
        let solver = session.identity.clone();
        Ok(Report {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.into(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            model_id: self.fingerprint.clone(),
            context: self.context.clone(),
            hypothesis: query.hypothesis_description,
            premises: query.descriptions,
            solver,
            checks,
            conclusion,
        })
    }
    fn smt_conclusion(
        &self,
        session: &mut Session<'_>,
        first: Answer,
        checks: &mut Vec<Check>,
    ) -> Result<Conclusion, Error> {
        let (first_values, supports) = match first {
            Answer::Unsat(core) => return Ok(Conclusion::Inconsistent { core }),
            Answer::Incomplete(reason) => {
                return Ok(Conclusion::Incomplete {
                    reason,
                    known: None,
                });
            }
            Answer::Sat { values, supports } => (values, supports),
        };
        let second = session.check(Some(!supports))?;
        checks.push(Check {
            purpose: if supports {
                CheckPurpose::Opposing
            } else {
                CheckPurpose::Supporting
            },
            result: second.result(),
        });
        let first = self.snapshot_values(&first_values);
        Ok(match second {
            Answer::Unsat(core) if supports => Conclusion::Required {
                witness: first,
                core,
            },
            Answer::Unsat(core) => Conclusion::RuledOut {
                counterexample: first,
                core,
            },
            Answer::Incomplete(reason) => Conclusion::Incomplete {
                reason,
                known: Some(if supports {
                    KnownAssignment::Supporting(first)
                } else {
                    KnownAssignment::Opposing(first)
                }),
            },
            Answer::Sat { values, .. } if supports => Conclusion::Possible {
                witness: first,
                counterexample: self.snapshot_values(&values),
            },
            Answer::Sat { values, .. } => Conclusion::Possible {
                witness: self.snapshot_values(&values),
                counterexample: first,
            },
        })
    }
}
