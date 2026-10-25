use std::error::Error;

use leitmotif::{FactHandle, Model, ModelBuilder, State, Transition};
use leitmotif_recite::{Bindings, BlockingEffects, EffectErrorKind};
use recite_core::{
    EffectId,
    compiled::{CompiledConditionCall, CompiledEffectMode},
};
use recite_runtime::{
    DialogueEffectMode, DialogueError, DialogueEvent, EffectAck, acknowledge_effect, next,
    snapshot::snapshot_session, start_scene,
};

use super::support::{Enabled, compile};

const STORY: &str = concat!(
    ":: start default\n",
    "! blocking grant(player)\n",
    ":if ready()\n",
    "  > yes@8843fd6f53f020a12b31\n    Changed.\n",
    ":else\n  > no@4dff358182d02d9090e3\n    Unchanged.\n",
    "-> END\n",
);

#[test]
fn pending_application_host_commit_and_acknowledgement_are_distinct_boundaries() {
    let asset = compile(STORY).unwrap();
    let mut builder = ModelBuilder::new("host").unwrap();
    let enabled = builder.fact::<Enabled>().unwrap();
    let model = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(enabled.is(false))
                .with_effect(enabled.set(true)),
        )
        .build()
        .unwrap();
    let mut host = model.state([enabled.set(false)]).unwrap();
    let mut conditions = Bindings::new();
    conditions
        .bind_bool(
            CompiledConditionCall {
                function: "ready".into(),
                args: vec![],
            },
            &enabled,
        )
        .unwrap();
    let mut effects = BlockingEffects::new(&model);
    effects
        .bind(asset.effects.first().unwrap(), "grant")
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::Effect(emitted) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected pending request")
    };
    assert_eq!(session.pending_effect(), Some(&emitted));
    assert!(model.evaluate(&host, &enabled.is(false)).unwrap());
    assert!(matches!(
        next(&asset, &mut session, &conditions.context(&model, &host)),
        Err(DialogueError::EffectPending { .. })
    ));
    let applied = effects.apply_pending(&session, &host).unwrap();
    assert_eq!(applied.request(), &emitted);
    assert_eq!(applied.transition().id, "grant");
    host = applied.into_state();
    assert!(session.pending_effect().is_some());
    assert!(model.evaluate(&host, &enabled.is(true)).unwrap());
    acknowledge_effect(&mut session, emitted.id, EffectAck::Completed).unwrap();
    assert!(session.pending_effect().is_none());
    let DialogueEvent::Line(line) =
        next(&asset, &mut session, &conditions.context(&model, &host)).unwrap()
    else {
        panic!("expected continuation")
    };
    assert_eq!(line.text, "Changed.");
}

struct GrantHost {
    model: Model,
    granted: FactHandle<Enabled>,
    initial: State,
    conditions: Bindings,
}

fn grant_once_host() -> Result<GrantHost, Box<dyn Error>> {
    let mut builder = ModelBuilder::new("grant-once")?;
    let granted = builder.fact::<Enabled>()?;
    let model = builder
        .with_transition(
            Transition::new("grant")
                .with_guard(granted.is(false))
                .with_effect(granted.set(true)),
        )
        .build()?;
    let initial = model.state([granted.set(false)])?;
    let mut conditions = Bindings::new();
    conditions.bind_bool(
        CompiledConditionCall {
            function: "ready".into(),
            args: vec![],
        },
        &granted,
    )?;
    Ok(GrantHost {
        model,
        granted,
        initial,
        conditions,
    })
}

#[test]
fn acknowledgement_outcomes_resume_without_mutating_host_or_selecting_the_branch() {
    let asset = compile(STORY).unwrap();
    let fixture = grant_once_host().unwrap();
    let mut effects = BlockingEffects::new(&fixture.model);
    effects
        .bind(asset.effects.first().unwrap(), "grant")
        .unwrap();
    for (apply, acknowledgement) in [
        (false, EffectAck::Completed),
        (true, EffectAck::Completed),
        (
            false,
            EffectAck::Failed {
                reason: "no host write".into(),
            },
        ),
        (
            true,
            EffectAck::Failed {
                reason: "host already changed".into(),
            },
        ),
    ] {
        let mut host = fixture.initial.clone();
        let mut session = start_scene(&asset, None).unwrap();
        let DialogueEvent::Effect(request) = next(
            &asset,
            &mut session,
            &fixture.conditions.context(&fixture.model, &host),
        )
        .unwrap() else {
            panic!("expected request")
        };
        if apply {
            host = effects.apply_pending(&session, &host).unwrap().into_state();
        }
        let before_ack = fixture.model.snapshot(&host).unwrap();
        let pending_position = snapshot_session(&session);
        acknowledge_effect(&mut session, request.id, acknowledgement).unwrap();
        assert_eq!(fixture.model.snapshot(&host).unwrap(), before_ack);
        assert_eq!(
            snapshot_session(&session).next_statement,
            pending_position.next_statement
        );
        assert!(session.pending_effect().is_none());
        let DialogueEvent::Line(line) = next(
            &asset,
            &mut session,
            &fixture.conditions.context(&fixture.model, &host),
        )
        .unwrap() else {
            panic!("expected continuation")
        };
        assert_eq!(line.text, if apply { "Changed." } else { "Unchanged." });
    }
}

#[test]
fn invalid_acknowledgements_and_modified_emitted_copies_cannot_replace_pending_work() {
    let asset = compile(STORY).unwrap();
    let fixture = grant_once_host().unwrap();
    let mut effects = BlockingEffects::new(&fixture.model);
    effects
        .bind(asset.effects.first().unwrap(), "grant")
        .unwrap();
    let mut session = start_scene(&asset, None).unwrap();
    let wrong = EffectId::new("wrong-request").unwrap();
    assert!(matches!(
        acknowledge_effect(&mut session, wrong.clone(), EffectAck::Completed),
        Err(DialogueError::NoEffectPending { .. })
    ));
    assert!(matches!(
        effects
            .apply_pending(&session, &fixture.initial)
            .unwrap_err()
            .kind,
        EffectErrorKind::NoPending
    ));
    let DialogueEvent::Effect(mut emitted) = next(
        &asset,
        &mut session,
        &fixture.conditions.context(&fixture.model, &fixture.initial),
    )
    .unwrap() else {
        panic!("expected request")
    };
    let exact = emitted.clone();
    let before = snapshot_session(&session);
    assert!(matches!(
        acknowledge_effect(
            &mut session,
            wrong.clone(),
            EffectAck::Failed {
                reason: "wrong id".into()
            }
        ),
        Err(DialogueError::WrongEffectAcknowledgement { .. })
    ));
    assert_eq!(snapshot_session(&session), before);
    emitted.id = wrong;
    emitted.mode = DialogueEffectMode::Immediate;
    emitted.function = "other".into();
    emitted.args.clear();
    let applied = effects.apply_pending(&session, &fixture.initial).unwrap();
    assert_eq!(applied.request(), &exact);
    assert_ne!(applied.request(), &emitted);
    let successor = applied.into_state();
    assert!(
        fixture
            .model
            .evaluate(&successor, &fixture.granted.is(true))
            .unwrap()
    );
    assert_eq!(snapshot_session(&session), before);
    acknowledge_effect(&mut session, exact.id.clone(), EffectAck::Completed).unwrap();
    assert!(matches!(
        acknowledge_effect(&mut session, exact.id, EffectAck::Completed),
        Err(DialogueError::NoEffectPending { .. })
    ));
    assert!(matches!(
        effects
            .apply_pending(&session, &successor)
            .unwrap_err()
            .kind,
        EffectErrorKind::NoPending
    ));
}

#[test]
fn failed_host_application_preserves_pending_request_and_pre_state() {
    let asset = compile(STORY).unwrap();
    let fixture = grant_once_host().unwrap();
    let mut effects = BlockingEffects::new(&fixture.model);
    let declaration = asset.effects.first().unwrap();
    for (compiled_mode, actual) in [
        (CompiledEffectMode::Immediate, DialogueEffectMode::Immediate),
        (CompiledEffectMode::Deferred, DialogueEffectMode::Deferred),
    ] {
        let mut wrong = declaration.clone();
        wrong.mode = compiled_mode;
        assert!(
            matches!(effects.bind(&wrong, "grant").unwrap_err().kind, EffectErrorKind::WrongMode { expected: DialogueEffectMode::Blocking, actual: found } if found == actual)
        );
    }
    let mut session = start_scene(&asset, None).unwrap();
    let DialogueEvent::Effect(request) = next(
        &asset,
        &mut session,
        &fixture.conditions.context(&fixture.model, &fixture.initial),
    )
    .unwrap() else {
        panic!("expected request")
    };
    let before = snapshot_session(&session);
    let pre_state = fixture.model.snapshot(&fixture.initial).unwrap();
    let unmapped = effects
        .apply_pending(&session, &fixture.initial)
        .unwrap_err();
    assert!(matches!(unmapped.kind, EffectErrorKind::Unmapped(_)));
    assert_eq!(
        (unmapped.provenance.file.as_str(), unmapped.provenance.line),
        (
            request.source_span.file.as_str(),
            request.source_span.start.line()
        )
    );
    effects.bind(declaration, "grant").unwrap();
    let foreign_model = ModelBuilder::new("foreign").unwrap().build().unwrap();
    let foreign = foreign_model.state([]).unwrap();
    assert!(matches!(
        effects.apply_pending(&session, &foreign).unwrap_err().kind,
        EffectErrorKind::Host(_)
    ));
    let granted = effects
        .apply_pending(&session, &fixture.initial)
        .unwrap()
        .into_state();
    assert!(matches!(
        effects.apply_pending(&session, &granted).unwrap_err().kind,
        EffectErrorKind::DisabledTransition(_)
    ));
    assert_eq!(snapshot_session(&session), before);
    assert_eq!(fixture.model.snapshot(&fixture.initial).unwrap(), pre_state);
}

#[path = "blocking_effects/recovery.rs"]
mod recovery;
