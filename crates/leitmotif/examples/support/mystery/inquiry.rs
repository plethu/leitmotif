use super::{
    claims::{Claim, ClaimId, Person, Reference, Statement, claim},
    story::Story,
};
use leitmotif::{ClassificationLimits, HypothesisReport, Premise};
use std::panic::Location;

#[derive(Clone, Copy, Debug)]
pub(super) struct Authority {
    name: &'static str,
    endorsement: Endorsement,
    source: &'static Location<'static>,
}
impl Authority {
    #[track_caller]
    pub(super) fn new(name: &'static str, endorsement: Endorsement) -> Result<Self, InquiryError> {
        let source = Location::caller();
        if name.trim().is_empty() {
            return Err(InquiryError::InvalidAuthority {
                source_location: source,
            });
        }
        Ok(Self {
            name,
            endorsement,
            source,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Endorsement {
    Claim(ClaimId),
    IdentifiedCoat(Person),
}

#[derive(Debug, thiserror::Error)]
pub(super) enum InquiryError {
    #[error("empty authority assumption at {source_location}")]
    InvalidAuthority {
        source_location: &'static Location<'static>,
    },
    #[error(
        "unresolved salt-coat reference in {claim:?}\n  assumption: {assumption}\n  {source_location}"
    )]
    Unresolved {
        claim: ClaimId,
        assumption: &'static str,
        source_location: &'static Location<'static>,
    },
    #[error(transparent)]
    Model(#[from] leitmotif::Error),
}
pub(super) struct Admitted {
    pub(super) assumption: &'static str,
    pub(super) assumption_source: &'static Location<'static>,
    pub(super) claim: Claim,
    pub(super) resolved_subject: Option<Person>,
}
pub(super) struct Inquiry {
    pub(super) report: HypothesisReport,
    pub(super) admitted: Vec<Admitted>,
}
impl Story {
    pub(super) fn inquire(
        &self,
        authorities: &[Authority],
        max_candidates: usize,
    ) -> Result<Inquiry, InquiryError> {
        let mut premises = Vec::new();
        let mut admitted = Vec::new();
        for authority in authorities {
            let (id, coat_identity) = match authority.endorsement {
                Endorsement::Claim(id) => (id, None),
                Endorsement::IdentifiedCoat(person) => (ClaimId::OrinSawCoat, Some(person)),
            };
            let claim = claim(id);
            let (predicate, resolved_subject) = match claim.statement {
                Statement::CabinetLocked(value) => (self.locked(claim.context, value), None),
                Statement::TookPlate(reference) => {
                    let person = match reference {
                        Reference::Person(person) => person,
                        Reference::SaltCoat => coat_identity.ok_or(InquiryError::Unresolved {
                            claim: claim.id,
                            assumption: authority.name,
                            source_location: claim.source,
                        })?,
                    };
                    (self.culprit(person), Some(person))
                }
            };
            premises.push(Premise::new(authority.name, predicate));
            admitted.push(Admitted {
                assumption: authority.name,
                assumption_source: authority.source,
                claim,
                resolved_subject,
            });
        }
        let report = self.model.classify(
            &self.culprit(Person::Mira),
            &premises,
            ClassificationLimits { max_candidates },
        )?;
        Ok(Inquiry { report, admitted })
    }
}
