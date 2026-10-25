use super::super::{Model, compile::CompiledPredicate};
use rsmt2::{SmtRes, print::Expr2Smt};
use std::io::Write;

pub(super) struct Expression<'a> {
    pub(super) model: &'a Model,
    pub(super) predicate: &'a CompiledPredicate,
    pub(super) negate: bool,
}
impl Expr2Smt for Expression<'_> {
    fn expr_to_smt2<W: Write>(&self, writer: &mut W, (): ()) -> SmtRes<()> {
        if self.negate {
            write!(writer, "(not ")?;
        }
        write_predicate(writer, self.model, self.predicate)?;
        if self.negate {
            write!(writer, ")")?;
        }
        Ok(())
    }
}
fn write_predicate(
    writer: &mut impl Write,
    model: &Model,
    predicate: &CompiledPredicate,
) -> SmtRes<()> {
    match predicate {
        CompiledPredicate::Constant(value) => write!(writer, "{value}")?,
        CompiledPredicate::Eq(index, value) => {
            let ordinal = model
                .facts
                .get(*index)
                .and_then(|fact| fact.domain.iter().position(|candidate| candidate == value))
                .ok_or("validated equality has no finite ordinal")?;
            write!(writer, "(= f{index} {ordinal})")?;
        }
        CompiledPredicate::And(left, right) => write_binary(writer, model, "and", left, right)?,
        CompiledPredicate::Or(left, right) => write_binary(writer, model, "or", left, right)?,
        CompiledPredicate::Not(child) => {
            write!(writer, "(not ")?;
            write_predicate(writer, model, child)?;
            write!(writer, ")")?;
        }
    }
    Ok(())
}
fn write_binary(
    writer: &mut impl Write,
    model: &Model,
    operation: &str,
    left: &CompiledPredicate,
    right: &CompiledPredicate,
) -> SmtRes<()> {
    write!(writer, "({operation} ")?;
    write_predicate(writer, model, left)?;
    write!(writer, " ")?;
    write_predicate(writer, model, right)?;
    write!(writer, ")")?;
    Ok(())
}
