use std::collections::BTreeMap;

use super::{
    ClassificationCoverage, ClassificationLimits, EXPERIMENTAL_SCHEMA_VERSION, Error, ErrorKind,
    HypothesisConclusion, HypothesisReport, Model, NamedPredicate, Predicate, Premise, Snapshot,
    Source, Value,
    author::unique_name,
    compile::{CompiledPredicate, describe},
    exploration::increment,
};

/// Mixed-radix enumeration retains one candidate and never materializes the
/// Cartesian product. The zero-fact model has exactly one empty assignment.
struct Candidates {
    positions: Vec<usize>,
    current: Option<Vec<Value>>,
}

impl Candidates {
    fn new(model: &Model) -> Result<Self, Error> {
        let mut values = Vec::new();
        for fact in &model.facts {
            let value = fact.domain.first().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidModel("empty compiled domain"),
                    fact.provenance.clone(),
                )
            })?;
            values.push(value.clone());
        }
        Ok(Self {
            positions: vec![0; model.facts.len()],
            current: Some(values),
        })
    }

    fn advance(&mut self, model: &Model) -> Result<(), Error> {
        for (index, position) in self.positions.iter_mut().enumerate().rev() {
            let domain = &model
                .facts
                .get(index)
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidModel("missing candidate fact"),
                        Source::caller(),
                    )
                })?
                .domain;
            let next = increment(*position)?;
            if next < domain.len() {
                *position = next;
                self.refresh(model)?;
                return Ok(());
            }
            *position = 0;
        }
        self.current = None;
        Ok(())
    }

    fn refresh(&mut self, model: &Model) -> Result<(), Error> {
        let mut values = Vec::new();
        for (fact, position) in model.facts.iter().zip(&self.positions) {
            let value = fact.domain.get(*position).ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidModel("invalid candidate position"),
                    fact.provenance.clone(),
                )
            })?;
            values.push(value.clone());
        }
        self.current = Some(values);
        Ok(())
    }
}

pub(super) struct ClassificationQuery {
    pub(super) hypothesis: CompiledPredicate,
    pub(super) premises: Vec<CompiledPredicate>,
    pub(super) hypothesis_description: super::PredicateDescription,
    pub(super) descriptions: Vec<NamedPredicate>,
}

struct Evidence {
    coverage: ClassificationCoverage,
    witness: Option<Snapshot>,
    counterexample: Option<Snapshot>,
}

impl Evidence {
    fn record(&mut self, model: &Model, supports: bool, values: &[Value]) {
        let slot = if supports {
            &mut self.witness
        } else {
            &mut self.counterexample
        };
        if slot.is_none() {
            *slot = Some(model.snapshot_values(values));
        }
    }

    const fn conclusion(&self) -> HypothesisConclusion {
        if !self.coverage.exhausted {
            return if self.witness.is_some() {
                HypothesisConclusion::Possible
            } else {
                HypothesisConclusion::Inconclusive
            };
        }
        match (self.witness.is_some(), self.counterexample.is_some()) {
            (false, false) => HypothesisConclusion::Inconsistent,
            (true, false) => HypothesisConclusion::Required,
            (false, true) => HypothesisConclusion::RuledOut,
            (true, true) => HypothesisConclusion::Possible,
        }
    }
}

impl Model {
    /// Classifies over the complete finite assignment space, restricted ONLY by
    /// explicitly supplied premises. Transitions and invariants are not premises.
    /// A supporting witness establishes possibility before exhaustion; required,
    /// ruled-out and inconsistent verdicts always require exhausted enumeration.
    ///
    /// # Errors
    /// Rejects empty/duplicate premise IDs, foreign handles and undeclared values,
    /// even when the candidate budget is zero. Reports counter capacity failure.
    #[track_caller]
    pub fn classify(
        &self,
        hypothesis: &Predicate,
        premises: &[Premise],
        limits: ClassificationLimits,
    ) -> Result<HypothesisReport, Error> {
        let query = self.prepare_classification(hypothesis, premises)?;
        let evidence = self.enumerate(&query.hypothesis, &query.premises, limits)?;
        Ok(HypothesisReport {
            schema_version: EXPERIMENTAL_SCHEMA_VERSION.to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            model_id: self.fingerprint.clone(),
            context: self.context.clone(),
            hypothesis: query.hypothesis_description,
            premises: query.descriptions,
            conclusion: evidence.conclusion(),
            coverage: evidence.coverage,
            witness: evidence.witness,
            counterexample: evidence.counterexample,
        })
    }

    pub(super) fn prepare_classification(
        &self,
        hypothesis: &Predicate,
        premises: &[Premise],
    ) -> Result<ClassificationQuery, Error> {
        let compiled_hypothesis = self.compile_predicate(hypothesis)?;
        let mut seen = BTreeMap::new();
        let mut compiled_premises = Vec::new();
        let mut descriptions = Vec::new();
        for premise in premises {
            unique_name(&mut seen, &premise.id, "premise", &premise.provenance)?;
            compiled_premises.push(self.compile_predicate(&premise.predicate)?);
            let mut description = NamedPredicate {
                id: premise.id.clone(),
                predicate: describe(&premise.predicate),
                provenance: premise.provenance.clone(),
            };
            self.portable_named(&mut description);
            descriptions.push(description);
        }
        Ok(ClassificationQuery {
            hypothesis: compiled_hypothesis,
            premises: compiled_premises,
            hypothesis_description: self.portable_predicate(describe(hypothesis)),
            descriptions,
        })
    }

    fn enumerate(
        &self,
        hypothesis: &CompiledPredicate,
        premises: &[CompiledPredicate],
        limits: ClassificationLimits,
    ) -> Result<Evidence, Error> {
        let mut candidates = Candidates::new(self)?;
        let mut evidence = Evidence {
            coverage: ClassificationCoverage {
                limits,
                evaluated_candidates: 0,
                satisfying_candidates: 0,
                exhausted: false,
            },
            witness: None,
            counterexample: None,
        };
        while let Some(values) = &candidates.current {
            if evidence.coverage.evaluated_candidates >= limits.max_candidates {
                break;
            }
            evidence.coverage.evaluated_candidates =
                increment(evidence.coverage.evaluated_candidates)?;
            if satisfies(premises, values)? {
                evidence.coverage.satisfying_candidates =
                    increment(evidence.coverage.satisfying_candidates)?;
                evidence.record(self, hypothesis.evaluate(values)?, values);
            }
            candidates.advance(self)?;
        }
        evidence.coverage.exhausted = candidates.current.is_none();
        Ok(evidence)
    }
}

pub(super) fn satisfies(premises: &[CompiledPredicate], values: &[Value]) -> Result<bool, Error> {
    for premise in premises {
        if !premise.evaluate(values)? {
            return Ok(false);
        }
    }
    Ok(true)
}
