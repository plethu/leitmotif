use std::collections::BTreeMap;

use super::super::{Error, Predicate, Source, author::unique_name};

/// A named terminal completion permitted by one progression query.
#[derive(Clone, Debug)]
pub struct PermittedEnding {
    pub(super) id: String,
    pub(super) predicate: Predicate,
    pub(super) provenance: Source,
}

impl PermittedEnding {
    /// Declares an ending; its name is validated when the query is constructed.
    /// Intentional failure endings use the same declaration as other completions.
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>, predicate: Predicate) -> Self {
        Self {
            id: id.into(),
            predicate,
            provenance: Source::caller(),
        }
    }
}

/// Explicit stopping boundaries and allowed completions for progression analysis.
/// Overlapping named endings and an empty permitted set are valid.
#[derive(Clone, Debug)]
pub struct ProgressionQuery {
    pub(super) terminal: Predicate,
    pub(super) endings: Vec<PermittedEnding>,
}

impl ProgressionQuery {
    /// Constructs a query without enumerating model assignments.
    /// All predicates are validated against the model when analysis starts.
    ///
    /// # Errors
    /// Rejects blank or duplicate ending IDs with declaration provenance.
    pub fn new(
        terminal: Predicate,
        endings: impl IntoIterator<Item = PermittedEnding>,
    ) -> Result<Self, Error> {
        let endings: Vec<_> = endings.into_iter().collect();
        let mut names = BTreeMap::new();
        for ending in &endings {
            unique_name(
                &mut names,
                &ending.id,
                "permitted ending",
                &ending.provenance,
            )?;
        }
        Ok(Self { terminal, endings })
    }
}
