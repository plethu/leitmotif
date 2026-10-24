use super::Error;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// Explicit deployment and soft per-check timeout for the optional Z3 experiment.
#[derive(Clone, Debug)]
pub struct Options {
    executable: String,
    timeout_ms: u32,
}
impl Options {
    /// Selects an executable and a positive whole-millisecond soft timeout.
    ///
    /// This bounds each solver check only. Startup, protocol I/O, model/core
    /// retrieval and synchronous process cleanup are outside that soft limit.
    /// It is not a hard deadline or a cancellation guarantee.
    ///
    /// # Errors
    /// Rejects empty/non-UTF-8 paths, fractional milliseconds, zero and timeouts
    /// at or beyond the solver's unbounded 32-bit sentinel.
    pub fn new(executable: impl Into<PathBuf>, soft_timeout: Duration) -> Result<Self, Error> {
        let executable = executable.into();
        let executable = executable
            .into_os_string()
            .into_string()
            .map_err(|_| Error::InvalidOptions("executable must be UTF-8"))?;
        if executable.is_empty() {
            return Err(Error::InvalidOptions("executable must be nonempty"));
        }
        let timeout_ms = u32::try_from(soft_timeout.as_millis())
            .map_err(|_| Error::InvalidOptions("soft timeout exceeds u32 milliseconds"))?;
        if (timeout_ms == 0 || timeout_ms == u32::MAX)
            || !soft_timeout.subsec_nanos().is_multiple_of(1_000_000)
        {
            return Err(Error::InvalidOptions(
                "soft timeout must be finite positive whole milliseconds",
            ));
        }
        Ok(Self {
            executable,
            timeout_ms,
        })
    }
    /// Executable explicitly selected by the caller.
    #[must_use]
    pub fn executable(&self) -> &Path {
        Path::new(&self.executable)
    }
    pub(super) fn command(&self) -> &str {
        &self.executable
    }
    /// Positive soft timeout applied separately to each satisfiability check.
    #[must_use]
    pub const fn soft_timeout_ms(&self) -> u32 {
        self.timeout_ms
    }
}
