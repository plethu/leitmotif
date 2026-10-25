use super::facts::Access;
use super::{Initial, ModelVariant, Scenario};
use leitmotif::{InvariantConclusion, SearchLimits};

#[test]
fn host_fingerprint_matches_report_identity_and_includes_model_prerequisites() {
    let faulty = Scenario::new(ModelVariant::Faulty).unwrap();
    let corrected = Scenario::new(ModelVariant::Corrected).unwrap();
    let initial = Initial {
        has_key: true,
        truth: false,
        belief: true,
    };
    let state = faulty.initial(initial).unwrap();
    let report = faulty
        .model
        .check_invariants(
            &state,
            SearchLimits {
                max_states: 0,
                max_depth: 0,
            },
        )
        .unwrap();
    assert_eq!(faulty.model.fingerprint(), report.model_id);
    assert_eq!(report.conclusion, InvariantConclusion::Inconclusive);
    assert_ne!(faulty.model.fingerprint(), corrected.model.fingerprint());
    assert_eq!(faulty.access(&state).unwrap(), Access::Closed);
}

#[test]
fn required_witness_keeps_its_initial_assignment_when_analysis_order_changes() {
    let faulty = Scenario::new(ModelVariant::Faulty).unwrap();
    let limits = super::protocol::Limits {
        max_nodes: 1000,
        max_depth: 32,
    };
    let mut reports = super::analyse_all(&faulty, limits).unwrap();
    let (original_initial, _) = super::required_violation(&reports).unwrap();
    reports.reverse();
    let (initial, witness) = super::required_violation(&reports).unwrap();
    assert_ne!(initial, original_initial);
    super::exploration::replay(&faulty, initial, witness).unwrap();
    let continued =
        super::exploration::show_opened_continuation(&faulty, initial, witness).unwrap();
    super::exploration::replay(&faulty, initial, &continued).unwrap();
    let corrected = Scenario::new(ModelVariant::Corrected).unwrap();
    let safe = super::analyse_all(&corrected, limits).unwrap();
    assert!(super::required_violation(&safe).is_err());
}
