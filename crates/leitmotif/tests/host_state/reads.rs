use std::fmt::Debug;

use leitmotif::{Domain, Error, ErrorKind, Fact, FactHandle, Model, State};

use super::{Harbour, Weather, harbour};

/// Domain values where `get` and `evaluate(is(value))` disagree.
fn evaluate_disagreements<F>(
    model: &Model,
    state: &State,
    fact: &FactHandle<F>,
) -> Result<Vec<String>, Error>
where
    F: Fact,
    F::Value: PartialEq + Debug,
{
    let read = model.get(state, fact)?;
    let mut disagreements = Vec::new();
    for value in F::Value::values() {
        if (read == value) != model.evaluate(state, &fact.is(value.clone()))? {
            disagreements.push(format!("{} = {value:?}", F::KEY));
        }
    }
    Ok(disagreements)
}

fn every_fact_disagreement(harbour: &Harbour, state: &State) -> Result<Vec<String>, Error> {
    let mut disagreements = evaluate_disagreements(&harbour.model, state, &harbour.learned)?;
    disagreements.extend(evaluate_disagreements(&harbour.model, state, &harbour.sky)?);
    disagreements.extend(evaluate_disagreements(
        &harbour.model,
        state,
        &harbour.hardcore,
    )?);
    Ok(disagreements)
}

#[test]
fn reads_return_assigned_and_applied_values_including_settings() {
    let harbour = harbour().unwrap();
    let model = &harbour.model;
    let stormy = harbour.stormy_state().unwrap();
    assert!(!model.get(&stormy, &harbour.learned).unwrap());
    assert_eq!(model.get(&stormy, &harbour.sky).unwrap(), Weather::Storm);
    assert!(model.get(&stormy, &harbour.hardcore).unwrap());

    let calmed = model.apply(&stormy, "storm.break").unwrap().unwrap();
    assert!(model.get(&calmed, &harbour.learned).unwrap());
    assert_eq!(model.get(&calmed, &harbour.sky).unwrap(), Weather::Calm);
    assert!(model.get(&calmed, &harbour.hardcore).unwrap());
}

#[test]
fn reads_agree_with_evaluate_across_every_domain_value() {
    let harbour = harbour().unwrap();
    let stormy = harbour.stormy_state().unwrap();
    let calmed = harbour
        .model
        .apply(&stormy, "storm.break")
        .unwrap()
        .unwrap();
    assert_eq!(
        every_fact_disagreement(&harbour, &stormy).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        every_fact_disagreement(&harbour, &calmed).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn reads_reject_foreign_states_and_handles() {
    let home = harbour().unwrap();
    let other = harbour().unwrap();
    let state = home.stormy_state().unwrap();
    let foreign = other.stormy_state().unwrap();
    let state_line = line!() + 1;
    let error = home.model.get(&foreign, &home.sky).unwrap_err();
    assert_eq!(error.kind, ErrorKind::ForeignState);
    assert_eq!(error.provenance.line, state_line);
    let handle_line = line!() + 1;
    let error = home.model.get(&state, &other.sky).unwrap_err();
    assert_eq!(error.kind, ErrorKind::ForeignHandle("sky".to_owned()));
    assert_eq!(error.provenance.line, handle_line);
}

#[test]
fn handles_for_thread_safe_values_can_cross_threads() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let harbour = harbour().unwrap();
    assert_send_sync(&harbour.sky);
    assert_send_sync(&harbour.learned);
}

#[test]
fn reads_do_not_allocate() {
    let harbour = harbour().unwrap();
    let state = harbour.stormy_state().unwrap();
    let model = &harbour.model;
    let mut every_read_matched = true;
    let measured = allocation_counter::measure(|| {
        for _ in 0..64 {
            every_read_matched &= model.get(&state, &harbour.sky).unwrap() == Weather::Storm
                && !model.get(&state, &harbour.learned).unwrap()
                && model.get(&state, &harbour.hardcore).unwrap();
        }
    });
    assert!(every_read_matched);
    assert_eq!(measured.count_total, 0);
}
