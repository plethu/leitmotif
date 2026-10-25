mod backend;
mod encoding;
mod search;

#[cfg(test)]
mod tests;

use oxidd::error::OutOfMemory;
use std::{num::TryFromIntError, result::Result as StandardResult};

use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum Failure {
    #[error("BDD manager allocation limit reached")]
    Allocation(#[from] OutOfMemory),
    #[error(transparent)]
    Model(#[from] leitmotif::Error),
    #[error(transparent)]
    Integer(#[from] TryFromIntError),
    #[error("{0}")]
    Invalid(&'static str),
}

type Result<T> = StandardResult<T, Failure>;

pub(super) use backend::Oxidd;
pub(super) use encoding::{Encoding, Order};
pub(super) use search::{Outcome, explore, replay};
