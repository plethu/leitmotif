use crate::adapter::error::caller;
use leitmotif::{Fact, ModelBuilder, Transition};
use recite_core::{EffectId, SourcePosition, SourceSpan, compiled::SourceMapIndex};

use super::{
    CompiledArgument, CompiledEffect, CompiledEffectMode, DialogueEffectArgument,
    DialogueEffectMode, DialogueEffectRequest, Registry, ScalarValue, matching_binding,
};

struct Granted;
impl Fact for Granted {
    type Value = bool;
    const KEY: &'static str = "granted";
}

#[test]
fn fixed_registry_bounds_exact_signature_probes_and_applies_the_declared_transition() {
    let mut builder = ModelBuilder::new("registry-cost").unwrap();
    let granted = builder.fact::<Granted>().unwrap();
    let model = builder
        .with_transition(Transition::new("grant").with_effect(granted.set(true)))
        .build()
        .unwrap();
    let initial = model.state([granted.set(false)]).unwrap();
    let mut registry = Registry::new(&model);
    for index in 0..128 {
        let effect = CompiledEffect {
            id: EffectId::new(format!("effect-{index}")).unwrap(),
            mode: CompiledEffectMode::Immediate,
            function: format!("grant_{index}"),
            args: vec![
                CompiledArgument::Identifier("player".into()),
                CompiledArgument::Value(ScalarValue::Integer(3)),
            ],
            source_map: SourceMapIndex::new(0),
        };
        registry.bind(&effect, "grant", caller()).unwrap();
    }
    let mut request = DialogueEffectRequest {
        id: EffectId::new("opaque-request").unwrap(),
        mode: DialogueEffectMode::Immediate,
        function: "grant_127".into(),
        args: vec![
            DialogueEffectArgument::Identifier("player".into()),
            DialogueEffectArgument::Integer(3),
        ],
        source_span: SourceSpan::point("story.recite", SourcePosition::new(2, 1).unwrap()),
    };
    let mut probes = 0;
    let matched =
        matching_binding(registry.bindings.iter().inspect(|_| probes += 1), &request).unwrap();
    assert_eq!(probes, 128);
    assert_eq!(matched.transition.id, "grant");
    let applied = registry.apply(&request, &initial).unwrap();
    assert_eq!(applied.request(), &request);
    let successor = applied.into_state();
    assert_eq!(
        model.snapshot(&successor).unwrap(),
        model
            .snapshot(&model.state([granted.set(true)]).unwrap())
            .unwrap()
    );
    assert!(model.evaluate(&initial, &granted.is(false)).unwrap());

    request.function = "grant_0".into();
    probes = 0;
    assert!(
        matching_binding(registry.bindings.iter().inspect(|_| probes += 1), &request).is_some()
    );
    assert_eq!(probes, 1);
    request.function = "missing".into();
    probes = 0;
    assert!(
        matching_binding(registry.bindings.iter().inspect(|_| probes += 1), &request).is_none()
    );
    assert_eq!(probes, 128);
}
