use leitmotif::{
    Fact, FactKind, ModelBuilder, PermittedEnding, PredicateExpression, ProgressionConclusion,
    ProgressionQuery, Source, Value,
};

use super::{LIMITS, Location, Place};

struct Setting;
impl Fact for Setting {
    type Value = bool;
    const KEY: &'static str = "setting";
    const KIND: FactKind = FactKind::Setting;
}

#[test]
fn reports_retain_query_identity_initial_settings_and_portable_sources_deterministically() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut builder = ModelBuilder::new("portable-progression")
        .unwrap()
        .with_source_root(root);
    let location = builder.fact::<Location>().unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder.build().unwrap();
    let initial = model
        .state([location.set(Place::Start), setting.set(true)])
        .unwrap();
    let mut terminal = location.is(Place::Exit);
    terminal.provenance = Source {
        file: format!("{root}/queries/ending.rs"),
        portable_file: None,
        line: 4,
        column: 2,
    };
    let query = ProgressionQuery::new(terminal.clone(), [PermittedEnding::new("finish", terminal)])
        .unwrap();
    let first = model.progression(&initial, &query, LIMITS).unwrap();
    let repeated = model.progression(&initial, &query, LIMITS).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&repeated).unwrap()
    );
    assert_eq!(first.model_id, model.fingerprint());
    assert_eq!(first.context, model.context());
    assert_eq!(first.initial, model.snapshot(&initial).unwrap());
    assert_eq!(
        first.initial.values.get("setting"),
        Some(&Value::Bool(true))
    );
    assert_eq!(first.schema_version, leitmotif::EXPERIMENTAL_SCHEMA_VERSION);
    assert_eq!(first.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        first.terminal.provenance.portable_file.as_deref(),
        Some("queries/ending.rs")
    );
    assert!(
        matches!(&first.terminal.expression, PredicateExpression::Eq(value) if value.fact == "location")
    );
    let ending = first.permitted_endings.first().unwrap();
    assert_eq!(ending.id, "finish");
    assert_eq!(
        ending.predicate.provenance.portable_file.as_deref(),
        Some("queries/ending.rs")
    );
    assert!(ending.provenance.portable_file.is_some());
    let ProgressionConclusion::Trap {
        entry,
        trapped_states,
    } = first.conclusion
    else {
        panic!("expected trap")
    };
    assert_eq!(trapped_states, 1);
    assert_eq!(
        entry.initial.values.get("setting"),
        Some(&Value::Bool(true))
    );
}
