// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain genuine construction charges while exercising existing owner fixtures.
use super::*;
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::{JobOutcome, MemoryLease};
/// Wait on the real execution owner's existing completion notification.
pub(crate) trait ConstructionWake {
    /// Capture the existing notification revision before checking actual completion.
    fn revision(&self) -> u64;
    /// Await a changed revision with a bounded failure guard.
    fn wait_revision(&self, previous: u64) -> Result<(), Box<dyn Error>>;
}
/// Keep the actual completion charge fixed during capacity fixtures.
pub(crate) struct TableAdmission {
    pub(crate) memory: MemoryLease,
}
/// Construct through the original executor, then consume its actual typed Success.
pub(crate) fn construct(
    client: &ExecutionClient,
    scope: &Scope,
    spec: JobSpec,
    owner: CoordinatorKey,
    wake: &impl ConstructionWake,
    admissions: &mut Vec<TableAdmission>,
    mut consume: impl FnMut(&CompletionBatch) -> Result<(), Box<dyn Error>>,
) -> Result<WindowsProcessRegistry, Box<dyn Error>> {
    let mut job = WindowsProcessRegistry::submit_new(client, scope, spec, owner)?;
    loop {
        let revision = wake.revision();
        let batch =
            client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid constructor drain")?)?;
        consume(&batch)?;
        if let Some(completion) = job.try_take_completion() {
            if completion.scope_id != scope.id() || completion.spec != spec {
                return Err("Constructor terminal identity changed".into());
            }
            let registry = match completion.outcome {
                JobOutcome::Success(registry) => registry,
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                JobOutcome::Panicked(cause) => {
                    return Err(Box::new(RegistryError::owner_panic(
                        "registry_constructor",
                        cause,
                    )));
                }
                _ => return Err("Actual registry construction stopped".into()),
            };
            admissions.push(TableAdmission { memory: completion.memory });
            return Ok(registry);
        }
        wake.wait_revision(revision)?;
    }
}
/// Allocate the first fixed construction specification for a fresh real owner.
pub(crate) fn first_spec() -> Result<JobSpec, Box<dyn Error>> {
    Ok(JobSpec {
        id: JobId::new(1)?,
        operation: sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
        class: sugarsubstitute_contracts::execution::JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: WindowsProcessRegistry::required_reservation_bytes(),
        replacement: None,
    })
}
