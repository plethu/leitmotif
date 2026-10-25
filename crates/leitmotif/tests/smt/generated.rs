use super::{Facts, options, verdict};
use leitmotif::{ClassificationLimits, ModelBuilder, Predicate, Premise, smt::Conclusion};

#[test]
fn generated_finite_queries_match_complete_enumeration_and_validate_every_witness() {
    for configuration in 0_u8..32 {
        let mut builder = ModelBuilder::new("generated_smt").unwrap();
        let facts = Facts::declare(&mut builder).unwrap();
        let model = builder.build().unwrap();
        let mut premises = vec![Premise::new(
            "choice",
            match configuration & 3 {
                0 => Predicate::constant(true),
                1 => facts.a.is(true).or(facts.b.is(false)),
                2 => facts.b.is(true).and(facts.c.is(false)).negate(),
                _ => facts.a.is(true).implies(facts.c.is(true)),
            },
        )];
        if configuration & 4 != 0 {
            premises.push(Premise::new("a", facts.a.is(true)));
        }
        if configuration & 8 != 0 {
            premises.push(Premise::new("not_a", facts.a.is(false)));
        }
        if configuration & 16 != 0 {
            premises.push(Premise::new("b", facts.b.is(true)));
        }
        for hypothesis in [
            facts.a.is(true),
            facts.b.is(false).or(facts.c.is(true)),
            facts.a.is(true).and(facts.c.is(true)).negate(),
        ] {
            let reference = model
                .classify(
                    &hypothesis,
                    &premises,
                    ClassificationLimits { max_candidates: 8 },
                )
                .unwrap();
            let report = model
                .classify_smt(&hypothesis, &premises, &options().unwrap())
                .unwrap();
            assert_eq!(verdict(&report.conclusion), Some(reference.conclusion));
            assert_eq!(report.model_id, reference.model_id);
            assert_eq!(report.hypothesis, reference.hypothesis);
            assert_eq!(report.premises, reference.premises);
            assert!((1..=2).contains(&report.checks.len()));
            match report.conclusion {
                Conclusion::Required { witness, .. } => facts
                    .validate(&model, &witness, &hypothesis, &premises, true)
                    .unwrap(),
                Conclusion::RuledOut { counterexample, .. } => facts
                    .validate(&model, &counterexample, &hypothesis, &premises, false)
                    .unwrap(),
                Conclusion::Possible {
                    witness,
                    counterexample,
                } => {
                    facts
                        .validate(&model, &witness, &hypothesis, &premises, true)
                        .unwrap();
                    facts
                        .validate(&model, &counterexample, &hypothesis, &premises, false)
                        .unwrap();
                }
                Conclusion::Inconsistent { .. } => assert_eq!(report.checks.len(), 1),
                Conclusion::Incomplete { reason, .. } => {
                    panic!("small finite query should complete: {reason:?}")
                }
            }
        }
    }
}
