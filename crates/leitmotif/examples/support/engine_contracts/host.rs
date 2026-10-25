//! Game-owned state and effects. No model successor is assigned into this host.
use leitmotif::{Snapshot, Value};
use serde::{Deserialize, Serialize};

use super::super::composed_town::{
    Ending as ModelEnding, HarbourProjection, KeeperProjection, Projection,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Ending {
    Open,
    Sheltered,
    Departed,
}
impl Ending {
    const fn symbol(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Sheltered => "sheltered",
            Self::Departed => "departed",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "bool", into = "bool")]
pub(super) enum OilSupply {
    Available,
    Spent,
}
impl OilSupply {
    pub(super) const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}
impl From<bool> for OilSupply {
    fn from(value: bool) -> Self {
        if value { Self::Available } else { Self::Spent }
    }
}
impl From<OilSupply> for bool {
    fn from(value: OilSupply) -> Self {
        value.is_available()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "bool", into = "bool")]
pub(super) enum Beacon {
    Dark,
    Lit,
}
impl Beacon {
    pub(super) const fn is_lit(self) -> bool {
        matches!(self, Self::Lit)
    }
}
impl From<bool> for Beacon {
    fn from(value: bool) -> Self {
        if value { Self::Lit } else { Self::Dark }
    }
}
impl From<Beacon> for bool {
    fn from(value: Beacon) -> Self {
        value.is_lit()
    }
}

/// The entire finite projection, including the immutable storm setting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HostSnapshot {
    #[serde(rename = "harbour.ending")]
    pub(super) ending: Ending,
    #[serde(rename = "harbour.oil_available")]
    pub(super) oil: OilSupply,
    #[serde(rename = "harbour.storm_passed")]
    pub(super) storm: bool,
    #[serde(rename = "keeper.warning_shared")]
    pub(super) warning: bool,
    #[serde(rename = "keeper.beacon_lit")]
    pub(super) beacon: Beacon,
    #[serde(rename = "ferryman.ready")]
    pub(super) ferry: bool,
}
impl HostSnapshot {
    pub(super) const fn initial() -> Self {
        Self {
            ending: Ending::Open,
            oil: OilSupply::Available,
            storm: false,
            warning: false,
            beacon: Beacon::Dark,
            ferry: false,
        }
    }
    pub(super) const fn projection(&self) -> Projection {
        Projection {
            harbour: HarbourProjection {
                ending: match self.ending {
                    Ending::Open => ModelEnding::Open,
                    Ending::Sheltered => ModelEnding::Sheltered,
                    Ending::Departed => ModelEnding::Departed,
                },
                oil: self.oil.is_available(),
                storm: self.storm,
            },
            keeper: KeeperProjection {
                warning: self.warning,
                beacon: self.beacon.is_lit(),
            },
            ferry: self.ferry,
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            values: [
                (
                    "harbour.ending".into(),
                    Value::Symbol(self.ending.symbol().into()),
                ),
                (
                    "harbour.oil_available".into(),
                    Value::Bool(self.oil.is_available()),
                ),
                ("harbour.storm_passed".into(), Value::Bool(self.storm)),
                ("keeper.warning_shared".into(), Value::Bool(self.warning)),
                (
                    "keeper.beacon_lit".into(),
                    Value::Bool(self.beacon.is_lit()),
                ),
                ("ferryman.ready".into(), Value::Bool(self.ferry)),
            ]
            .into(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    ShareWarning,
    LightBeacon,
    FuelFerry,
    Shelter,
    Depart,
}
impl Action {
    pub(super) const ALL: [Self; 5] = [
        Self::ShareWarning,
        Self::LightBeacon,
        Self::FuelFerry,
        Self::Shelter,
        Self::Depart,
    ];
    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::ShareWarning => "keeper.share_warning",
            Self::LightBeacon => "keeper.light_beacon",
            Self::FuelFerry => "ferryman.fuel_ferry",
            Self::Shelter => "harbour.shelter",
            Self::Depart => "harbour.depart",
        }
    }
    pub(super) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}
#[derive(Clone, Copy, Debug)]
enum Lighting {
    SpendOil,
    PreserveOil,
}
#[derive(Clone, Debug)]
pub(super) struct Host {
    state: HostSnapshot,
    lighting: Lighting,
}
impl Host {
    pub(super) const fn initial() -> Self {
        Self::new(HostSnapshot::initial())
    }
    pub(super) const fn new(state: HostSnapshot) -> Self {
        Self {
            state,
            lighting: Lighting::SpendOil,
        }
    }
    pub(super) const fn with_lighting_fault(state: HostSnapshot) -> Self {
        Self {
            state,
            lighting: Lighting::PreserveOil,
        }
    }
    pub(super) const fn snapshot(&self) -> &HostSnapshot {
        &self.state
    }
    pub(super) fn apply(&mut self, action: Action) -> bool {
        if self.state.ending != Ending::Open {
            return false;
        }
        match action {
            Action::ShareWarning if !self.state.warning => self.state.warning = true,
            Action::LightBeacon if self.state.warning && self.state.oil.is_available() => {
                self.state.beacon = Beacon::Lit;
                if matches!(self.lighting, Lighting::SpendOil) {
                    self.state.oil = OilSupply::Spent;
                }
            }
            Action::FuelFerry if self.state.warning && self.state.oil.is_available() => {
                self.state.ferry = true;
                self.state.oil = OilSupply::Spent;
            }
            Action::Shelter if self.state.beacon.is_lit() => self.state.ending = Ending::Sheltered,
            Action::Depart if self.state.ferry => self.state.ending = Ending::Departed,
            _ => return false,
        }
        true
    }
}
