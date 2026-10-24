use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    EXPERIMENTAL_SCHEMA_VERSION, Error, Model, Predicate, ProjectedCoverage,
    ReachabilityConclusion, SearchLimits, SlicedReachabilityReport, State,
    compile::{CompiledPredicate, CompiledTransition, describe},
    exploration::Exploration,
};

/// Closed dependencies of one validated state-predicate query.
pub(super) struct Dependencies {
    pub(super) facts: BTreeSet<usize>,
    pub(super) transitions: BTreeSet<usize>,
}

impl CompiledPredicate {
    fn reads(&self, facts: &mut BTreeSet<usize>) {
        match self {
            Self::Constant(_) => {}
            Self::Eq(index, _) => {
                facts.insert(*index);
            }
            Self::And(left, right) | Self::Or(left, right) => {
                left.reads(facts);
                right.reads(facts);
            }
            Self::Not(child) => child.reads(facts),
        }
    }
}

impl Dependencies {
    fn close(model: &Model, target: &CompiledPredicate) -> Self {
        let mut facts = BTreeSet::new();
        target.reads(&mut facts);
        let mut pending: VecDeque<_> = facts.iter().copied().collect();
        let mut writers = BTreeMap::<usize, Vec<_>>::new();
        for (index, transition) in model.transitions.iter().enumerate() {
            for (fact, _) in &transition.effects {
                writers.entry(*fact).or_default().push((index, transition));
            }
        }
        let mut dependencies = Self {
            facts,
            transitions: BTreeSet::new(),
        };
        while let Some(fact) = pending.pop_front() {
            for (index, transition) in writers.get(&fact).into_iter().flatten() {
                dependencies.retain_writer(*index, transition, &mut pending);
            }
        }
        dependencies
    }

    fn retain_writer(
        &mut self,
        index: usize,
        transition: &CompiledTransition,
        pending: &mut VecDeque<usize>,
    ) {
        if !self.transitions.insert(index) {
            return;
        }
        let mut reads = BTreeSet::new();
        transition.guard.reads(&mut reads);
        for read in reads {
            if self.facts.insert(read) {
                pending.push_back(read);
            }
        }
    }
}

impl Model {
    /// Searches a dependency-closed projection for a finite state predicate.
    ///
    /// Preserves existential reachability and minimum transition distance when
    /// the state budget does not truncate exploration. Witnesses execute original
    /// transitions with all effects and contain complete concrete snapshots.
    /// Budgets and coverage count projected assignments, not full-model states.
    /// Declared invariants do not prune this search. History is represented only
    /// by explicit facts; no ordering, strategy, liveness or exact-length property
    /// is inferred. Use `reachability` for the full-assignment reference search.
    ///
    /// # Errors
    /// Rejects foreign states, foreign query handles and undeclared query values,
    /// including in dead Boolean branches and when the state budget is zero.
    /// Reports counter capacity failure rather than truncating coverage silently.
    #[track_caller]
    pub fn sliced_reachability(
        &self,
        initial: &State,
        target: &Predicate,
        limits: SearchLimits,
    ) -> Result<SlicedReachabilityReport, Error> {
        self.validate_state(initial)?;
        let compiled = self.compile_predicate(target)?;
        let dependencies = Dependencies::close(self, &compiled);
        let retained_facts = self
            .facts
            .iter()
            .enumerate()
            .filter(|(index, _)| dependencies.facts.contains(index))
            .map(|(_, fact)| fact.key.clone())
            .collect();
        let retained_transitions = self
            .transitions
            .iter()
            .enumerate()
            .filter(|(index, _)| dependencies.transitions.contains(index))
            .map(|(_, transition)| transition.info.id.clone())
            .collect();
        let mut search = Exploration::sliced(initial, limits, dependencies);
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
        Ok(SlicedReachabilityReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            model_id: self.fingerprint.clone(),
            initial: self.snapshot_values(&initial.values),
            context: self.context.clone(),
            target: self.portable_predicate(describe(target)),
            retained_facts,
            retained_transitions,
            coverage: ProjectedCoverage::from_search(search.coverage()),
            conclusion,
        })
    }
}
