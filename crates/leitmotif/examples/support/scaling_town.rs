use std::{error::Error, iter};

use leitmotif::{
    Assignment, Domain, Fact, FactHandle, FactKind, Model, ModelBuilder, Predicate, State,
    Transition, Value,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Shared-resource policy for the synthetic family.
pub enum Resource {
    /// Threads advance without competing for a permit.
    Unlimited,
    /// One thread holds the permit until it returns to idle.
    SinglePermit,
}

#[derive(Clone, Copy, Debug, Serialize)]
/// Validated dimensions of one synthetic town.
pub struct Parameters {
    threads: usize,
    local_domain: usize,
    coupling: usize,
    resource: Resource,
}

impl Parameters {
    pub(super) fn new(
        threads: usize,
        local_domain: usize,
        coupling: usize,
        resource: Resource,
    ) -> Result<Self, &'static str> {
        if !(1..=16).contains(&threads) {
            return Err("threads must be between 1 and 16");
        }
        if !(2..=8).contains(&local_domain) {
            return Err("local domain must be between 2 and 8");
        }
        if coupling >= threads {
            return Err("coupling must be smaller than the thread count");
        }
        Ok(Self {
            threads,
            local_domain,
            coupling,
            resource,
        })
    }
}

#[derive(Clone)]
struct Phase<const SIZE: usize>(usize);

impl<const SIZE: usize> Domain for Phase<SIZE> {
    fn values() -> Vec<Self> {
        (0..SIZE).map(Self).collect()
    }

    fn encode(self) -> Value {
        Value::Symbol(self.0.to_string())
    }
}

macro_rules! declare_threads {
    ($($name:ident => $key:literal),+ $(,)?) => {
        $(struct $name<const SIZE: usize>;
        impl<const SIZE: usize> Fact for $name<SIZE> {
            type Value = Phase<SIZE>;
            const KEY: &'static str = $key;
        })+

        fn register_threads<const SIZE: usize>(
            builder: &mut ModelBuilder,
            count: usize,
        ) -> Result<Vec<Thread>, Box<dyn Error>> {
            let mut threads = Vec::new();
            $(if threads.len() < count {
                threads.push(register::<$name<SIZE>, SIZE>(builder)?);
            })+
            Ok(threads)
        }
    };
}

declare_threads! {
    Thread0 => "town.thread.0", Thread1 => "town.thread.1",
    Thread2 => "town.thread.2", Thread3 => "town.thread.3",
    Thread4 => "town.thread.4", Thread5 => "town.thread.5",
    Thread6 => "town.thread.6", Thread7 => "town.thread.7",
    Thread8 => "town.thread.8", Thread9 => "town.thread.9",
    Thread10 => "town.thread.10", Thread11 => "town.thread.11",
    Thread12 => "town.thread.12", Thread13 => "town.thread.13",
    Thread14 => "town.thread.14", Thread15 => "town.thread.15",
}

struct Permit;
impl Fact for Permit {
    type Value = bool;
    const KEY: &'static str = "town.permit_available";
}

struct ClosedEnding;
impl Fact for ClosedEnding {
    type Value = bool;
    const KEY: &'static str = "town.closed_ending";
    const KIND: FactKind = FactKind::Setting;
}

struct Step {
    from: usize,
    to: usize,
    transition: Transition,
}

impl Step {
    fn constrained_transition(
        &self,
        entry: &Predicate,
        resource: Resource,
        permit: &FactHandle<Permit>,
    ) -> Transition {
        let mut transition = self.transition.clone();
        if self.from == 0 {
            transition = transition.with_guard(entry.clone());
        }
        if resource == Resource::SinglePermit && (self.from == 0 || self.to == 0) {
            transition = transition.with_effect(permit.set(self.to == 0));
        }
        transition
    }
}

struct Thread {
    idle: Predicate,
    finished: Predicate,
    initial: Assignment,
    steps: Vec<Step>,
}

fn register<F: Fact<Value = Phase<SIZE>>, const SIZE: usize>(
    builder: &mut ModelBuilder,
) -> Result<Thread, Box<dyn Error>> {
    let fact = builder.fact::<F>()?;
    let steps = (0..SIZE)
        .zip((1..SIZE).chain(iter::once(0)))
        .map(|(from, to)| Step {
            from,
            to,
            transition: Transition::new(format!("{}.{}", F::KEY, from))
                .with_guard(fact.is(Phase(from)))
                .with_effect(fact.set(Phase(to))),
        })
        .collect();
    Ok(Thread {
        idle: fact.is(Phase(0)),
        finished: fact.is(Phase(SIZE.checked_sub(1).ok_or("empty domain")?)),
        initial: fact.set(Phase(0)),
        steps,
    })
}

/// A cyclic synthetic graph with a collective goal and a closed ending.
pub struct Town {
    pub(super) model: Model,
    pub(super) initial: State,
    pub(super) goal: Predicate,
    pub(super) unreachable: Predicate,
}

impl Town {
    pub(super) fn new(parameters: Parameters) -> Result<Self, Box<dyn Error>> {
        match parameters.local_domain {
            2 => build::<2>(parameters),
            3 => build::<3>(parameters),
            4 => build::<4>(parameters),
            5 => build::<5>(parameters),
            6 => build::<6>(parameters),
            7 => build::<7>(parameters),
            8 => build::<8>(parameters),
            _ => Err("unsupported local domain".into()),
        }
    }
}

fn build<const SIZE: usize>(parameters: Parameters) -> Result<Town, Box<dyn Error>> {
    let mut builder = ModelBuilder::new("synthetic_town")?;
    let permit = builder.fact::<Permit>()?;
    let closed = builder.fact::<ClosedEnding>()?;
    let threads = register_threads::<SIZE>(&mut builder, parameters.threads)?;
    let mut initial = vec![permit.set(true), closed.set(false)];
    let mut goal = Predicate::constant(true);
    for (index, thread) in threads.iter().enumerate() {
        initial.push(thread.initial.clone());
        goal = goal.and(thread.finished.clone());
        let mut entry = thread.idle.clone();
        for neighbor in threads
            .iter()
            .cycle()
            .skip(index)
            .skip(1)
            .take(parameters.coupling)
        {
            entry = entry.and(neighbor.idle.clone());
        }
        if parameters.resource == Resource::SinglePermit {
            entry = entry.and(permit.is(true));
        }
        for step in &thread.steps {
            builder = builder.with_transition(step.constrained_transition(
                &entry,
                parameters.resource,
                &permit,
            ));
        }
    }
    let model = builder.build()?;
    Ok(Town {
        initial: model.state(initial)?,
        model,
        goal,
        unreachable: closed.is(true),
    })
}
