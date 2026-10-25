//! An observer can request authority only over evidence they have encountered.
use super::{
    claims::{ClaimId, Evidence, EvidenceId, Exposure, Person, evidence, portrait},
    inquiry::{Authority, Endorsement, Inquiry, InquiryError},
    story::Story,
};
use std::{error::Error, panic::Location};

#[derive(Clone, Copy, Debug)]
pub(super) enum Basis {
    Claim(ClaimId),
    CoatFromPortrait,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Request {
    name: &'static str,
    basis: Basis,
    source: &'static Location<'static>,
}
impl Request {
    #[track_caller]
    pub(super) const fn new(name: &'static str, basis: Basis) -> Self {
        Self {
            name,
            basis,
            source: Location::caller(),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub(super) enum ObserverError {
    #[error("{observer:?} has not encountered {evidence:?}\n  requested at {source_location}")]
    Inaccessible {
        observer: Person,
        evidence: EvidenceId,
        source_location: &'static Location<'static>,
    },
    #[error("observer evidence comparison capacity exceeded")]
    Capacity,
    #[error(transparent)]
    Inquiry(#[from] InquiryError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Observation {
    pub(super) exposure: Exposure,
    pub(super) evidence: Evidence,
}
pub(super) struct ObserverInquiry {
    pub(super) observer: Person,
    pub(super) observations: Vec<Observation>,
    pub(super) inquiry: Inquiry,
    pub(super) exposure_records_examined: usize,
    /// Actual equality comparisons while validating requested evidence access.
    pub(super) access_comparisons: usize,
}

struct Accessible {
    observer: Person,
    observations: Vec<Observation>,
    comparisons: usize,
}
impl Accessible {
    fn require(&mut self, id: EvidenceId, request: &Request) -> Result<(), ObserverError> {
        for observation in &self.observations {
            self.comparisons = self
                .comparisons
                .checked_add(1)
                .ok_or(ObserverError::Capacity)?;
            if observation.exposure.evidence == id {
                return Ok(());
            }
        }
        Err(ObserverError::Inaccessible {
            observer: self.observer,
            evidence: id,
            source_location: request.source,
        })
    }
}
impl Story {
    pub(super) fn observe(
        &self,
        observer: Person,
        requests: &[Request],
        max_candidates: usize,
    ) -> Result<ObserverInquiry, ObserverError> {
        let mut accessible = Accessible {
            observer,
            observations: self
                .exposures
                .iter()
                .filter(|exposure| exposure.observer == observer)
                .map(|exposure| Observation {
                    exposure: *exposure,
                    evidence: evidence(exposure.evidence),
                })
                .collect(),
            comparisons: 0,
        };
        let mut authorities = Vec::new();
        for request in requests {
            let endorsement = match request.basis {
                Basis::Claim(id) => {
                    accessible.require(EvidenceId::Testimony(id), request)?;
                    Endorsement::Claim(id)
                }
                Basis::CoatFromPortrait => {
                    accessible.require(EvidenceId::Testimony(ClaimId::OrinSawCoat), request)?;
                    accessible.require(EvidenceId::CoatPortrait, request)?;
                    Endorsement::IdentifiedCoat(portrait().person)
                }
            };
            authorities.push(Authority::at(request.name, endorsement, request.source)?);
        }
        // No author assignment or character belief is consulted here.
        let inquiry = self.inquire(&authorities, max_candidates)?;
        Ok(ObserverInquiry {
            observer,
            observations: accessible.observations,
            inquiry,
            access_comparisons: accessible.comparisons,
            exposure_records_examined: self.exposures.len(),
        })
    }
}

pub(super) fn demonstrate() -> Result<(), Box<dyn Error>> {
    let mut story = Story::new()?;
    println!("\nObserver: Orin; observed testimony is not assumed true.");
    let before = story.observe(Person::Orin, &[], 8)?;
    super::print_inquiry("Before reveal: two admitted explanations", &before.inquiry);
    println!(
        "  observer: {:?}; observed records: {}",
        before.observer,
        before.observations.len()
    );
    for (label, world) in [
        ("Supporting world", &before.inquiry.report.witness),
        ("Opposing world", &before.inquiry.report.counterexample),
    ] {
        if let Some(world) = world {
            println!("  {label} under the same observed records:");
            for (fact, value) in &world.values {
                println!("    {fact}: {value:?}");
            }
        }
    }
    let request = Request::new("accept_testimony_and_portrait", Basis::CoatFromPortrait);
    match story.observe(Person::Orin, &[request], 0) {
        Err(error @ ObserverError::Inaccessible { .. }) => {
            println!("Hidden identity rejected:\n  {error}");
        }
        Err(error) => return Err(error.into()),
        Ok(_) => return Err("hidden portrait was admitted".into()),
    }
    story
        .exposures
        .push(Exposure::new(Person::Orin, EvidenceId::CoatPortrait));
    let after = story.observe(Person::Orin, &[request], 8)?;
    super::print_inquiry(
        "After exposed portrait and explicit authority",
        &after.inquiry,
    );
    println!(
        "  examined exposures: {}; access comparisons: {}",
        after.exposure_records_examined, after.access_comparisons
    );
    for observation in &after.observations {
        println!("  {:?}", observation.exposure.evidence);
        println!("    exposure: {}", observation.exposure.source);
        if let Evidence::CoatPortrait(portrait) = &observation.evidence {
            println!(
                "    portrait identifies {:?}; source:\n    {}",
                portrait.person, portrait.source
            );
        }
    }
    // Exposing a false accusation still does not promote it without this separate request.
    let accusation = Request::new(
        "accept_false_accusation",
        Basis::Claim(ClaimId::MiraAccusesOrin),
    );
    super::print_inquiry(
        "Both incompatible authorities",
        &story
            .observe(Person::Orin, &[request, accusation], 8)?
            .inquiry,
    );
    Ok(())
}
