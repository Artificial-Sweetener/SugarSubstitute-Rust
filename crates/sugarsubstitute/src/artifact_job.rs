// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit qualification artifact work through execution and observe actual post-UI retirement.
mod lifetime;
mod outputs;
#[cfg(not(windows))]
mod portable;
#[cfg(test)]
mod tests;
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) use outputs::write_bytes;
pub(crate) use outputs::{write_frame, write_json};
use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Artifact roles use finite fixed policies, independent of native fixture execution grants.
pub(crate) enum Kind {
    Record,
    Raster,
}
impl Kind {
    fn bytes(&self) -> u64 {
        match self {
            Self::Record => 8 * 1024 * 1024,
            Self::Raster => 192 * 1024 * 1024,
        }
    }
    fn duration(&self) -> Duration {
        match self {
            Self::Record => Duration::from_secs(30),
            Self::Raster => Duration::from_secs(2 * 60 * 60),
        }
    }
}

/// Preserve concrete producer/storage causes in the execution owner's typed result.
#[derive(Debug)]
pub(crate) struct WorkError(Box<dyn Error + Send + Sync>);
impl WorkError {
    pub(crate) fn boxed(cause: Box<dyn Error + Send + Sync>) -> Self {
        Self(cause)
    }
    pub(crate) fn new(cause: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(cause))
    }
}
impl fmt::Display for WorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl Error for WorkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// Run only off-screen qualification or post-native-teardown artifact work.
/// Windows keeps the existing bootstrap message pump alive until jobs and workers actually
/// retire, without constructing another Tao loop. No scheduler/runtime is exposed to producers.
pub(crate) fn run<T: Send + 'static>(
    kind: Kind,
    work: impl FnOnce(JobContext) -> std::result::Result<T, WorkError> + Send + 'static,
) -> Result<T> {
    let mut lifetime = Lifetime::new(kind, work)?;
    #[cfg(windows)]
    let native = sugarsubstitute_desktop::run_headless(&mut lifetime)
        .map_err(|cause| Box::new(cause) as Box<dyn Error>);
    #[cfg(not(windows))]
    let native = portable::run(&mut lifetime);
    match (native, lifetime.result.take()) {
        (Ok(()), Some(result)) => result,
        (Err(retirement), Some(Err(primary))) => Err(Box::new(Failure { primary, retirement })),
        (Err(cause), _) => Err(cause),
        (Ok(()), None) => Err("Artifact work produced no terminal outcome".into()),
    }
}

#[derive(Debug)]
struct Failure {
    primary: Box<dyn Error>,
    retirement: Box<dyn Error>,
}
impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; artifact lifetime failed: {}", self.primary, self.retirement)
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref())
    }
}

type Producer<T> = Box<dyn FnOnce(JobContext) -> std::result::Result<T, WorkError> + Send>;
struct Lifetime<T> {
    clock: Arc<MonotonicClock>,
    deadline: Instant,
    until: Moment,
    bytes: u64,
    producer: Option<Producer<T>>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    job: Option<OwnedJob<T, WorkError>>,
    result: Option<Result<T>>,
    closing: bool,
    expiry_observed: bool,
}
impl<T: Send + 'static> Lifetime<T> {
    fn new(
        kind: Kind,
        producer: impl FnOnce(JobContext) -> std::result::Result<T, WorkError> + Send + 'static,
    ) -> Result<Self> {
        let clock = MonotonicClock::new();
        let duration = kind.duration();
        let deadline = Instant::now().checked_add(duration).ok_or("Artifact deadline overflow")?;
        let until = clock.now().checked_add(duration)?;
        Ok(Self {
            clock,
            deadline,
            until,
            bytes: kind.bytes(),
            producer: Some(Box::new(producer)),
            owner: None,
            client: None,
            scope: None,
            job: None,
            result: None,
            closing: false,
            expiry_observed: false,
        })
    }
    fn advance(&mut self, maximum: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            client.drain_completions(maximum)?;
        }
        if let Some(completion) = self.job.as_mut().and_then(OwnedJob::try_take_completion) {
            self.result = Some(match completion.outcome {
                JobOutcome::Success(value) => Ok(value),
                JobOutcome::Failure(cause) => Err(Box::new(cause)),
                other => Err(Box::new(std::io::Error::other(format!(
                    "Artifact job stopped: {:?}",
                    other.kind()
                )))),
            });
            self.job = None;
        }
        if self.result.is_some() {
            return Ok(HeadlessStep::Shutdown);
        }
        if !self.expiry_observed && Instant::now() >= self.deadline {
            self.expiry_observed = true;
            if let Some(job) = &self.job {
                job.cancel();
            }
            return Err(
                "Artifact work exceeded its original deadline; native work remains owned".into()
            );
        }
        Ok(HeadlessStep::Active)
    }
}
