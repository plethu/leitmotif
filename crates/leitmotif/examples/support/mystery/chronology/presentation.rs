use super::{Chronology, DisclosureRule, LIMITS, Occurrence, Performance};
use leitmotif::InvariantConclusion;
use std::error::Error;

pub(in super::super) fn demonstrate() -> Result<(), Box<dyn Error>> {
    let chronology = Chronology::story()?;
    println!("\nChronology: two distinct cabinet-lock occurrences, A and B.");
    println!("  Both precede today's conversation; A versus B is unspecified.");
    println!(
        "  Lock A before conversation required: {}",
        chronology.requires_before(Occurrence::LockA, Occurrence::Conversation)
    );
    println!(
        "  Lock A before lock B required: {}",
        chronology.requires_before(Occurrence::LockA, Occurrence::LockB)
    );
    for edge in &chronology.edges {
        println!("  {:?} < {:?}", edge.earlier, edge.later);
        println!("    {}", edge.source);
    }
    println!("  Explicit acquisition licenses Orin's disclosure of that occurrence.");
    println!("  Audience flashbacks do not grant Orin knowledge.");
    let unchecked = Performance::new(DisclosureRule::Unchecked)?;
    let report = unchecked
        .model
        .check_invariants(&unchecked.initial, LIMITS)?;
    if let InvariantConclusion::Violation { witness, invariant } = report.conclusion {
        println!("Early disclosure rejected: {}", invariant.id);
        for (index, step) in witness.steps.iter().enumerate() {
            println!("  execution {index}: {}", step.transition.id);
            println!(
                "    source:\n    {}:{}",
                step.transition.provenance.file, step.transition.provenance.line
            );
        }
    } else {
        return Err("expected early-disclosure witness".into());
    }
    let corrected = Performance::new(DisclosureRule::RequiresAcquisition)?;
    let report = corrected
        .model
        .check_invariants(&corrected.initial, LIMITS)?;
    println!(
        "Guarded disclosures: {:?}; {} states evaluated",
        report.conclusion, report.coverage.evaluated_states
    );
    let mut state = corrected.initial.clone();
    println!("Valid late revelation (execution order):");
    for id in [
        "conversation",
        "acquire_lock_a",
        "disclose_lock_a",
        "flashback_lock_a",
    ] {
        state = corrected
            .model
            .apply(&state, id)?
            .ok_or("disabled late-revelation action")?;
        println!("  {id}");
    }
    println!("  The flashback still depicts historical lock A.");
    Ok(())
}
