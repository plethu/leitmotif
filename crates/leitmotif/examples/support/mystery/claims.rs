use std::panic::Location;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Person {
    Mira,
    Orin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    Dusk,
    Dawn,
}
impl Context {
    pub(super) const fn precedes(self, other: Self) -> bool {
        matches!((self, other), (Self::Dusk, Self::Dawn))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reference {
    Person(Person),
    SaltCoat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Statement {
    TookPlate(Reference),
    CabinetLocked(bool),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClaimId {
    MiraAccusesOrin,
    OrinSawCoat,
    DuskLocked,
    DuskOpen,
    DawnOpen,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Claim {
    pub(super) id: ClaimId,
    pub(super) speaker: Person,
    pub(super) context: Context,
    pub(super) statement: Statement,
    pub(super) source: &'static Location<'static>,
}
impl Claim {
    #[track_caller]
    const fn new(id: ClaimId, speaker: Person, context: Context, statement: Statement) -> Self {
        Self {
            id,
            speaker,
            context,
            statement,
            source: Location::caller(),
        }
    }
}
pub(super) const fn claim(id: ClaimId) -> Claim {
    match id {
        ClaimId::MiraAccusesOrin => Claim::new(
            id,
            Person::Mira,
            Context::Dusk,
            Statement::TookPlate(Reference::Person(Person::Orin)),
        ),
        ClaimId::OrinSawCoat => Claim::new(
            id,
            Person::Orin,
            Context::Dusk,
            Statement::TookPlate(Reference::SaltCoat),
        ),
        ClaimId::DuskLocked => Claim::new(
            id,
            Person::Mira,
            Context::Dusk,
            Statement::CabinetLocked(true),
        ),
        ClaimId::DuskOpen => Claim::new(
            id,
            Person::Orin,
            Context::Dusk,
            Statement::CabinetLocked(false),
        ),
        ClaimId::DawnOpen => Claim::new(
            id,
            Person::Orin,
            Context::Dawn,
            Statement::CabinetLocked(false),
        ),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attitude {
    Accepts,
    Rejects,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Belief {
    pub(super) holder: Person,
    pub(super) claim: ClaimId,
    pub(super) attitude: Attitude,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EvidenceId {
    Testimony(ClaimId),
    CoatPortrait,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Exposure {
    pub(super) observer: Person,
    pub(super) evidence: EvidenceId,
    pub(super) source: &'static Location<'static>,
}
impl Exposure {
    #[track_caller]
    pub(super) const fn new(observer: Person, evidence: EvidenceId) -> Self {
        Self {
            observer,
            evidence,
            source: Location::caller(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Evidence {
    Testimony(Claim),
    CoatPortrait(Portrait),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Portrait {
    pub(super) person: Person,
    pub(super) source: &'static Location<'static>,
}
pub(super) const fn portrait() -> Portrait {
    // A labelled harbour portrait identifies the salt-coat figure as Mira.
    Portrait {
        person: Person::Mira,
        source: Location::caller(),
    }
}
pub(super) const fn evidence(id: EvidenceId) -> Evidence {
    match id {
        EvidenceId::Testimony(id) => Evidence::Testimony(claim(id)),
        EvidenceId::CoatPortrait => Evidence::CoatPortrait(portrait()),
    }
}
