use super::{
    EXPERIMENTAL_SCHEMA_VERSION, Error, ErrorKind, Model, NamedPredicate, ProgressionConclusion,
    ProgressionReport, SearchLimits, State, Value,
    compile::{CompiledPredicate, describe},
    exploration::{Exploration, increment},
};

mod query;
mod reverse;

pub use query::{PermittedEnding, ProgressionQuery};
use reverse::ReverseGraph;

struct CompiledEnding {
    predicate: CompiledPredicate,
    description: NamedPredicate,
}

struct CompiledQuery {
    terminal: CompiledPredicate,
    endings: Vec<CompiledEnding>,
}

#[derive(Clone, Copy)]
enum StateKind {
    Nonterminal,
    Terminal,
    PermittedEnding,
}

impl CompiledQuery {
    fn new(model: &Model, query: &ProgressionQuery) -> Result<Self, Error> {
        let terminal = model.compile_predicate(&query.terminal)?;
        let mut endings = Vec::new();
        for ending in &query.endings {
            let predicate = model.compile_predicate(&ending.predicate)?;
            let mut description = NamedPredicate {
                id: ending.id.clone(),
                predicate: describe(&ending.predicate),
                provenance: ending.provenance.clone(),
            };
            model.portable_named(&mut description);
            endings.push(CompiledEnding {
                predicate,
                description,
            });
        }
        Ok(Self { terminal, endings })
    }

    fn classify(&self, model: &Model, values: &[Value]) -> Result<StateKind, Error> {
        let terminal = self.terminal.evaluate(values)?;
        for ending in &self.endings {
            if !ending.predicate.evaluate(values)? {
                continue;
            }
            if !terminal {
                return Err(Error::new(
                    ErrorKind::NonterminalEnding {
                        ending: ending.description.id.clone(),
                        counterexample: Box::new(model.snapshot_values(values)),
                    },
                    ending.description.provenance.clone(),
                ));
            }
            return Ok(StateKind::PermittedEnding);
        }
        Ok(if terminal {
            StateKind::Terminal
        } else {
            StateKind::Nonterminal
        })
    }
}

impl Model {
    /// Finds reachable nonterminal states with no route to any permitted ending.
    /// Terminal states stop this query even when the model declares successors.
    /// Incomplete exploration is always inconclusive. A route does not imply an
    /// eventual completion or a winning strategy.
    ///
    /// # Errors
    /// Rejects foreign states or predicates, undeclared values, and any visited
    /// nonterminal matching a permitted ending. The latter retains the ending's
    /// provenance and the full reachable counterexample assignment.
    #[track_caller]
    pub fn progression(
        &self,
        initial: &State,
        query: &ProgressionQuery,
        limits: SearchLimits,
    ) -> Result<ProgressionReport, Error> {
        self.validate_state(initial)?;
        let compiled = CompiledQuery::new(self, query)?;
        let mut search = Exploration::new(initial, limits);
        let mut graph = ReverseGraph::default();
        let mut kinds = Vec::new();
        while let Some(id) = search.next()? {
            let kind = compiled.classify(self, search.values(id)?)?;
            kinds.push(kind);
            if matches!(kind, StateKind::Nonterminal) {
                search.expand(self, id, |successor| graph.record(id, successor))?;
            }
        }
        let conclusion = if search.coverage().exhausted {
            graph.complete(kinds.len());
            conclude(self, &search, &graph, &kinds)?
        } else {
            ProgressionConclusion::Inconclusive
        };
        Ok(ProgressionReport {
            initial: self.snapshot_values(&initial.values),
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            model_id: self.fingerprint.clone(),
            context: self.context.clone(),
            terminal: self.portable_predicate(describe(&query.terminal)),
            permitted_endings: compiled
                .endings
                .into_iter()
                .map(|ending| ending.description)
                .collect(),
            coverage: search.into_coverage(),
            conclusion,
        })
    }
}

fn conclude(
    model: &Model,
    search: &Exploration,
    graph: &ReverseGraph,
    kinds: &[StateKind],
) -> Result<ProgressionConclusion, Error> {
    let endings = kinds
        .iter()
        .enumerate()
        .filter_map(|(id, kind)| matches!(kind, StateKind::PermittedEnding).then_some(id));
    let routes = graph.routes_from(endings, |_, _| {})?;
    let mut first = None;
    let mut trapped_states = 0;
    for (id, (kind, has_route)) in kinds.iter().zip(routes).enumerate() {
        if matches!(kind, StateKind::Nonterminal) && !has_route {
            first.get_or_insert(id);
            trapped_states = increment(trapped_states)?;
        }
    }
    first.map_or(Ok(ProgressionConclusion::NoTraps), |id| {
        Ok(ProgressionConclusion::Trap {
            entry: search.trace(model, id)?,
            trapped_states,
        })
    })
}
