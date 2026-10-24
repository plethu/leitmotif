use super::*;
use claims::{Attitude, Belief, Evidence, EvidenceId, Exposure, Reference, Statement};
use leitmotif::{Domain, HypothesisConclusion};
use observer::{Basis, ObserverError, Request};

fn reveal(story: &mut Story, observer: Person) {
    story
        .exposures
        .push(Exposure::new(observer, EvidenceId::CoatPortrait));
}
fn resolve() -> Request {
    Request::new("accept_testimony_and_portrait", Basis::CoatFromPortrait)
}

#[test]
fn observer_cannot_resolve_hidden_or_other_observers_coat_identity() {
    let mut story = Story::new().unwrap();
    for budget in [0, 8] {
        let error = story
            .observe(Person::Orin, &[resolve()], budget)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ObserverError::Inaccessible {
                observer: Person::Orin,
                evidence: EvidenceId::CoatPortrait,
                ..
            }
        ));
    }
    reveal(&mut story, Person::Mira);
    assert!(matches!(
        story.observe(Person::Orin, &[resolve()], 8),
        Err(ObserverError::Inaccessible {
            evidence: EvidenceId::CoatPortrait,
            ..
        })
    ));
    // A visible identity does not make the underlying unencountered testimony accessible.
    assert!(matches!(
        story.observe(Person::Mira, &[resolve()], 0),
        Err(ObserverError::Inaccessible {
            evidence: EvidenceId::Testimony(ClaimId::OrinSawCoat),
            ..
        })
    ));
}

#[test]
fn two_complete_explanations_share_observations_without_assuming_testimony_true() {
    let mut story = Story::new().unwrap();
    let before = story.observe(Person::Orin, &[], 8).unwrap();
    assert_eq!(
        before.inquiry.report.conclusion,
        HypothesisConclusion::Possible
    );
    assert!(before.inquiry.report.coverage.exhausted);
    assert!(before.inquiry.report.premises.is_empty());
    let supporting = before.inquiry.report.witness.as_ref().unwrap();
    let opposing = before.inquiry.report.counterexample.as_ref().unwrap();
    assert_eq!(supporting.values.len(), 3);
    assert_eq!(
        supporting.values.keys().collect::<Vec<_>>(),
        opposing.values.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        supporting.values["tide_plate.culprit"],
        Person::Mira.encode()
    );
    assert_eq!(opposing.values["tide_plate.culprit"], Person::Orin.encode());
    // These are two explanations of exactly the same observed records, not two observation sets.
    let explanations = [
        (supporting, before.observations.clone()),
        (opposing, before.observations.clone()),
    ];
    assert_eq!(explanations[0].1, explanations[1].1);
    let accusation = before
        .observations
        .iter()
        .find(|observation| {
            observation.exposure.evidence == EvidenceId::Testimony(ClaimId::MiraAccusesOrin)
        })
        .unwrap();
    let Evidence::Testimony(record) = &accusation.evidence else {
        panic!("expected testimony");
    };
    assert_eq!(
        record.statement,
        Statement::TookPlate(Reference::Person(Person::Orin))
    );
    assert_ne!(
        supporting.values["tide_plate.culprit"],
        Person::Orin.encode()
    );
    story.beliefs = [Belief {
        holder: Person::Mira,
        claim: ClaimId::MiraAccusesOrin,
        attitude: Attitude::Accepts,
    }; 2];
    let changed_belief = story.observe(Person::Orin, &[], 8).unwrap();
    assert_eq!(before.inquiry.report, changed_belief.inquiry.report);
    assert_eq!(before.observations, changed_belief.observations);
}

#[test]
fn exposed_reveal_needs_explicit_authority_and_retains_changed_evidence() {
    let mut story = Story::new().unwrap();
    let before = story.observe(Person::Orin, &[], 8).unwrap();
    reveal(&mut story, Person::Orin);
    let untrusted = story.observe(Person::Orin, &[], 8).unwrap();
    assert_eq!(untrusted.inquiry.report, before.inquiry.report);
    let request_line = line!() + 1;
    let request = Request::new(
        "portrait_and_testimony_are_reliable",
        Basis::CoatFromPortrait,
    );
    let after = story.observe(Person::Orin, &[request], 8).unwrap();
    assert_eq!(
        after.inquiry.report.conclusion,
        HypothesisConclusion::Required
    );
    assert!(after.inquiry.report.coverage.exhausted);
    assert!(after.inquiry.report.counterexample.is_none());
    assert_eq!(&after.observations[..2], before.observations.as_slice());
    assert_eq!(after.observations.len(), 3);
    let reveal = after.observations.last().unwrap();
    assert_eq!(reveal.exposure.evidence, EvidenceId::CoatPortrait);
    let Evidence::CoatPortrait(portrait) = &reveal.evidence else {
        panic!("expected portrait");
    };
    assert_eq!(portrait.person, Person::Mira);
    assert!(portrait.source.file().ends_with("mystery/claims.rs"));
    assert!(
        reveal
            .exposure
            .source
            .file()
            .ends_with("tests/mystery/observer.rs")
    );
    let admitted = &after.inquiry.admitted[0];
    assert_eq!(admitted.claim.id, ClaimId::OrinSawCoat);
    assert_eq!(admitted.resolved_subject, Some(Person::Mira));
    assert_eq!(admitted.assumption, "portrait_and_testimony_are_reliable");
    assert_eq!(admitted.assumption_source.line(), request_line);
    assert!(
        admitted
            .assumption_source
            .file()
            .ends_with("tests/mystery/observer.rs")
    );
}

#[test]
fn inaccessible_inconsistent_and_incomplete_have_different_outcomes() {
    let mut story = Story::new().unwrap();
    let hidden = Request::new("accept_hidden_dusk", Basis::Claim(ClaimId::DuskLocked));
    assert!(matches!(
        story.observe(Person::Orin, &[hidden], 0),
        Err(ObserverError::Inaccessible {
            evidence: EvidenceId::Testimony(ClaimId::DuskLocked),
            ..
        })
    ));
    let incomplete = story.observe(Person::Orin, &[], 0).unwrap();
    assert_eq!(
        incomplete.inquiry.report.conclusion,
        HypothesisConclusion::Inconclusive
    );
    assert!(!incomplete.inquiry.report.coverage.exhausted);
    reveal(&mut story, Person::Orin);
    let accusation = Request::new(
        "trust_false_accusation",
        Basis::Claim(ClaimId::MiraAccusesOrin),
    );
    let contradictory = story
        .observe(Person::Orin, &[resolve(), accusation], 8)
        .unwrap();
    assert_eq!(
        contradictory.inquiry.report.conclusion,
        HypothesisConclusion::Inconsistent
    );
    assert!(contradictory.inquiry.report.coverage.exhausted);
    assert_eq!(contradictory.inquiry.admitted.len(), 2);
    assert!(contradictory.inquiry.report.witness.is_none());
    let invalid = Request::new("", Basis::Claim(ClaimId::MiraAccusesOrin));
    assert!(matches!(
        story.observe(Person::Orin, &[invalid], 0),
        Err(ObserverError::Inquiry(_))
    ));
    let unresolved = Request::new(
        "accept_unidentified_witness",
        Basis::Claim(ClaimId::OrinSawCoat),
    );
    assert!(matches!(
        story.observe(Person::Orin, &[unresolved], 8),
        Err(ObserverError::Inquiry(
            inquiry::InquiryError::Unresolved { .. }
        ))
    ));
}

#[test]
fn fixed_observer_selection_cost_includes_access_validation_and_classification() {
    let mut story = Story::new().unwrap();
    reveal(&mut story, Person::Orin);
    story.exposures.push(Exposure::new(
        Person::Mira,
        EvidenceId::Testimony(ClaimId::DuskOpen),
    ));
    let report = story.observe(Person::Orin, &[resolve()], 8).unwrap();
    // The initial filter visits all four records, including another observer's private exposure.
    assert_eq!(report.exposure_records_examined, 4);
    // Measured: two comparisons find testimony; three find portrait. No hidden model enumeration.
    assert_eq!(report.access_comparisons, 5);
    assert_eq!(report.observations.len(), 3);
    assert_eq!(report.inquiry.report.coverage.evaluated_candidates, 8);
    assert_eq!(report.inquiry.report.coverage.satisfying_candidates, 4);
    assert_eq!(
        report.inquiry.report.conclusion,
        HypothesisConclusion::Required
    );
}
