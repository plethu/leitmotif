use std::collections::VecDeque;

use super::super::{Error, ErrorKind, Source, exploration::increment};

#[derive(Default)]
pub(super) struct ReverseGraph {
    predecessors: Vec<Vec<usize>>,
}

impl ReverseGraph {
    pub(super) fn record(&mut self, from: usize, to: usize) -> Result<(), Error> {
        let required = increment(to)?;
        if self.predecessors.len() < required {
            self.predecessors.resize_with(required, Vec::new);
        }
        self.predecessors
            .get_mut(to)
            .ok_or_else(invalid_edge)?
            .push(from);
        Ok(())
    }

    pub(super) fn complete(&mut self, nodes: usize) {
        self.predecessors.resize_with(nodes, Vec::new);
    }

    pub(super) fn routes_from(
        &self,
        endings: impl IntoIterator<Item = usize>,
        mut inspect: impl FnMut(usize, usize),
    ) -> Result<Vec<bool>, Error> {
        let mut marked = vec![false; self.predecessors.len()];
        let mut pending = VecDeque::new();
        for ending in endings {
            enqueue_unseen(&mut marked, &mut pending, ending)?;
        }
        while let Some(node) = pending.pop_front() {
            for &predecessor in self.predecessors.get(node).ok_or_else(invalid_edge)? {
                inspect(predecessor, node);
                enqueue_unseen(&mut marked, &mut pending, predecessor)?;
            }
        }
        Ok(marked)
    }
}

fn enqueue_unseen(
    marked: &mut [bool],
    pending: &mut VecDeque<usize>,
    node: usize,
) -> Result<(), Error> {
    let seen = marked.get_mut(node).ok_or_else(invalid_edge)?;
    if !*seen {
        *seen = true;
        pending.push_back(node);
    }
    Ok(())
}

fn invalid_edge() -> Error {
    Error::new(
        ErrorKind::InvalidModel("missing progression graph node"),
        Source::caller(),
    )
}

#[cfg(test)]
mod tests;
