use leitmotif::{Predicate, Source, Value};
use recite_core::{
    SourceSpan,
    compiled::{CompiledConditionCall, CompiledConditionExpression, MAX_COMPILED_CONDITION_DEPTH},
};
use recite_runtime::ConditionExpectedType;

use super::{Bindings, Error, ErrorKind, error::source};

impl Bindings {
    /// Lowers a real compiled Boolean expression, validating every branch.
    /// Supply the containing compiled row's source-map span.
    ///
    /// # Errors
    /// Rejects unmapped calls, enum calls in Boolean expressions, empty groups
    /// and expressions exceeding Recite's compiled depth bound. The lowered
    /// binary predicate also permits at most 128 child edges per path; mixed
    /// deep/wide expressions can exceed this bound despite being valid Recite.
    pub fn guard(
        &self,
        expression: &CompiledConditionExpression,
        span: &SourceSpan,
    ) -> Result<Predicate, Error> {
        Ok(self.lower(expression, &source(span), 0)?.predicate)
    }

    /// Builds a finite predicate for one variant of a compiled `:match` scrutinee.
    ///
    /// # Errors
    /// Rejects unmapped calls, Boolean bindings and undeclared variants.
    pub fn enum_is(
        &self,
        call: &CompiledConditionCall,
        variant: &str,
        span: &SourceSpan,
    ) -> Result<Predicate, Error> {
        self.case(
            call,
            ConditionExpectedType::Enum,
            &Value::Symbol(variant.to_owned()),
            &source(span),
        )
    }

    fn case(
        &self,
        call: &CompiledConditionCall,
        kind: ConditionExpectedType,
        value: &Value,
        provenance: &Source,
    ) -> Result<Predicate, Error> {
        let binding = self.lookup(call, kind, provenance)?;
        let mut predicate = binding
            .cases
            .iter()
            .find(|(candidate, _)| candidate == value)
            .map(|(_, predicate)| predicate.clone())
            .ok_or_else(|| Error {
                kind: ErrorKind::UnknownVariant(format!("{value:?}")),
                provenance: provenance.clone(),
            })?;
        predicate.provenance = provenance.clone();
        Ok(predicate)
    }

    fn lower(
        &self,
        expression: &CompiledConditionExpression,
        provenance: &Source,
        depth: usize,
    ) -> Result<LoweredPredicate, Error> {
        if depth > MAX_COMPILED_CONDITION_DEPTH {
            return Err(Error {
                kind: ErrorKind::Unsupported("condition depth exceeds Recite limit"),
                provenance: provenance.clone(),
            });
        }
        let next_depth = depth.saturating_add(1);
        let mut predicate = match expression {
            CompiledConditionExpression::Call(call) => LoweredPredicate::leaf(self.case(
                call,
                ConditionExpectedType::Bool,
                &Value::Bool(true),
                provenance,
            )?),
            CompiledConditionExpression::Not(inner) => self
                .lower(inner, provenance, next_depth)?
                .negate(provenance)?,
            CompiledConditionExpression::And(children) => {
                self.group(children, provenance, next_depth, Predicate::and)?
            }
            CompiledConditionExpression::Or(children) => {
                self.group(children, provenance, next_depth, Predicate::or)?
            }
        };
        predicate.predicate.provenance = provenance.clone();
        Ok(predicate)
    }

    fn group(
        &self,
        children: &[CompiledConditionExpression],
        provenance: &Source,
        depth: usize,
        combine: fn(Predicate, Predicate) -> Predicate,
    ) -> Result<LoweredPredicate, Error> {
        let mut predicates = children
            .iter()
            .map(|child| self.lower(child, provenance, depth))
            .collect::<Result<Vec<_>, _>>()?;
        // Pair adjacent predicates to preserve short-circuit order without making
        // wide Recite groups into deep, left-associated core expression trees.
        while predicates.len() > 1 {
            predicates = pair_adjacent(predicates, combine, provenance)?;
        }
        predicates.pop().ok_or_else(|| Error {
            kind: ErrorKind::Unsupported("empty Boolean group"),
            provenance: provenance.clone(),
        })
    }
}

fn pair_adjacent(
    predicates: Vec<LoweredPredicate>,
    combine: fn(Predicate, Predicate) -> Predicate,
    provenance: &Source,
) -> Result<Vec<LoweredPredicate>, Error> {
    let mut next = Vec::new();
    let mut pairs = predicates.into_iter();
    while let Some(left) = pairs.next() {
        let mut predicate = if let Some(right) = pairs.next() {
            left.combine(right, combine, provenance)?
        } else {
            left
        };
        predicate.predicate.provenance = provenance.clone();
        next.push(predicate);
    }
    Ok(next)
}

// Every constructed subtree already satisfies the binary-depth bound, so an
// unsupported parent is rejected before creating a recursively unsafe tree.
struct LoweredPredicate {
    predicate: Predicate,
    depth: usize,
}

impl LoweredPredicate {
    const fn leaf(predicate: Predicate) -> Self {
        Self {
            predicate,
            depth: 0,
        }
    }

    fn negate(self, provenance: &Source) -> Result<Self, Error> {
        let depth = enclosing_depth(self.depth, provenance)?;
        Ok(Self {
            predicate: self.predicate.negate(),
            depth,
        })
    }

    fn combine(
        self,
        right: Self,
        combine: fn(Predicate, Predicate) -> Predicate,
        provenance: &Source,
    ) -> Result<Self, Error> {
        let depth = enclosing_depth(self.depth.max(right.depth), provenance)?;
        Ok(Self {
            predicate: combine(self.predicate, right.predicate),
            depth,
        })
    }
}

fn enclosing_depth(child_depth: usize, provenance: &Source) -> Result<usize, Error> {
    child_depth
        .checked_add(1)
        .filter(|depth| *depth <= MAX_COMPILED_CONDITION_DEPTH)
        .ok_or_else(|| Error {
            kind: ErrorKind::Unsupported("lowered binary predicate depth exceeds 128 edges"),
            provenance: provenance.clone(),
        })
}
