// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe independently admitted storage jobs before acknowledging native helper requests.
use super::*;
use crate::setup_storage::{SelectionStore, TRIAL_WITNESS_RESERVATION_BYTES};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
#[cfg(test)]
mod tests;

/// The helper lifetime owns this controller through job completion and native source retirement.
/// Admission and finalization use a separate ordinary scope, preserving the native pool and
/// allowing finalization after the native worker's cancellation. No job runs on the UI thread.
pub struct BackendCustodyInstallation {
    control: Control,
    job: Option<OwnedJob<(), ClaimError>>,
    terminal: Option<JobCompletion<(), ClaimError>>,
    operation: Option<Phase>,
}
impl BackendCustodyInstallation {
    pub(super) fn new(control: Control) -> Self {
        Self { control, job: None, terminal: None, operation: None }
    }

    /// Consume the actual matching completion before publishing Ready or Settled to native code.
    /// # Errors
    /// Preserves original failures; forged notices or discarded results never confirm ownership.
    pub fn observe(&mut self, batch: &CompletionBatch) -> Result<(), ClaimError> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        let Some(notice) = batch
            .notifications
            .iter()
            .find(|notice| (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id))
        else {
            return Ok(());
        };
        let Some(done) = job.try_take_completion() else {
            return Ok(());
        };
        let valid = done.scope_id == job.scope_id()
            && done.spec == job.spec()
            && done.outcome.kind() == notice.kind;
        let success = matches!(done.outcome, JobOutcome::Success(()));
        let cause = match &done.outcome {
            JobOutcome::Failure(cause) => Some(cause.clone()),
            _ if !valid => Some(ClaimError::invalid("installation_witness_completion_identity")),
            _ if !success => Some(ClaimError::new(
                "installation_witness_terminal",
                std::io::Error::other(format!("Storage job ended as {:?}", done.outcome)),
            )),
            _ => None,
        };
        self.terminal = Some(done);
        self.job = None;
        if let Some(cause) = cause {
            return self.control.fail(cause);
        }
        let wake = {
            let mut data = crate::registry::lock(&self.control.0.data);
            data.phase = match self.operation {
                Some(Phase::Admitting)
                    if data.phase == Phase::Admitting && data.witness.is_some() =>
                {
                    Phase::Ready
                }
                Some(Phase::Settling) if data.phase == Phase::Settling => Phase::Settled,
                _ => {
                    drop(data);
                    let cause = ClaimError::invalid("installation_witness_completion_phase");
                    return self.control.fail(cause);
                }
            };
            data.native_wake.clone()
        };
        if let Some(wake) = wake
            && let Err(cause) = wake.wake()
        {
            return self.control.fail(ClaimError::new("installation_witness_native_wake", cause));
        }
        Ok(())
    }

    /// Submit a requested storage operation once under its original native deadline.
    /// `admission` and `settlement` are distinct unused job identities from this execution owner.
    /// # Errors
    /// Retains input/witness and the original admission cause; no deadline is renewed on failure.
    pub fn advance(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        admission: JobId,
        settlement: JobId,
    ) -> Result<(), ClaimError> {
        if self.job.is_some() {
            return Ok(());
        }
        let (operation, until) = {
            let mut data = crate::registry::lock(&self.control.0.data);
            if let Some(cause) = &data.failure {
                return Err(cause.clone());
            }
            let operation = match data.phase {
                Phase::AdmissionRequested => Phase::Admitting,
                Phase::SettlementRequested => Phase::Settling,
                _ => return Ok(()),
            };
            let until =
                data.until.ok_or_else(|| ClaimError::invalid("installation_witness_deadline"))?;
            data.phase = operation;
            (operation, until)
        };
        let id = if operation == Phase::Admitting { admission } else { settlement };
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            reservation_bytes: TRIAL_WITNESS_RESERVATION_BYTES,
            deadline: Some(until),
            replacement: None,
        };
        let control = self.control.clone();
        let submitted =
            client.submit_blocking(scope, spec, move |context| run(&control, operation, &context));
        match submitted {
            Ok(job) => {
                self.terminal = None;
                self.operation = Some(operation);
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                let cause = ClaimError::new("installation_witness_job_admission", cause);
                self.control.fail(cause)
            }
        }
    }
}

/// Keep the witness in shared custody before returning; discarded completion cannot drop it.
fn run(control: &Control, operation: Phase, context: &JobContext) -> Result<(), ClaimError> {
    if operation == Phase::Admitting {
        let input = crate::registry::lock(&control.0.data)
            .input
            .take()
            .ok_or_else(|| ClaimError::invalid("installation_witness_input"))?;
        let generation = &control.0.generation;
        let root = generation
            .parent()
            .filter(|parent| {
                parent
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("releases"))
            })
            .and_then(std::path::Path::parent)
            .ok_or_else(|| ClaimError::invalid("installation_witness_layout"))?;
        let store = Arc::new(
            SelectionStore::open(root, context)
                .map_err(|cause| ClaimError::new("installation_witness_store", cause))?,
        );
        let witness = TrialWitness::receive_transferred(store, input, generation, context)
            .map_err(|cause| ClaimError::new("installation_witness_receive", cause))?;
        crate::registry::lock(&control.0.data).witness = Some(Arc::new(witness));
        Ok(())
    } else {
        let (witness, exit) = {
            let data = crate::registry::lock(&control.0.data);
            (data.witness.clone(), data.exit)
        };
        let witness =
            witness.ok_or_else(|| ClaimError::invalid("installation_witness_settlement_owner"))?;
        let exit = exit.ok_or_else(|| ClaimError::invalid("installation_witness_host_exit"))?;
        witness
            .finish_settled(exit.verdict(), context)
            .map(|_| ())
            .map_err(|cause| ClaimError::new("installation_witness_finalization", cause))
    }
}
