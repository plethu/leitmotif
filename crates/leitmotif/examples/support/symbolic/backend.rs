use std::{collections::hash_map::RandomState, result::Result as StandardResult};

use oxidd::{
    BooleanFunction, BooleanFunctionQuant, FunctionSubst, Manager, ManagerRef, Subst,
    bdd::{BDDFunction, BDDManagerRef, new_manager},
    util::{OptBool, SatCountCache, num::F64},
};

use super::{Failure, Result};

pub(in super::super) struct Oxidd {
    manager: BDDManagerRef,
    variables: Vec<BDDFunction>,
}

impl Oxidd {
    pub(in super::super) fn new(variables: u32, capacity: usize) -> Result<Self> {
        let manager = new_manager(capacity, 65_536, 1);
        let variables = manager.with_manager_exclusive(|manager| {
            manager
                .add_vars(variables)
                .map(|id| BDDFunction::var(manager, id))
                .collect::<StandardResult<Vec<_>, _>>()
        })?;
        Ok(Self { manager, variables })
    }

    pub(in super::super) fn constant(&self, value: bool) -> BDDFunction {
        self.manager.with_manager_shared(|manager| {
            if value {
                BDDFunction::t(manager)
            } else {
                BDDFunction::f(manager)
            }
        })
    }
    pub(in super::super) fn variable(&self, index: u32) -> Result<BDDFunction> {
        self.variables
            .get(usize::try_from(index)?)
            .cloned()
            .ok_or(Failure::Invalid("unknown BDD variable"))
    }

    pub(in super::super) fn exists(
        &self,
        value: &BDDFunction,
        variables: &[u32],
    ) -> Result<BDDFunction> {
        let mut cube = self.constant(true);
        for id in variables {
            cube = cube.and(&self.variable(*id)?)?;
        }
        Ok(value.exists(&cube)?)
    }
    pub(in super::super) fn rename(
        &self,
        value: &BDDFunction,
        pairs: &[(u32, u32)],
    ) -> Result<BDDFunction> {
        let mut from = Vec::new();
        let mut to = Vec::new();
        for (source, target) in pairs {
            from.push(*source);
            to.push(self.variable(*target)?);
        }
        Ok(value.substitute(&Subst::new(from, to))?)
    }
    pub(in super::super) fn pick(value: &BDDFunction, variables: u32) -> Result<Option<Vec<bool>>> {
        value
            .pick_cube(|_, _, _| false)
            .map(|cube| {
                (0..variables)
                    .map(|id| Ok(cube.get(usize::try_from(id)?) == Some(&OptBool::True)))
                    .collect()
            })
            .transpose()
    }
    pub(in super::super) fn count(value: &BDDFunction, variables: u32) -> f64 {
        value
            .sat_count(variables, &mut SatCountCache::<F64, RandomState>::default())
            .0
    }

    pub(in super::super) fn allocated(&self) -> usize {
        self.manager
            .with_manager_shared(|manager| manager.num_inner_nodes())
    }
}
