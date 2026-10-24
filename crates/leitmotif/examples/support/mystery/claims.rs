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
#[derive(Clone, Debug)]
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
pub(super) struct Exposure {
    pub(super) observer: Person,
    pub(super) claim: ClaimId,
}
