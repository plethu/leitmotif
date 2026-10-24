use std::collections::{BTreeMap, VecDeque};

use super::{
    EXPERIMENTAL_SCHEMA_VERSION, Error, ErrorKind, InvariantConclusion, InvariantReport, Model,
    Predicate, ReachabilityConclusion, ReachabilityReport, SearchCoverage, SearchLimits, Source,
    State, Trace, TraceStep, Value, compile::describe,
};

struct Node {
    values: Vec<Value>,
    depth: usize,
    parent: Option<(usize, usize)>,
}

struct Exploration {
    nodes: BTreeMap<usize, Node>,
    visited: BTreeMap<Vec<Value>, usize>,
    pending: VecDeque<usize>,
    coverage: SearchCoverage,
}

impl Exploration {
    fn new(initial: &State, limits: SearchLimits) -> Self {
        let mut search = Self {
            nodes: BTreeMap::new(),
            visited: BTreeMap::new(),
            pending: VecDeque::new(),
            coverage: SearchCoverage {
                limits,
                discovered_states: 0,
                evaluated_states: 0,
                deepest_evaluated: None,
                exhausted: false,
                state_limit_hit: limits.max_states == 0,
                depth_limit_hit: false,
            },
        };
        if limits.max_states > 0 {
            search.nodes.insert(
                0,
                Node {
                    values: initial.values.clone(),
                    depth: 0,
                    parent: None,
                },
            );
            search.visited.insert(initial.values.clone(), 0);
            search.pending.push_back(0);
            search.coverage.discovered_states = 1;
        }
        search
    }

    fn node(&self, id: usize) -> Result<&Node, Error> {
        self.nodes.get(&id).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidModel("missing search node"),
                Source::caller(),
            )
        })
    }

    fn next(&mut self) -> Result<Option<usize>, Error> {
        let Some(id) = self.pending.pop_front() else {
            self.coverage.exhausted =
                !self.coverage.state_limit_hit && !self.coverage.depth_limit_hit;
            return Ok(None);
        };
        let depth = self.node(id)?.depth;
        self.coverage.evaluated_states = increment(self.coverage.evaluated_states)?;
        self.coverage.deepest_evaluated = Some(depth);
        Ok(Some(id))
    }

    fn expand(&mut self, model: &Model, id: usize) -> Result<(), Error> {
        let depth = self.node(id)?.depth;
        for (transition_index, transition) in model.transitions.iter().enumerate() {
            let Some(values) = transition.apply(&self.node(id)?.values)? else {
                continue;
            };
            if self.visited.contains_key(&values) {
                continue;
            }
            if depth >= self.coverage.limits.max_depth {
                self.coverage.depth_limit_hit = true;
                continue;
            }
            if self.nodes.len() >= self.coverage.limits.max_states {
                self.coverage.state_limit_hit = true;
                continue;
            }
            let successor_id = self.nodes.len();
            let successor_depth = increment(depth)?;
            self.visited.insert(values.clone(), successor_id);
            self.nodes.insert(
                successor_id,
                Node {
                    values,
                    depth: successor_depth,
                    parent: Some((id, transition_index)),
                },
            );
            self.pending.push_back(successor_id);
            self.coverage.discovered_states = self.nodes.len();
        }
        Ok(())
    }

    fn trace(&self, model: &Model, mut id: usize) -> Result<Trace, Error> {
        let mut steps = Vec::new();
        loop {
            let node = self.node(id)?;
            let Some((parent, transition_index)) = node.parent else {
                steps.reverse();
                return Ok(Trace {
                    initial: model.snapshot_values(&node.values),
                    steps,
                });
            };
            let transition = model.transitions.get(transition_index).ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidModel("missing trace transition"),
                    Source::caller(),
                )
            })?;
            steps.push(TraceStep {
                transition: transition.info.clone(),
                state: model.snapshot_values(&node.values),
            });
            id = parent;
        }
    }
}

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
            if compiled.evaluate(&search.node(id)?.values)? {
                witness = Some(search.trace(self, id)?);
                break;
            }
            search.expand(self, id)?;
        }
        let conclusion = if let Some(witness) = witness {
            ReachabilityConclusion::Reached { witness }
        } else if search.coverage.exhausted {
            ReachabilityConclusion::Unreachable
        } else {
            ReachabilityConclusion::Inconclusive
        };
        Ok(ReachabilityReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            context: self.context.clone(),
            target: describe(target),
            coverage: search.coverage,
            conclusion,
        })
    }

    fn violation(
        &self,
        search: &Exploration,
        id: usize,
    ) -> Result<Option<InvariantConclusion>, Error> {
        for invariant in &self.invariants {
            if !invariant.predicate.evaluate(&search.node(id)?.values)? {
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
            search.expand(self, id)?;
        }
        let conclusion = match violation {
            Some(conclusion) => conclusion,
            None if search.coverage.exhausted => InvariantConclusion::Holds,
            None => InvariantConclusion::Inconclusive,
        };
        Ok(InvariantReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            context: self.context.clone(),
            invariants: self
                .invariants
                .iter()
                .map(|invariant| invariant.info.clone())
                .collect(),
            coverage: search.coverage,
            conclusion,
        })
    }
}

pub(super) fn increment(value: usize) -> Result<usize, Error> {
    value
        .checked_add(1)
        .ok_or_else(|| Error::new(ErrorKind::Capacity, Source::caller()))
}
