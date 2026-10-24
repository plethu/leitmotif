use std::error::Error;

use leitmotif::{Fact, FactHandle, Model, ModelBuilder, State, Transition};
use leitmotif_recite::{Bindings, DeferredEffects, ImmediateEffects};
use recite_core::compiled::{CompiledConditionCall, CompiledDialogue};
use recite_runtime::{
    DialogueEffectRequest, DialogueEvent, DialogueSession, choose, next, start_scene,
};

use super::super::super::support::{Enabled, Other};

pub(super) const STORY: &str = concat!(
    ":: start default\n",
    "! deferred grant(player)\n",
    "! immediate cue(\"bell\")\n",
    ":if ready()\n  ! deferred impossible()\n",
    ":else\n  ! deferred record_else()\n",
    "> prompt@8843fd6f53f020a12b31\n  Continue?\n",
    "  ? continue@4dff358182d02d9090e3 requires=(not ready())\n    Continue.\n    -> finish\n",
    "  ? abandon@a179e0df75e958fa949b\n    Abandon.\n    -> abandon\n",
    ":: finish\n! deferred finish()\n-> END\n",
    ":: abandon\n! deferred abandoned()\n-> END\n",
);

pub(super) struct Visited;
impl Fact for Visited {
    type Value = bool;
    const KEY: &'static str = "visited";
}
pub(super) struct Finished;
impl Fact for Finished {
    type Value = bool;
    const KEY: &'static str = "finished";
}

pub(super) struct Host {
    pub(super) model: Model,
    pub(super) initial: State,
    pub(super) conditions: Bindings,
    pub(super) enabled: FactHandle<Enabled>,
    pub(super) other: FactHandle<Other>,
    pub(super) visited: FactHandle<Visited>,
    pub(super) finished: FactHandle<Finished>,
}

impl Host {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let mut builder = ModelBuilder::new("mixed-deferred")?;
        let enabled = builder.fact::<Enabled>()?;
        let other = builder.fact::<Other>()?;
        let visited = builder.fact::<Visited>()?;
        let finished = builder.fact::<Finished>()?;
        let model = builder
            .with_transition(
                Transition::new("grant")
                    .with_guard(enabled.is(false))
                    .with_effect(enabled.set(true)),
            )
            .with_transition(Transition::new("cue").with_effect(other.set(true)))
            .with_transition(
                Transition::new("record_else")
                    .with_guard(enabled.is(true))
                    .with_effect(visited.set(true)),
            )
            .with_transition(
                Transition::new("finish")
                    .with_guard(visited.is(true))
                    .with_effect(finished.set(true)),
            )
            .build()?;
        let initial = model.state([
            enabled.set(false),
            other.set(false),
            visited.set(false),
            finished.set(false),
        ])?;
        let mut conditions = Bindings::new();
        conditions.bind_bool(
            CompiledConditionCall {
                function: "ready".into(),
                args: vec![],
            },
            &enabled,
        )?;
        Ok(Self {
            model,
            initial,
            conditions,
            enabled,
            other,
            visited,
            finished,
        })
    }

    pub(super) fn deferred(
        &self,
        asset: &CompiledDialogue,
    ) -> Result<DeferredEffects<'_>, Box<dyn Error>> {
        let mut effects = DeferredEffects::new(&self.model);
        for name in ["grant", "record_else", "finish"] {
            effects.bind(
                asset
                    .effects
                    .iter()
                    .find(|effect| effect.function == name)
                    .ok_or("missing fixture effect")?,
                name,
            )?;
        }
        Ok(effects)
    }
}

impl Host {
    pub(super) fn finish_reached_path(
        &self,
        asset: &CompiledDialogue,
    ) -> Result<(DialogueSession, State, Vec<DialogueEffectRequest>), Box<dyn Error>> {
        let mut immediate = ImmediateEffects::new(&self.model);
        immediate.bind(
            asset
                .effects
                .iter()
                .find(|effect| effect.function == "cue")
                .ok_or("missing cue")?,
            "cue",
        )?;
        let mut session = start_scene(asset, None)?;
        let DialogueEvent::Effect(cue) = next(
            asset,
            &mut session,
            &self.conditions.context(&self.model, &self.initial),
        )?
        else {
            return Err("expected cue".into());
        };
        let state = immediate.apply(&cue, &self.initial)?.into_state();
        let DialogueEvent::Prompt { choices, .. } = next(
            asset,
            &mut session,
            &self.conditions.context(&self.model, &state),
        )?
        else {
            return Err("expected prompt".into());
        };
        let choice = choices.first().ok_or("missing continue choice")?;
        let DialogueEvent::End { deferred_effects } = choose(
            asset,
            &mut session,
            choice.id.clone(),
            &self.conditions.context(&self.model, &state),
        )?
        else {
            return Err("expected End".into());
        };
        Ok((session, state, deferred_effects))
    }
}
