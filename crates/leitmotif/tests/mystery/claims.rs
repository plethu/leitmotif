use super::*;
use claims::{Attitude, Belief, Exposure, Reference, Statement};
use inquiry::InquiryError;
use leitmotif::HypothesisConclusion;

fn authority(name: &'static str, id: ClaimId) -> Authority {
    Authority::new(name, Endorsement::Claim(id)).unwrap()
}

#[test]
fn testimony_belief_exposure_and_author_truth_are_not_premises() {
    let mut story = Story::new().unwrap();
    assert!(
        story
            .model
            .evaluate(&story.truth, &story.culprit(Person::Mira))
            .unwrap()
    );
    assert!(
        !story
            .model
            .evaluate(&story.truth, &story.culprit(Person::Orin))
            .unwrap()
    );
    let before = story.inquire(&[], 8).unwrap();
    assert_eq!(before.report.conclusion, HypothesisConclusion::Possible);
    assert!(before.report.witness.is_some());
    assert!(before.report.counterexample.is_some());
    assert!(before.report.premises.is_empty());
    assert!(before.admitted.is_empty());
    story.beliefs = [Belief {
        holder: Person::Mira,
        claim: ClaimId::MiraAccusesOrin,
        attitude: Attitude::Accepts,
    }; 2];
    story.exposures = [Exposure {
        observer: Person::Mira,
        claim: ClaimId::DuskOpen,
    }; 2];
    assert_eq!(before.report, story.inquire(&[], 8).unwrap().report);
    let accepted = story
        .inquire(
            &[authority("trust_accusation", ClaimId::MiraAccusesOrin)],
            8,
        )
        .unwrap();
    assert_eq!(accepted.report.conclusion, HypothesisConclusion::RuledOut);
    assert_eq!(accepted.admitted[0].resolved_subject, Some(Person::Orin));
}

#[test]
fn context_order_does_not_imply_truth_inheritance() {
    assert!(Context::Dusk.precedes(Context::Dawn));
    assert!(!Context::Dawn.precedes(Context::Dusk));
    assert!(!Context::Dusk.precedes(Context::Dusk));
    let story = Story::new().unwrap();
    let admitted = story
        .inquire(
            &[
                authority("dusk_lock", ClaimId::DuskLocked),
                authority("dawn_open", ClaimId::DawnOpen),
            ],
            8,
        )
        .unwrap();
    assert_eq!(admitted.report.conclusion, HypothesisConclusion::Possible);
    assert_eq!(admitted.report.coverage.satisfying_candidates, 2);
    assert_eq!(admitted.admitted[0].claim.context, Context::Dusk);
    assert_eq!(admitted.admitted[1].claim.context, Context::Dawn);
}

#[test]
fn incompatible_authorities_retain_named_assumptions_and_claim_sources() {
    let story = Story::new().unwrap();
    let left = authority("mira_is_correct_at_dusk", ClaimId::DuskLocked);
    let right = authority("orin_is_correct_at_dusk", ClaimId::DuskOpen);
    for authority in [left, right] {
        assert_eq!(
            story.inquire(&[authority], 8).unwrap().report.conclusion,
            HypothesisConclusion::Possible
        );
    }
    let inquiry = story.inquire(&[left, right], 8).unwrap();
    assert_eq!(
        inquiry.report.conclusion,
        HypothesisConclusion::Inconsistent
    );
    assert!(inquiry.report.witness.is_none());
    assert!(inquiry.report.counterexample.is_none());
    for (admitted, premise) in inquiry.admitted.iter().zip(&inquiry.report.premises) {
        assert_eq!(admitted.assumption, premise.id);
        assert!(
            admitted
                .assumption_source
                .file()
                .ends_with("tests/mystery/claims.rs")
        );
        assert_ne!(
            admitted.assumption_source.file(),
            admitted.claim.source.file()
        );
        assert!(admitted.claim.source.file().ends_with("mystery/claims.rs"));
        assert!(admitted.claim.source.line() > 0);
    }
    assert_eq!(inquiry.admitted[0].claim.speaker, Person::Mira);
    assert_eq!(inquiry.admitted[1].claim.speaker, Person::Orin);
}

#[test]
fn unresolved_reference_is_preserved_and_needs_explicit_identity() {
    let record = claim(ClaimId::OrinSawCoat);
    assert_eq!(record.statement, Statement::TookPlate(Reference::SaltCoat));
    assert_eq!(record.id, claim(record.id).id);
    let story = Story::new().unwrap();
    let unresolved = authority("trust_coat_witness", record.id);
    let error = story.inquire(&[unresolved], 0).err().unwrap();
    match error {
        InquiryError::Unresolved {
            claim,
            assumption,
            source_location,
        } => {
            assert_eq!(claim, record.id);
            assert_eq!(assumption, "trust_coat_witness");
            assert_eq!(source_location, record.source);
        }
        other => panic!("unexpected error: {other}"),
    }
    for (person, expected) in [
        (Person::Mira, HypothesisConclusion::Required),
        (Person::Orin, HypothesisConclusion::RuledOut),
    ] {
        let resolved =
            Authority::new("identified_salt_coat", Endorsement::IdentifiedCoat(person)).unwrap();
        let inquiry = story.inquire(&[resolved], 8).unwrap();
        assert_eq!(inquiry.report.conclusion, expected);
        assert_eq!(inquiry.admitted[0].claim.id, record.id);
        assert_eq!(inquiry.admitted[0].resolved_subject, Some(person));
    }
}

#[test]
fn invalid_assumptions_and_incomplete_enumeration_remain_distinct() {
    let story = Story::new().unwrap();
    let a = authority("duplicate", ClaimId::DuskLocked);
    let error = story.inquire(&[a, a], 0).err().unwrap();
    assert!(matches!(error, InquiryError::Model(_)));
    let incomplete = story.inquire(&[], 0).unwrap();
    assert_eq!(
        incomplete.report.conclusion,
        HypothesisConclusion::Inconclusive
    );
    assert!(!incomplete.report.coverage.exhausted);
    assert_eq!(incomplete.report.coverage.evaluated_candidates, 0);
}

#[test]
fn fixed_mystery_classification_cost_and_outputs() {
    // Three binary facts measure eight evaluated assignments, even when no premises admit them.
    // This bounds finite enumeration work for this fixture, not elapsed time or arbitrary stories.
    let story = Story::new().unwrap();
    for (authorities, expected, admitted) in [
        (vec![], HypothesisConclusion::Possible, 8),
        (
            vec![
                authority("dusk", ClaimId::DuskLocked),
                authority("dawn", ClaimId::DawnOpen),
            ],
            HypothesisConclusion::Possible,
            2,
        ),
        (
            vec![
                authority("locked", ClaimId::DuskLocked),
                authority("open", ClaimId::DuskOpen),
            ],
            HypothesisConclusion::Inconsistent,
            0,
        ),
    ] {
        let report = story.inquire(&authorities, 8).unwrap().report;
        assert_eq!(report.conclusion, expected);
        assert_eq!(report.coverage.evaluated_candidates, 8);
        assert_eq!(report.coverage.satisfying_candidates, admitted);
        assert!(report.coverage.exhausted);
    }
}

#[test]
fn executable_demonstrates_the_semantics() {
    run().unwrap();
}

#[test]
fn assumption_ingress_and_expected_failure_presentation_preserve_errors() {
    let error = Authority::new("  ", Endorsement::Claim(ClaimId::DuskLocked))
        .err()
        .unwrap();
    assert!(matches!(error, InquiryError::InvalidAuthority { .. }));
    assert!(print_unresolved(Err(error)).is_err());
    let story = Story::new().unwrap();
    let a = authority("duplicate", ClaimId::DuskLocked);
    let result = print_unresolved(story.inquire(&[a, a], 8));
    assert!(result.unwrap_err().downcast_ref::<InquiryError>().is_some());
    assert!(print_unresolved(story.inquire(&[], 8)).is_err());
}
