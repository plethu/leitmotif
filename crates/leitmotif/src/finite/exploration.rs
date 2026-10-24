use std::collections::{BTreeMap, VecDeque};

use super::{
    Error, ErrorKind, Model, SearchCoverage, SearchLimits, Source, State, Trace, TraceStep, Value,
    slicing::Dependencies,
};

struct Node {
    values: Vec<Value>,
    depth: usize,
    parent: Option<(usize, usize)>,
}

pub(super) struct Exploration {
    nodes: BTreeMap<usize, Node>,
    visited: BTreeMap<Vec<Value>, usize>,
    pending: VecDeque<usize>,
    coverage: SearchCoverage,
    dependencies: Option<Dependencies>,
}

impl Exploration {
    pub(super) fn new(initial: &State, limits: SearchLimits) -> Self {
        Self::with_dependencies(initial, limits, None)
    }

    pub(super) fn sliced(
        initial: &State,
        limits: SearchLimits,
        dependencies: Dependencies,
    ) -> Self {
        Self::with_dependencies(initial, limits, Some(dependencies))
    }

    fn with_dependencies(
        initial: &State,
        limits: SearchLimits,
        dependencies: Option<Dependencies>,
    ) -> Self {
        let mut search = Self {
            dependencies,
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
            search.visited.insert(search.key(&initial.values), 0);
            search.pending.push_back(0);
            search.coverage.discovered_states = 1;
        }
        search
    }

    fn key(&self, values: &[Value]) -> Vec<Value> {
        values
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.dependencies
                    .as_ref()
                    .is_none_or(|dependencies| dependencies.facts.contains(index))
            })
            .map(|(_, value)| value.clone())
            .collect()
    }

    pub(super) fn values(&self, id: usize) -> Result<&[Value], Error> {
        Ok(&self.node(id)?.values)
    }

    pub(super) const fn coverage(&self) -> &SearchCoverage {
        &self.coverage
    }

    pub(super) fn into_coverage(self) -> SearchCoverage {
        self.coverage
    }

    fn node(&self, id: usize) -> Result<&Node, Error> {
        self.nodes.get(&id).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidModel("missing search node"),
                Source::caller(),
            )
        })
    }

    pub(super) fn next(&mut self) -> Result<Option<usize>, Error> {
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

    pub(super) fn expand(
        &mut self,
        model: &Model,
        id: usize,
        mut edge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let depth = self.node(id)?.depth;
        for (transition_index, transition) in model.transitions.iter().enumerate() {
            if self
                .dependencies
                .as_ref()
                .is_some_and(|dependencies| !dependencies.transitions.contains(&transition_index))
            {
                continue;
            }
            let Some(values) = transition.apply(&self.node(id)?.values)? else {
                continue;
            };
            let key = self.key(&values);
            if let Some(successor) = self.visited.get(&key) {
                edge(*successor)?;
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
            self.visited.insert(key, successor_id);
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
            edge(successor_id)?;
        }
        Ok(())
    }

    pub(super) fn trace(&self, model: &Model, mut id: usize) -> Result<Trace, Error> {
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

pub(super) fn increment(value: usize) -> Result<usize, Error> {
    value
        .checked_add(1)
        .ok_or_else(|| Error::new(ErrorKind::Capacity, Source::caller()))
}
