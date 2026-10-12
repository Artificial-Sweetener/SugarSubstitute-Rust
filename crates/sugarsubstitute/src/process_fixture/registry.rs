// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe one actual admitted table constructor from the existing qualifier pump.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{JobOutcome, JobSpec, OwnedJob, Scope},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{RegistryError, WindowsProcessRegistry};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Retain the original constructor until its actual typed completion transfers.
pub(crate) struct PendingRegistry {
    job: OwnedJob<WindowsProcessRegistry, RegistryError>,
}
impl PendingRegistry {
    /// Admit the existing finite producer; the caller retains its original scope/deadline.
    pub(crate) fn submit(
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        owner: CoordinatorKey,
    ) -> Result<Self> {
        Ok(Self { job: WindowsProcessRegistry::submit_new(client, scope, spec, owner)? })
    }
    /// Request cancellation without claiming constructor or allocation retirement.
    pub(crate) fn cancel(&self) {
        self.job.cancel();
    }
}
/// Consume genuine constructor success in the same bounded shared completion turn.
/// Closing may observe actual cancellation/timeout; failures and panics keep their causes.
pub(crate) fn poll(
    pending: &mut Option<PendingRegistry>,
    registry: &mut Option<WindowsProcessRegistry>,
    closing: bool,
) -> Result<bool> {
    let Some(original) = pending.as_mut() else { return Ok(false) };
    let Some(completion) = original.job.try_take_completion() else { return Ok(false) };
    if completion.scope_id != original.job.scope_id() || completion.spec != original.job.spec() {
        return Err("Registry constructor terminal identity changed".into());
    }
    let job = pending.take().ok_or("Registry constructor custody disappeared")?;
    let outcome = match completion.outcome {
        JobOutcome::Success(mut value) => {
            if closing {
                value.close_launch_admission();
            }
            Some(value)
        }
        JobOutcome::Failure(cause) => return Err(Box::new(cause)),
        JobOutcome::Panicked(cause) => {
            return Err(Box::new(ConstructorPanic(std::io::Error::other(
                cause.as_str().to_owned(),
            ))));
        }
        JobOutcome::Cancelled | JobOutcome::TimedOut if closing => None,
        _ => return Err("Actual registry constructor stopped before required admission".into()),
    };
    drop(job);
    if registry.is_some() {
        return Err("Registry constructor attempted to replace live custody".into());
    }
    *registry = outcome;
    Ok(true)
}
/// Preserve the bounded original panic cause through qualifier error observation.
struct ConstructorPanic(std::io::Error);
impl fmt::Display for ConstructorPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Actual registry constructor panicked")
    }
}
impl fmt::Debug for ConstructorPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConstructorPanic { redacted }")
    }
}
impl Error for ConstructorPanic {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
