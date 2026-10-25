use super::{
    super::{Model, Value, classify::ClassificationQuery},
    CheckResult, Core, CoreDependency, Error, IncompleteReason, Options, SolverIdentity, decoder,
    encoding::Expression,
    error::protocol,
    info,
};
use rsmt2::{SmtConf, Solver, errors::ErrorKind as SolverErrorKind};
use std::collections::{BTreeMap, BTreeSet};

pub(super) enum Answer {
    Sat { values: Vec<Value>, supports: bool },
    Unsat(Core),
    Incomplete(IncompleteReason),
}
pub(super) struct Session<'a> {
    model: &'a Model,
    query: &'a ClassificationQuery,
    solver: Solver<()>,
    core_names: BTreeMap<String, CoreDependency>,
    pub(super) identity: SolverIdentity,
}
impl<'a> Session<'a> {
    pub(super) fn new(
        model: &'a Model,
        query: &'a ClassificationQuery,
        options: &Options,
    ) -> Result<Self, Error> {
        let mut config = SmtConf::z3(options.command());
        config.check_success();
        config.models();
        config.unsat_cores();
        config.option(format!("-t:{}", options.soft_timeout_ms()));
        let mut solver = Solver::new(config, ()).map_err(|source| {
            if matches!(source.kind(), SolverErrorKind::Unsupported) {
                protocol("solver initialization", source)
            } else {
                Error::Deployment {
                    executable: options.executable().to_path_buf(),
                    source: Box::new(source),
                }
            }
        })?;
        let version_response = info::get(&mut solver, ":version")?.raw;
        let mut session = Self {
            model,
            query,
            solver,
            core_names: BTreeMap::new(),
            identity: SolverIdentity {
                executable: options.executable().to_path_buf(),
                version_response,
                soft_timeout_ms: options.soft_timeout_ms(),
            },
        };
        session.declare()?;
        Ok(session)
    }
    fn declare(&mut self) -> Result<(), Error> {
        for (index, fact) in self.model.facts.iter().enumerate() {
            let name = format!("f{index}");
            self.solver
                .declare_const(&name, "Int")
                .map_err(|source| protocol("fact declaration", source))?;
            let domain_name = format!("d{index}");
            let bounds = format!("(and (<= 0 {name}) (< {name} {}))", fact.domain.len());
            self.solver
                .named_assert(&domain_name, bounds)
                .map_err(|source| protocol("finite domain", source))?;
            self.core_names
                .insert(domain_name, CoreDependency::Domain { fact: fact.clone() });
        }
        for (index, (predicate, premise)) in self
            .query
            .premises
            .iter()
            .zip(&self.query.descriptions)
            .enumerate()
        {
            let name = format!("p{index}");
            self.solver
                .named_assert(
                    &name,
                    Expression {
                        model: self.model,
                        predicate,
                        negate: false,
                    },
                )
                .map_err(|source| protocol("premise assertion", source))?;
            self.core_names.insert(
                name,
                CoreDependency::Premise {
                    premise: premise.clone(),
                },
            );
        }
        Ok(())
    }
    pub(super) fn check(&mut self, supports: Option<bool>) -> Result<Answer, Error> {
        if let Some(supports) = supports {
            self.solver
                .named_assert(
                    "h",
                    Expression {
                        model: self.model,
                        predicate: &self.query.hypothesis,
                        negate: !supports,
                    },
                )
                .map_err(|source| protocol("hypothesis assertion", source))?;
            self.core_names
                .insert("h".into(), CoreDependency::Hypothesis { supports });
        }
        match self.solver.check_sat_or_unk() {
            Ok(Some(true)) => self.assignment(supports),
            Ok(Some(false)) => Ok(Answer::Unsat(self.core()?)),
            Ok(None) => {
                let response = info::get(&mut self.solver, ":reason-unknown")?;
                Ok(Answer::Incomplete(if response.value == "timeout" {
                    IncompleteReason::Timeout
                } else {
                    IncompleteReason::Unknown {
                        response: response.raw,
                    }
                }))
            }
            Err(source) if matches!(source.kind(), SolverErrorKind::Timeout) => {
                Ok(Answer::Incomplete(IncompleteReason::Timeout))
            }
            Err(source) => Err(protocol("check-sat", source)),
        }
    }
    fn assignment(&mut self, supports: Option<bool>) -> Result<Answer, Error> {
        let returned = if self.model.facts.is_empty() {
            Vec::new()
        } else {
            let names = self
                .model
                .facts
                .iter()
                .enumerate()
                .map(|(index, _)| format!("f{index}"));
            self.solver
                .get_values(names)
                .map_err(|source| protocol("assignment retrieval", source))?
        };
        let (values, supports) = decoder::assignment(self.model, self.query, returned, supports)?;
        Ok(Answer::Sat { values, supports })
    }
    fn core(&mut self) -> Result<Core, Error> {
        let returned: Vec<String> = self
            .solver
            .get_unsat_core()
            .map_err(|source| protocol("unsat core", source))?;
        let mut names = BTreeSet::new();
        for name in returned {
            if !self.core_names.contains_key(&name) || !names.insert(name.clone()) {
                return Err(Error::InvalidCore(name));
            }
        }
        Ok(Core {
            dependencies: self
                .core_names
                .iter()
                .filter(|(name, _)| names.contains(*name))
                .map(|(_, dependency)| dependency.clone())
                .collect(),
        })
    }
    pub(super) fn finish(&mut self) -> Result<(), Error> {
        self.solver
            .kill()
            .map_err(|source| protocol("solver cleanup", source))
    }
}
impl Answer {
    pub(super) fn result(&self) -> CheckResult {
        match self {
            Self::Sat { .. } => CheckResult::Sat,
            Self::Unsat(_) => CheckResult::Unsat,
            Self::Incomplete(reason) => CheckResult::Incomplete {
                reason: reason.clone(),
            },
        }
    }
}
