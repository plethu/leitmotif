//! The game owns entities, commands, scheduling and all effects.
use bevy_app::{App, Update};
use bevy_ecs::prelude::{Component, Entity, ResMut, Resource, Single};
use leitmotif::{Snapshot, Value};

use super::{
    Error,
    composed_town::{Ending as ModelEnding, HarbourProjection, KeeperProjection, Projection},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    Open,
    Sheltered,
    Departed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Oil {
    Available,
    Spent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beacon {
    Dark,
    Lit,
}
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(super) struct Harbour {
    ending: Ending,
    oil: Oil,
    storm: bool,
    warning: bool,
    beacon: Beacon,
    ferry: bool,
}
impl Harbour {
    pub(super) const fn initial() -> Self {
        Self {
            ending: Ending::Open,
            oil: Oil::Available,
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
                oil: matches!(self.oil, Oil::Available),
                storm: self.storm,
            },
            keeper: KeeperProjection {
                warning: self.warning,
                beacon: matches!(self.beacon, Beacon::Lit),
            },
            ferry: self.ferry,
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        let ending = match self.ending {
            Ending::Open => "open",
            Ending::Sheltered => "sheltered",
            Ending::Departed => "departed",
        };
        Snapshot {
            values: [
                ("harbour.ending".into(), Value::Symbol(ending.into())),
                (
                    "harbour.oil_available".into(),
                    Value::Bool(matches!(self.oil, Oil::Available)),
                ),
                ("harbour.storm_passed".into(), Value::Bool(self.storm)),
                ("keeper.warning_shared".into(), Value::Bool(self.warning)),
                (
                    "keeper.beacon_lit".into(),
                    Value::Bool(matches!(self.beacon, Beacon::Lit)),
                ),
                ("ferryman.ready".into(), Value::Bool(self.ferry)),
            ]
            .into(),
        }
    }
    fn apply(&mut self, action: Action, lighting: Lighting) -> bool {
        if self.ending != Ending::Open {
            return false;
        }
        match action {
            Action::ShareWarning if !self.warning => self.warning = true,
            Action::LightBeacon if self.warning && matches!(self.oil, Oil::Available) => {
                self.beacon = Beacon::Lit;
                if lighting == Lighting::SpendOil {
                    self.oil = Oil::Spent;
                }
            }
            Action::FuelFerry if self.warning && matches!(self.oil, Oil::Available) => {
                self.ferry = true;
                self.oil = Oil::Spent;
            }
            Action::Shelter if matches!(self.beacon, Beacon::Lit) => {
                self.ending = Ending::Sheltered;
            }
            Action::Depart if self.ferry => self.ending = Ending::Departed,
            _ => return false,
        }
        true
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub(super) fn parse(id: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|action| action.id() == id)
            .ok_or_else(|| Error::Unsupported(id.into()))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lighting {
    SpendOil,
    PreserveOil,
}
#[derive(Resource)]
struct Intent {
    pending: Option<Action>,
    accepted: Option<bool>,
    lighting: Lighting,
}
fn execute(mut harbour: Single<&mut Harbour>, mut intent: ResMut<Intent>) {
    if let Some(action) = intent.pending.take() {
        intent.accepted = Some(harbour.apply(action, intent.lighting));
    }
}
pub(super) struct Host {
    app: App,
    harbour: Entity,
}
impl Host {
    pub(super) fn new(initial: Harbour, lighting: Lighting) -> Self {
        let mut app = App::new();
        let harbour = app.world_mut().spawn(initial).id();
        app.insert_resource(Intent {
            pending: None,
            accepted: None,
            lighting,
        });
        app.add_systems(Update, execute);
        Self { app, harbour }
    }
    pub(super) fn state(&self) -> Result<&Harbour, Error> {
        self.app
            .world()
            .get::<Harbour>(self.harbour)
            .ok_or(Error::MissingHarbour)
    }
    pub(super) fn apply(&mut self, action: Action) -> Result<bool, Error> {
        self.state()?;
        let mut intent = self.app.world_mut().resource_mut::<Intent>();
        intent.pending = Some(action);
        intent.accepted = None;
        self.app.update();
        self.app
            .world_mut()
            .resource_mut::<Intent>()
            .accepted
            .take()
            .ok_or(Error::MissingExecution)
    }
}
