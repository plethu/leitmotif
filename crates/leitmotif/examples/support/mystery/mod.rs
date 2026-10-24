mod claims;
mod inquiry;
mod story;
use claims::{ClaimId, Context, Person, claim};
use inquiry::{Authority, Endorsement, Inquiry, InquiryError};
use std::error::Error;
use story::Story;

fn print_inquiry(label: &str, inquiry: &Inquiry) {
    println!("\n{label}: {:?}", inquiry.report.conclusion);
    println!(
        "  candidates: {}; admitted: {}; exhausted: {}",
        inquiry.report.coverage.evaluated_candidates,
        inquiry.report.coverage.satisfying_candidates,
        inquiry.report.coverage.exhausted
    );
    for admitted in &inquiry.admitted {
        println!(
            "  assumption {} <- {:?}",
            admitted.assumption, admitted.claim.id
        );
        println!("    authority: {}", admitted.assumption_source);
        println!("    claim: {}", admitted.claim.source);
        if let Some(person) = admitted.resolved_subject {
            println!("    resolved subject: {person:?}");
        }
    }
}

fn print_unresolved(result: Result<Inquiry, InquiryError>) -> Result<(), Box<dyn Error>> {
    match result {
        Err(error @ InquiryError::Unresolved { .. }) => {
            println!("\nUnresolved testimony:\n  {error}");
            Ok(())
        }
        Err(error) => Err(error.into()),
        Ok(_) => Err("unresolved reference was silently admitted".into()),
    }
}

/// Demonstrates explicit authority without equating testimony or belief with truth.
///
/// # Errors
/// Returns invalid model, premise or unresolved-reference diagnostics.
pub fn run() -> Result<(), Box<dyn Error>> {
    let story = Story::new()?;
    println!("The missing tide plate");
    println!("Question: did Mira take the plate?");
    println!("Finite domain: exactly one culprit, Mira or Orin; one cabinet.");
    println!("Author truth: Mira; cabinet locked at dusk, open at dawn.");
    println!("Author truth, beliefs and exposure are not implicit premises.");
    println!("Model: {}", story.model.fingerprint());
    println!(
        "Dusk precedes dawn: {} (no truth inheritance)",
        Context::Dusk.precedes(Context::Dawn)
    );
    for belief in &story.beliefs {
        println!(
            "Belief: {:?} {:?} {:?}",
            belief.holder, belief.attitude, belief.claim
        );
    }
    for exposure in &story.exposures {
        println!(
            "Exposure: {:?} encountered {:?}",
            exposure.observer, exposure.evidence
        );
    }
    for id in [
        ClaimId::MiraAccusesOrin,
        ClaimId::OrinSawCoat,
        ClaimId::DuskLocked,
        ClaimId::DuskOpen,
        ClaimId::DawnOpen,
    ] {
        let record = claim(id);
        println!(
            "Claim {:?}: {:?} at {:?}",
            record.id, record.speaker, record.context
        );
        println!("  {:?}", record.statement);
        println!("  {}", record.source);
    }
    let dusk = Authority::new("accept_mira_dusk", Endorsement::Claim(ClaimId::DuskLocked))?;
    let dawn = Authority::new("accept_orin_dawn", Endorsement::Claim(ClaimId::DawnOpen))?;
    let conflict = Authority::new("accept_orin_dusk", Endorsement::Claim(ClaimId::DuskOpen))?;
    for (label, authorities) in [
        ("Testimony only", vec![]),
        ("Different times", vec![dusk, dawn]),
        ("Same-time authorities", vec![dusk, conflict]),
    ] {
        print_inquiry(label, &story.inquire(&authorities, 8)?);
    }
    let coat = Authority::new(
        "accept_coat_testimony",
        Endorsement::Claim(ClaimId::OrinSawCoat),
    )?;
    print_unresolved(story.inquire(&[coat], 8))?;
    let resolved = Authority::new(
        "identify_and_accept_coat",
        Endorsement::IdentifiedCoat(Person::Mira),
    )?;
    print_inquiry("Explicit coat identity", &story.inquire(&[resolved], 8)?);
    // The assignment is retained independently for checking testimony against the authored world.
    println!(
        "Mira's accusation matches author truth: {}",
        story
            .model
            .evaluate(&story.truth, &story.culprit(Person::Orin))?
    );
    observer::demonstrate()?;
    Ok(())
}
mod observer;
#[cfg(test)]
#[path = "../../../tests/mystery/observer.rs"]
mod observer_tests;
#[cfg(test)]
#[path = "../../../tests/mystery/claims.rs"]
mod tests;
