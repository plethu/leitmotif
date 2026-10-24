use std::collections::BTreeSet;

use super::{
    Assignment, Error, ErrorKind, FactKind, FactValue, Model, NamedPredicate, Predicate,
    PredicateDescription, PredicateExpression, PredicateKind, Source, Transition, TransitionInfo,
    Value, author::unique_name,
};

#[derive(Debug)]
pub(super) enum CompiledPredicate {
    Constant(bool),
    Eq(usize, Value),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
}

impl CompiledPredicate {
    pub(super) fn evaluate(&self, values: &[Value]) -> Result<bool, Error> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Eq(index, value) => {
                values
                    .get(*index)
                    .map(|actual| actual == value)
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidModel("missing indexed value"),
                            Source::caller(),
                        )
                    })
            }
            Self::And(left, right) => Ok(left.evaluate(values)? && right.evaluate(values)?),
            Self::Or(left, right) => Ok(left.evaluate(values)? || right.evaluate(values)?),
            Self::Not(predicate) => Ok(!predicate.evaluate(values)?),
        }
    }
}

#[derive(Debug)]
pub(super) struct CompiledTransition {
    pub(super) info: TransitionInfo,
    pub(super) guard: CompiledPredicate,
    pub(super) effects: Vec<(usize, Value)>,
}

impl CompiledTransition {
    pub(super) fn apply(&self, values: &[Value]) -> Result<Option<Vec<Value>>, Error> {
        if !self.guard.evaluate(values)? {
            return Ok(None);
        }
        let mut successor = values.to_vec();
        for (index, value) in &self.effects {
            let target = successor.get_mut(*index).ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidModel("missing effect target"),
                    self.info.provenance.clone(),
                )
            })?;
            target.clone_from(value);
        }
        Ok(Some(successor))
    }
}

#[derive(Debug)]
pub(super) struct CompiledInvariant {
    pub(super) info: NamedPredicate,
    pub(super) predicate: CompiledPredicate,
}

impl Model {
    pub(super) fn compile_predicate(
        &self,
        predicate: &Predicate,
    ) -> Result<CompiledPredicate, Error> {
        match &predicate.kind {
            PredicateKind::Constant(value) => Ok(CompiledPredicate::Constant(*value)),
            PredicateKind::Eq(assignment) => {
                self.validate_assignment(assignment)?;
                Ok(CompiledPredicate::Eq(
                    assignment.index,
                    assignment.value.clone(),
                ))
            }
            PredicateKind::And(left, right) => Ok(CompiledPredicate::And(
                Box::new(self.compile_predicate(left)?),
                Box::new(self.compile_predicate(right)?),
            )),
            PredicateKind::Or(left, right) => Ok(CompiledPredicate::Or(
                Box::new(self.compile_predicate(left)?),
                Box::new(self.compile_predicate(right)?),
            )),
            PredicateKind::Not(child) => Ok(CompiledPredicate::Not(Box::new(
                self.compile_predicate(child)?,
            ))),
        }
    }

    pub(super) fn validate_assignment(&self, assignment: &Assignment) -> Result<(), Error> {
        if !self.owner.matches(&assignment.owner) {
            return Err(Error::new(
                ErrorKind::ForeignHandle(assignment.fact.clone()),
                assignment.provenance.clone(),
            ));
        }
        let fact = self.facts.get(assignment.index).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidModel("missing fact index"),
                assignment.provenance.clone(),
            )
        })?;
        if !fact.domain.contains(&assignment.value) {
            return Err(Error::new(
                ErrorKind::InvalidValue {
                    fact: fact.key.clone(),
                    value: assignment.value.clone(),
                },
                assignment.provenance.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn compile_transition(
        &self,
        transition: Transition,
        seen: &mut BTreeSet<String>,
    ) -> Result<CompiledTransition, Error> {
        unique_name(seen, &transition.id, "transition", &transition.provenance)?;
        let guard = self.compile_predicate(&transition.guard)?;
        let mut writes = BTreeSet::new();
        for effect in &transition.effects {
            self.validate_assignment(effect)?;
            if !writes.insert(effect.index) {
                return Err(Error::new(
                    ErrorKind::DuplicateWrite(effect.fact.clone()),
                    effect.provenance.clone(),
                ));
            }
            if self
                .facts
                .get(effect.index)
                .is_some_and(|fact| fact.kind == FactKind::Setting)
            {
                return Err(Error::new(
                    ErrorKind::SettingWrite(effect.fact.clone()),
                    effect.provenance.clone(),
                ));
            }
        }
        let info = TransitionInfo {
            id: transition.id,
            guard: describe(&transition.guard),
            effects: transition.effects.iter().map(describe_assignment).collect(),
            provenance: transition.provenance,
        };
        let effects = transition
            .effects
            .into_iter()
            .map(|effect| (effect.index, effect.value))
            .collect();
        Ok(CompiledTransition {
            info,
            guard,
            effects,
        })
    }
}

pub(super) fn describe(predicate: &Predicate) -> PredicateDescription {
    let expression = match &predicate.kind {
        PredicateKind::Constant(value) => PredicateExpression::Constant(*value),
        PredicateKind::Eq(assignment) => PredicateExpression::Eq(describe_assignment(assignment)),
        PredicateKind::And(left, right) => {
            PredicateExpression::And(Box::new(describe(left)), Box::new(describe(right)))
        }
        PredicateKind::Or(left, right) => {
            PredicateExpression::Or(Box::new(describe(left)), Box::new(describe(right)))
        }
        PredicateKind::Not(child) => PredicateExpression::Not(Box::new(describe(child))),
    };
    PredicateDescription {
        expression,
        provenance: predicate.provenance.clone(),
    }
}

fn describe_assignment(assignment: &Assignment) -> FactValue {
    FactValue {
        fact: assignment.fact.clone(),
        value: assignment.value.clone(),
        provenance: assignment.provenance.clone(),
    }
}
