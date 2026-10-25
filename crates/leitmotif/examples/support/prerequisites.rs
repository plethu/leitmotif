use leitmotif::{Error, Fact, FactKind, Model, ModelBuilder, Predicate, Premise};
use std::error::Error as StdError;

struct Flag<const INDEX: usize>;
macro_rules! flags {
    ($($index:literal => $key:literal),+ $(,)?) => {
        impl<const INDEX: usize> Fact for Flag<INDEX> {
            type Value = bool;
            const KEY: &'static str = match INDEX { $($index => $key,)+ _ => "invalid" };
            const KIND: FactKind = FactKind::Setting;
        }
        fn declare(builder: &mut ModelBuilder, count: usize) -> Result<Vec<Predicate>, Error> {
            let mut flags = Vec::new();
            $(if flags.len() < count { flags.push(builder.fact::<Flag<$index>>()?.is(true)); })+
            Ok(flags)
        }
    }
}
flags! {
    0 => "prerequisite.0", 1 => "prerequisite.1", 2 => "prerequisite.2",
    3 => "prerequisite.3", 4 => "prerequisite.4", 5 => "prerequisite.5",
    6 => "prerequisite.6", 7 => "prerequisite.7", 8 => "prerequisite.8",
    9 => "prerequisite.9", 10 => "prerequisite.10", 11 => "prerequisite.11",
    12 => "prerequisite.12", 13 => "prerequisite.13", 14 => "prerequisite.14",
    15 => "prerequisite.15", 16 => "prerequisite.16", 17 => "prerequisite.17",
    18 => "prerequisite.18", 19 => "prerequisite.19", 20 => "prerequisite.20",
    21 => "prerequisite.21", 22 => "prerequisite.22", 23 => "prerequisite.23",
}
pub(super) struct Prerequisites {
    pub(super) model: Model,
    pub(super) hypothesis: Predicate,
    pub(super) premises: Vec<Premise>,
}
impl Prerequisites {
    pub(super) fn new(count: usize, query: super::Query) -> Result<Self, Box<dyn StdError>> {
        if !(1..=24).contains(&count) {
            return Err("facts must be between 1 and 24".into());
        }
        let mut builder = ModelBuilder::new("prerequisites")?;
        let flags = declare(&mut builder, count)?;
        let hypothesis = flags.first().ok_or("missing first prerequisite")?.clone();
        let (hypothesis, premises) = match query {
            super::Query::Possible => (
                hypothesis,
                flags
                    .windows(2)
                    .enumerate()
                    .map(|(index, pair)| {
                        let mut pair = pair.iter();
                        let left = pair.next().ok_or("missing left prerequisite")?;
                        let right = pair.next().ok_or("missing right prerequisite")?;
                        Ok(Premise::new(
                            format!("requires.{index}"),
                            left.clone().implies(right.clone()),
                        ))
                    })
                    .collect::<Result<Vec<_>, Box<dyn StdError>>>()?,
            ),
            super::Query::Required | super::Query::RuledOut | super::Query::Inconsistent => {
                let mut premises: Vec<_> = flags
                    .iter()
                    .enumerate()
                    .map(|(index, flag)| Premise::new(format!("evidence.{index}"), flag.clone()))
                    .collect();
                if matches!(query, super::Query::Inconsistent) {
                    premises.push(Premise::new(
                        "conflict",
                        flags
                            .last()
                            .ok_or("missing last prerequisite")?
                            .clone()
                            .negate(),
                    ));
                }
                (
                    if matches!(query, super::Query::RuledOut) {
                        hypothesis.negate()
                    } else {
                        hypothesis
                    },
                    premises,
                )
            }
        };
        Ok(Self {
            model: builder.build()?,
            hypothesis,
            premises,
        })
    }
}
