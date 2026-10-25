use super::{
    EXPERIMENTAL_SCHEMA_VERSION, Error, InvariantConclusion, InvariantReport, Model, Predicate,
    ReachabilityConclusion, ReachabilityReport, SearchLimits, State, compile::describe,
    exploration::Exploration,
};

impl Model {
    /// Searches reachable states in shortest transition-count order, deduplicating
    /// full assignments. Invariant violations do not prune this search.
    ///
    /// # Errors
    /// Rejects foreign states, foreign query handles and undeclared query values.
    /// Reports counter capacity failure rather than silently truncating coverage.
    #[track_caller]
    pub fn reachability(
        &self,
        initial: &State,
        target: &Predicate,
        limits: SearchLimits,
    ) -> Result<ReachabilityReport, Error> {
        self.validate_state(initial)?;
        let compiled = self.compile_predicate(target)?;
        let mut search = Exploration::new(initial, limits);
        let mut witness = None;
        while let Some(id) = search.next()? {
            if compiled.evaluate(search.values(id)?)? {
                witness = Some(search.trace(self, id)?);
                break;
            }
            search.expand(self, id, |_| Ok(()))?;
        }
        let conclusion = match (witness, search.coverage().exhausted) {
            (Some(witness), _) => ReachabilityConclusion::Reached { witness },
            (None, true) => ReachabilityConclusion::Unreachable,
            (None, false) => ReachabilityConclusion::Inconclusive,
        };
        Ok(ReachabilityReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            model_id: self.fingerprint.clone(),
            initial: self.snapshot_values(&initial.values),
            context: self.context.clone(),
            target: self.portable_predicate(describe(target)),
            coverage: search.into_coverage(),
            conclusion,
        })
    }

    fn violation(
        &self,
        search: &Exploration,
        id: usize,
    ) -> Result<Option<InvariantConclusion>, Error> {
        for invariant in &self.invariants {
            if !invariant.predicate.evaluate(search.values(id)?)? {
                return Ok(Some(InvariantConclusion::Violation {
                    invariant: Box::new(invariant.info.clone()),
                    witness: search.trace(self, id)?,
                }));
            }
        }
        Ok(None)
    }

    /// Checks all declared invariants on reachable states. The first violation
    /// has a shortest witness within admitted exploration, with ties resolved
    /// by declaration order. Consult state-budget coverage before making a
    /// global shortest-path claim.
    ///
    /// # Errors
    /// Rejects foreign states and reports counter capacity failures.
    #[track_caller]
    pub fn check_invariants(
        &self,
        initial: &State,
        limits: SearchLimits,
    ) -> Result<InvariantReport, Error> {
        self.validate_state(initial)?;
        let mut search = Exploration::new(initial, limits);
        let mut violation = None;
        while let Some(id) = search.next()? {
            violation = self.violation(&search, id)?;
            if violation.is_some() {
                break;
            }
            search.expand(self, id, |_| Ok(()))?;
        }
        let conclusion = match violation {
            Some(conclusion) => conclusion,
            None if search.coverage().exhausted => InvariantConclusion::Holds,
            None => InvariantConclusion::Inconclusive,
        };
        Ok(InvariantReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            model_id: self.fingerprint.clone(),
            initial: self.snapshot_values(&initial.values),
            context: self.context.clone(),
            invariants: self
                .invariants
                .iter()
                .map(|invariant| invariant.info.clone())
                .collect(),
            coverage: search.into_coverage(),
            conclusion,
        })
    }
}
