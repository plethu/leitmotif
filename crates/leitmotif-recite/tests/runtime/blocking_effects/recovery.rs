use leitmotif_recite::{BlockingEffects, EffectErrorKind};
use recite_core::EffectId;
use recite_runtime::{
    DialogueError, DialogueEvent, EffectAck, acknowledge_effect, next,
    snapshot::{
        decode_session_messagepack, encode_session_messagepack, restore_session, snapshot_session,
    },
    start_scene,
};

use super::{STORY, compile, grant_once_host};

#[test]
fn restored_pending_requests_reemit_once_despite_byte_equal_snapshots() {
    let asset = compile(STORY).unwrap();
    let fixture = grant_once_host().unwrap();
    let mut original = start_scene(&asset, None).unwrap();
    let context = fixture.conditions.context(&fixture.model, &fixture.initial);
    let DialogueEvent::Effect(request) = next(&asset, &mut original, &context).unwrap() else {
        panic!("expected request")
    };
    let saved = snapshot_session(&original);
    let bytes = encode_session_messagepack(&original).unwrap();
    let mut restored = decode_session_messagepack(&asset, &bytes).unwrap();
    assert_eq!(snapshot_session(&restored), saved);
    assert_eq!(encode_session_messagepack(&restored).unwrap(), bytes);
    assert!(matches!(
        next(&asset, &mut original, &context),
        Err(DialogueError::EffectPending { .. })
    ));
    assert!(matches!(
        acknowledge_effect(
            &mut restored,
            EffectId::new("wrong-request").unwrap(),
            EffectAck::Completed
        ),
        Err(DialogueError::WrongEffectAcknowledgement { .. })
    ));
    assert_eq!(snapshot_session(&restored), saved);
    let before = snapshot_session(&restored);
    let DialogueEvent::Effect(redelivered) = next(&asset, &mut restored, &context).unwrap() else {
        panic!("expected one restored delivery")
    };
    assert_eq!(redelivered, request);
    assert_eq!(snapshot_session(&restored), before);
    assert_eq!(encode_session_messagepack(&restored).unwrap(), bytes);
    assert!(matches!(
        next(&asset, &mut restored, &context),
        Err(DialogueError::EffectPending { .. })
    ));
    assert_eq!(snapshot_session(&restored), before);
}

#[test]
fn saved_host_assignment_and_domain_guard_prevent_double_application_after_restore() {
    let asset = compile(STORY).unwrap();
    let fixture = grant_once_host().unwrap();
    let mut effects = BlockingEffects::new(&fixture.model);
    effects
        .bind(asset.effects.first().unwrap(), "grant")
        .unwrap();
    for acknowledgement in [
        EffectAck::Completed,
        EffectAck::Failed {
            reason: "host committed before save".into(),
        },
    ] {
        let mut original = start_scene(&asset, None).unwrap();
        let DialogueEvent::Effect(request) = next(
            &asset,
            &mut original,
            &fixture.conditions.context(&fixture.model, &fixture.initial),
        )
        .unwrap() else {
            panic!("expected request")
        };
        let committed = effects
            .apply_pending(&original, &fixture.initial)
            .unwrap()
            .into_state();
        let saved_host = fixture.model.snapshot(&committed).unwrap();
        let saved_dialogue = snapshot_session(&original);
        let mut restored = restore_session(&asset, saved_dialogue.clone()).unwrap();
        let context = fixture.conditions.context(&fixture.model, &committed);
        let DialogueEvent::Effect(reemitted) = next(&asset, &mut restored, &context).unwrap()
        else {
            panic!("expected same pending request")
        };
        assert_eq!(reemitted, request);
        assert_eq!(snapshot_session(&restored), saved_dialogue);
        assert!(
            fixture
                .model
                .evaluate(&committed, &fixture.granted.is(true))
                .unwrap()
        );
        assert!(matches!(
            effects
                .apply_pending(&restored, &committed)
                .unwrap_err()
                .kind,
            EffectErrorKind::DisabledTransition(_)
        ));
        assert_eq!(fixture.model.snapshot(&committed).unwrap(), saved_host);
        acknowledge_effect(&mut restored, request.id, acknowledgement).unwrap();
        assert_eq!(fixture.model.snapshot(&committed).unwrap(), saved_host);
        let DialogueEvent::Line(line) = next(&asset, &mut restored, &context).unwrap() else {
            panic!("expected continuation using saved host state")
        };
        assert_eq!(line.text, "Changed.");
    }
}
