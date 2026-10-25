use oxidd::{BooleanFunction, bdd::BDDFunction};
use std::collections::BTreeMap;

use clap::ValueEnum;
use leitmotif::{FactInfo, Model, PredicateDescription, PredicateExpression, Snapshot, Value};
use serde::Serialize;

use super::{Failure, Oxidd, Result};

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(in super::super) enum Order {
    Paired,
    Banked,
}

struct Field<'a> {
    fact: &'a FactInfo,
    current: Vec<u32>,
    next: Vec<u32>,
}

pub(in super::super) struct Encoding<'a> {
    fields: Vec<Field<'a>>,
    pub(in super::super) current: Vec<u32>,
    pub(in super::super) next: Vec<u32>,
    pub(in super::super) variables: u32,
}

impl<'a> Encoding<'a> {
    pub(in super::super) fn new(model: &'a Model, order: Order) -> Result<Self> {
        let widths = model
            .facts()
            .iter()
            .map(|fact| {
                let last = fact
                    .domain
                    .len()
                    .checked_sub(1)
                    .ok_or(Failure::Invalid("empty fact domain"))?;
                usize::BITS
                    .checked_sub(last.leading_zeros())
                    .ok_or(Failure::Invalid("domain width"))
            })
            .collect::<Result<Vec<_>>>()?;
        let bits = widths.iter().try_fold(0_u32, |sum, width| {
            sum.checked_add(*width)
                .ok_or(Failure::Invalid("encoding width overflow"))
        })?;
        if !(1..=60).contains(&bits) {
            return Err(Failure::Invalid(
                "experiment supports 1 through 60 encoded bits",
            ));
        }
        let mut offset = 0_u32;
        let mut fields = Vec::new();
        let mut current = Vec::new();
        let mut next = Vec::new();
        for (fact, width) in model.facts().iter().zip(widths) {
            let mut field = Field {
                fact,
                current: Vec::new(),
                next: Vec::new(),
            };
            for _ in 0..width {
                let (before, after) = match order {
                    Order::Paired => (
                        offset.checked_mul(2),
                        offset.checked_mul(2).and_then(|n| n.checked_add(1)),
                    ),
                    Order::Banked => (Some(offset), offset.checked_add(bits)),
                };
                let before = before.ok_or(Failure::Invalid("variable index overflow"))?;
                let after = after.ok_or(Failure::Invalid("variable index overflow"))?;
                field.current.push(before);
                field.next.push(after);
                current.push(before);
                next.push(after);
                offset = offset
                    .checked_add(1)
                    .ok_or(Failure::Invalid("variable index overflow"))?;
            }
            fields.push(field);
        }
        Ok(Self {
            fields,
            current,
            next,
            variables: bits
                .checked_mul(2)
                .ok_or(Failure::Invalid("variable count overflow"))?,
        })
    }

    pub(in super::super) fn pairs(&self) -> Vec<(u32, u32)> {
        self.next
            .iter()
            .copied()
            .zip(self.current.iter().copied())
            .collect()
    }

    fn value(&self, backend: &Oxidd, key: &str, value: &Value, prime: bool) -> Result<BDDFunction> {
        let field = self
            .fields
            .iter()
            .find(|field| field.fact.key == key)
            .ok_or(Failure::Invalid("undeclared symbolic fact"))?;
        let code = field
            .fact
            .domain
            .iter()
            .position(|candidate| candidate == value)
            .ok_or(Failure::Invalid("undeclared symbolic value"))?;
        let mut result = backend.constant(true);
        let variables = if prime { &field.next } else { &field.current };
        for (bit, id) in variables.iter().enumerate() {
            let selected = code
                .checked_shr(u32::try_from(bit)?)
                .ok_or(Failure::Invalid("domain bit overflow"))?
                & 1
                == 1;
            let variable = backend.variable(*id)?;
            result = result.and(&if selected { variable } else { variable.not()? })?;
        }
        Ok(result)
    }

    pub(in super::super) fn predicate(
        &self,
        backend: &Oxidd,
        predicate: &PredicateDescription,
    ) -> Result<BDDFunction> {
        match &predicate.expression {
            PredicateExpression::Constant(value) => Ok(backend.constant(*value)),
            PredicateExpression::Eq(assignment) => {
                self.value(backend, &assignment.fact, &assignment.value, false)
            }
            PredicateExpression::And(left, right) => Ok(self
                .predicate(backend, left)?
                .and(&self.predicate(backend, right)?)?),
            PredicateExpression::Or(left, right) => Ok(self
                .predicate(backend, left)?
                .or(&self.predicate(backend, right)?)?),
            PredicateExpression::Not(child) => Ok(self.predicate(backend, child)?.not()?),
        }
    }

    pub(in super::super) fn legal(&self, backend: &Oxidd, prime: bool) -> Result<BDDFunction> {
        let mut result = backend.constant(true);
        for field in &self.fields {
            let mut allowed = backend.constant(false);
            for value in &field.fact.domain {
                allowed = allowed.or(&self.value(backend, &field.fact.key, value, prime)?)?;
            }
            result = result.and(&allowed)?;
        }
        Ok(result)
    }

    pub(in super::super) fn snapshot(
        &self,
        backend: &Oxidd,
        snapshot: &Snapshot,
        prime: bool,
    ) -> Result<BDDFunction> {
        if snapshot.values.len() != self.fields.len() {
            return Err(Failure::Invalid("incomplete symbolic snapshot"));
        }
        let mut result = backend.constant(true);
        for field in &self.fields {
            let value = snapshot
                .values
                .get(&field.fact.key)
                .ok_or(Failure::Invalid("missing symbolic fact"))?;
            result = result.and(&self.value(backend, &field.fact.key, value, prime)?)?;
        }
        Ok(result)
    }

    pub(in super::super) fn decode(&self, valuation: &[bool]) -> Result<Snapshot> {
        let mut values = BTreeMap::new();
        for field in &self.fields {
            let value = field.decode(valuation)?;
            values.insert(field.fact.key.clone(), value);
        }
        Ok(Snapshot { values })
    }

    pub(in super::super) fn relations(
        &self,
        backend: &Oxidd,
        model: &Model,
    ) -> Result<Vec<(String, BDDFunction)>> {
        let legal = (self.legal(backend, false)?).and(&self.legal(backend, true)?)?;
        let mut relations = Vec::new();
        for transition in model.transitions() {
            let effects = transition
                .effects
                .iter()
                .map(|effect| (effect.fact.as_str(), &effect.value))
                .collect::<BTreeMap<_, _>>();
            let mut relation = legal.and(&self.predicate(backend, &transition.guard)?)?;
            for field in &self.fields {
                let constraint = self.successor(
                    backend,
                    field,
                    effects.get(field.fact.key.as_str()).copied(),
                )?;
                relation = relation.and(&constraint)?;
            }
            relations.push((transition.id.clone(), relation));
        }
        Ok(relations)
    }

    fn successor(
        &self,
        backend: &Oxidd,
        field: &Field<'_>,
        value: Option<&Value>,
    ) -> Result<BDDFunction> {
        if let Some(value) = value {
            return self.value(backend, &field.fact.key, value, true);
        }
        let mut frame = backend.constant(true);
        for (before, after) in field.current.iter().zip(&field.next) {
            frame = frame.and(&(backend.variable(*before)?).equiv(&backend.variable(*after)?)?)?;
        }
        Ok(frame)
    }

    pub(in super::super) fn count(&self, value: &BDDFunction) -> Result<f64> {
        let free = i32::try_from(self.next.len())?;
        Ok(Oxidd::count(value, self.variables) / 2_f64.powi(free))
    }
}

impl Field<'_> {
    fn decode(&self, valuation: &[bool]) -> Result<Value> {
        let mut code = 0_usize;
        for (bit, id) in self.current.iter().enumerate() {
            if !*valuation
                .get(usize::try_from(*id)?)
                .ok_or(Failure::Invalid("incomplete BDD valuation"))?
            {
                continue;
            }
            code |= 1_usize
                .checked_shl(u32::try_from(bit)?)
                .ok_or(Failure::Invalid("domain code overflow"))?;
        }
        self.fact.domain.get(code).cloned().ok_or(Failure::Invalid(
            "BDD admitted an illegal finite-domain code",
        ))
    }
}
