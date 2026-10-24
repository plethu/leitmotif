//! Reproducible report inputs through the public API.
use leitmotif::{
    ClassificationLimits, Error, Fact, FactKind, Invariant, InvariantConclusion, ModelBuilder,
    Predicate, PredicateDescription, PredicateExpression, PredicateKind, Premise,
    ReachabilityConclusion, ReachabilityReport, SearchLimits, Source, Transition, Value,
};
struct Flag;
impl Fact for Flag {
    type Value = bool;
    const KEY: &'static str = "flag";
}
struct Setting;
impl Fact for Setting {
    type Value = bool;
    const KEY: &'static str = "setting";
    const KIND: FactKind = FactKind::Setting;
}
#[test]
fn witness_free_reports_retain_complete_initial_inputs() {
    let mut builder = ModelBuilder::new("report").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let setting = builder.fact::<Setting>().unwrap();
    let model = builder.build().unwrap();
    let initial = model.state([flag.set(false), setting.set(true)]).unwrap();
    for max_states in [0, 10] {
        let limits = SearchLimits {
            max_states,
            max_depth: 10,
        };
        let target = Predicate::constant(false);
        let report = model.reachability(&initial, &target, limits).unwrap();
        assert_eq!(
            report,
            model.reachability(&initial, &target, limits).unwrap()
        );
        assert_eq!(
            report.initial.values.get("setting"),
            Some(&Value::Bool(true))
        );
        assert_eq!(report.initial, model.snapshot(&initial).unwrap());
        assert!(report.model_id.starts_with("sha256:"));
        assert_eq!(report.tool_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            report.conclusion,
            if max_states == 0 {
                ReachabilityConclusion::Inconclusive
            } else {
                ReachabilityConclusion::Unreachable
            }
        );
        let safety = model.check_invariants(&initial, limits).unwrap();
        assert_eq!(safety.initial, report.initial);
        assert_eq!(safety.model_id, report.model_id);
        assert_eq!(
            safety.conclusion,
            if max_states == 0 {
                InvariantConclusion::Inconclusive
            } else {
                InvariantConclusion::Holds
            }
        );
    }
}

fn report_with_guard(root: &str, guard: bool) -> Result<ReachabilityReport, Error> {
    let mut builder = ModelBuilder::new("report")?.with_source_root(root);
    let flag = builder.fact::<Flag>()?;
    let mut predicate = Predicate::constant(guard);
    predicate.provenance.file = format!("{root}/models/story.rs");
    let model = builder
        .with_transition(
            Transition::new("event")
                .with_guard(predicate)
                .with_effect(flag.set(true)),
        )
        .build()?;
    let initial = model.state([flag.set(false)])?;
    model.reachability(
        &initial,
        &flag.is(true),
        SearchLimits {
            max_states: 10,
            max_depth: 10,
        },
    )
}

#[test]
fn model_identity_ignores_checkout_location_but_tracks_semantics() {
    let first = report_with_guard("/checkout/one", true).unwrap();
    let relocated = report_with_guard("/elsewhere/two", true).unwrap();
    assert_eq!(first.model_id, relocated.model_id);
    assert_ne!(
        first.model_id,
        report_with_guard("/checkout/one", false).unwrap().model_id
    );
    let ReachabilityConclusion::Reached { witness } = &first.conclusion else {
        panic!("expected witness")
    };
    let source = &witness.steps.first().unwrap().transition.guard.provenance;
    assert_eq!(source.file, "/checkout/one/models/story.rs");
    assert_eq!(source.portable_file.as_deref(), Some("models/story.rs"));
    let ReachabilityConclusion::Reached {
        witness: relocated_witness,
    } = &relocated.conclusion
    else {
        panic!("expected witness")
    };
    assert_eq!(
        source.portable_file,
        relocated_witness
            .steps
            .first()
            .unwrap()
            .transition
            .guard
            .provenance
            .portable_file
    );
}

#[test]
fn run_inputs_distinguish_changes_even_when_verdicts_agree() {
    let mut builder = ModelBuilder::new("report").unwrap();
    let flag = builder.fact::<Flag>().unwrap();
    let model = builder.build().unwrap();
    let limits = SearchLimits {
        max_states: 0,
        max_depth: 0,
    };
    let target = Predicate::constant(false);
    let initial = model.state([flag.set(false)]).unwrap();
    let changed = model.state([flag.set(true)]).unwrap();
    let first = model.reachability(&initial, &target, limits).unwrap();
    let second = model.reachability(&changed, &target, limits).unwrap();
    assert_eq!(first.model_id, second.model_id);
    assert_eq!(first.conclusion, second.conclusion);
    assert_ne!(first.initial, second.initial);
    let premise = Premise::new("observed", flag.is(false));
    let budget = ClassificationLimits { max_candidates: 0 };
    let classification = model.classify(&target, &[premise], budget).unwrap();
    let changed = model
        .classify(&target, &[Premise::new("observed", flag.is(true))], budget)
        .unwrap();
    assert_eq!(classification.model_id, first.model_id);
    assert_eq!(classification.conclusion, changed.conclusion);
    assert_ne!(classification.premises, changed.premises);
    assert_eq!(classification.tool_version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn external_source_locations_are_preserved_without_false_relative_aliases() {
    let model = ModelBuilder::new("report")
        .unwrap()
        .with_source_root("/checkout/root")
        .build()
        .unwrap();
    let initial = model.state([]).unwrap();
    let mut target = Predicate::constant(false);
    target.provenance = Source {
        file: "/checkout/root-other/external.rs".into(),
        portable_file: None,
        line: 3,
        column: 4,
    };
    let report = model
        .reachability(
            &initial,
            &target,
            SearchLimits {
                max_states: 0,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(report.target.provenance, target.provenance);
}

#[test]
fn model_identity_covers_declarations_and_authoring_order() {
    fn identity(builder: ModelBuilder) -> Result<String, Error> {
        let model = builder.build()?;
        Ok(model
            .classify(
                &Predicate::constant(true),
                &[],
                ClassificationLimits { max_candidates: 0 },
            )?
            .model_id)
    }
    let mut base = ModelBuilder::new("report").unwrap();
    base.fact::<Flag>().unwrap();
    let base_id = identity(base).unwrap();
    let mut renamed = ModelBuilder::new("renamed").unwrap();
    renamed.fact::<Flag>().unwrap();
    assert_ne!(base_id, identity(renamed).unwrap());
    let mut changed = ModelBuilder::new("report").unwrap();
    changed.fact::<Flag>().unwrap();
    assert_ne!(
        base_id,
        identity(changed.with_invariant(Invariant::new("rule", Predicate::constant(true))))
            .unwrap()
    );
    let mut changed = ModelBuilder::new("report").unwrap();
    let flag = changed.fact::<Flag>().unwrap();
    assert_ne!(
        base_id,
        identity(changed.with_transition(Transition::new("event").with_effect(flag.set(true))))
            .unwrap()
    );
    let mut first = ModelBuilder::new("report").unwrap();
    first.fact::<Flag>().unwrap();
    first.fact::<Setting>().unwrap();
    let mut reordered = ModelBuilder::new("report").unwrap();
    reordered.fact::<Setting>().unwrap();
    reordered.fact::<Flag>().unwrap();
    assert_ne!(identity(first).unwrap(), identity(reordered).unwrap());
}

#[test]
fn portable_sources_cover_nested_queries_premises_and_invariants() {
    fn absolute_sources(predicate: &mut Predicate) {
        predicate.provenance = Source {
            file: "/checkout/model.rs".into(),
            portable_file: None,
            line: 1,
            column: 1,
        };
        match &mut predicate.kind {
            PredicateKind::And(left, right) | PredicateKind::Or(left, right) => {
                absolute_sources(left);
                absolute_sources(right);
            }
            PredicateKind::Not(inner) => absolute_sources(inner),
            PredicateKind::Eq(_) | PredicateKind::Constant(_) => {}
        }
    }
    fn assert_portable_tree(predicate: &PredicateDescription) {
        assert_eq!(predicate.provenance.file, "/checkout/model.rs");
        assert_eq!(
            predicate.provenance.portable_file.as_deref(),
            Some("model.rs")
        );
        match &predicate.expression {
            PredicateExpression::And(left, right) | PredicateExpression::Or(left, right) => {
                assert_portable_tree(left);
                assert_portable_tree(right);
            }
            PredicateExpression::Not(inner) => assert_portable_tree(inner),
            PredicateExpression::Eq(_) | PredicateExpression::Constant(_) => {}
        }
    }

    let mut builder = ModelBuilder::new("report")
        .unwrap()
        .with_source_root("/checkout");
    let flag = builder.fact::<Flag>().unwrap();
    let mut query = flag.is(true).and(flag.is(false)).or(flag.is(true)).negate();
    absolute_sources(&mut query);
    let model = builder
        .with_invariant(Invariant::new("rule", query.clone()))
        .build()
        .unwrap();
    let report = model
        .classify(
            &query,
            &[Premise::new("known", query.clone())],
            ClassificationLimits { max_candidates: 0 },
        )
        .unwrap();
    assert_portable_tree(&report.hypothesis);
    assert_portable_tree(&report.premises.first().unwrap().predicate);
    let initial = model.state([flag.set(false)]).unwrap();
    let safety = model
        .check_invariants(
            &initial,
            SearchLimits {
                max_states: 0,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_portable_tree(&safety.invariants.first().unwrap().predicate);
}

#[test]
fn portable_sources_normalize_parents_without_escaping_the_root() {
    let model = ModelBuilder::new("report")
        .unwrap()
        .with_source_root("/checkout/root")
        .build()
        .unwrap();
    let initial = model.state([]).unwrap();
    for (file, expected) in [
        ("/checkout/root/inside/../model.rs", Some("model.rs")),
        ("/checkout/root/../outside.rs", None),
        ("/checkout/root/inside/../../outside.rs", None),
    ] {
        let mut target = Predicate::constant(false);
        target.provenance = Source {
            file: file.into(),
            portable_file: None,
            line: 1,
            column: 1,
        };
        let report = model
            .reachability(
                &initial,
                &target,
                SearchLimits {
                    max_states: 0,
                    max_depth: 0,
                },
            )
            .unwrap();
        assert_eq!(report.target.provenance.file, file);
        assert_eq!(report.target.provenance.portable_file.as_deref(), expected);
    }
}
