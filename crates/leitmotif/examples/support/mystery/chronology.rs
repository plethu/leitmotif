//! Three historical occurrences, with an authored strict partial order.
//! No dates, intervals, duration arithmetic, or unbounded occurrence generation.
use std::panic::Location;

mod performance;
mod presentation;
pub(super) use performance::{DisclosureRule, LIMITS, Performance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Occurrence {
    LockA,
    LockB,
    Conversation,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Before {
    pub(super) earlier: Occurrence,
    pub(super) later: Occurrence,
    pub(super) source: &'static Location<'static>,
}
impl Before {
    #[track_caller]
    pub(super) const fn new(earlier: Occurrence, later: Occurrence) -> Self {
        Self {
            earlier,
            later,
            source: Location::caller(),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub(super) enum OrderError {
    #[error("incompatible strict-before declarations: {chain:?}")]
    Cycle { chain: Vec<Before> },
    #[error("duplicate strict-before declarations: {original:?}; {repeated:?}")]
    Duplicate { original: Before, repeated: Before },
}
pub(super) struct Chronology {
    edges: Vec<Before>,
}
impl Chronology {
    pub(super) fn new(edges: impl IntoIterator<Item = Before>) -> Result<Self, OrderError> {
        let mut order = Self { edges: Vec::new() };
        for edge in edges {
            if let Some(original) = order
                .edges
                .iter()
                .find(|old| old.earlier == edge.earlier && old.later == edge.later)
            {
                return Err(OrderError::Duplicate {
                    original: *original,
                    repeated: edge,
                });
            }
            if let Some(mut chain) = order.path(edge.later, edge.earlier) {
                chain.push(edge);
                return Err(OrderError::Cycle { chain });
            }
            order.edges.push(edge);
        }
        Ok(order)
    }
    pub(super) fn story() -> Result<Self, OrderError> {
        Self::new([
            Before::new(Occurrence::LockA, Occurrence::Conversation),
            Before::new(Occurrence::LockB, Occurrence::Conversation),
        ])
    }
    pub(super) fn requires_before(&self, earlier: Occurrence, later: Occurrence) -> bool {
        earlier != later && self.path(earlier, later).is_some()
    }
    fn path(&self, from: Occurrence, to: Occurrence) -> Option<Vec<Before>> {
        if from == to {
            return Some(Vec::new());
        }
        // Existing edges are acyclic: each insertion validates before retaining its edge.
        for edge in self.edges.iter().filter(|edge| edge.earlier == from) {
            if let Some(mut suffix) = self.path(edge.later, to) {
                suffix.insert(0, *edge);
                return Some(suffix);
            }
        }
        None
    }
}
pub(super) use presentation::demonstrate;
