// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform finite seal/inventory/force/closure steps with retained actual observations.
use super::*;
use sugarsubstitute_execution::windows::NativeCorrelation;

impl WindowsProcessRegistry {
    /// Admit one finite cleanup step, retaining its unique typed job in this registry.
    /// An optional member-watch specification is required when the cached complete
    /// inventory needs a secondary observation. Its ID must precede this step's ID.
    /// The global64 watch reserve admits at most one secondary watch per family;
    /// rejection retains its original member handle and immutable shutdown budget.
    /// Retry after execution activity/capacity notifications, without sleeps/polling.
    /// Existing native scopes outlive the15s policy budget so late actual exit can
    /// still be observed. The caller schedules finite steps at the same clock's
    /// original grace/total boundaries; no deadline or transaction is reset here.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_cleanup_step(
        &mut self,
        execution: &ExecutionClient,
        key: ProcessKey,
        cleanup_scope: &Scope,
        spec: JobSpec,
        member_watch: Option<JobSpec>,
    ) -> Result<JobId, RegistryAdmissionError> {
        submission::validate_blocking(spec)?;
        if spec.reservation_bytes < 4096 {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(watch) = member_watch {
            submission::validate_watch(watch)?;
            if watch.id >= spec.id {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        }
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if cleanup_scope.owner_id() != record.native_scope.owner_id() {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        let (_, budget) = record.stop.ok_or(RegistryAdmissionError::Closed)?;
        if record.creation.is_some()
            || record.establishment_job.is_some()
            || record.cleanup.is_some()
            || (record.settled && !record.custody_retirement_pending())
        {
            return Err(RegistryAdmissionError::Busy);
        }
        let candidate = if record.member_watch.is_none() { record.next } else { None };
        if candidate.is_some() && member_watch.is_none() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if candidate.is_none() && member_watch.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let (Some(watch), Some((_, identity))) = (member_watch, candidate) {
            if self.cleanup_watches == 64 {
                return Err(RegistryAdmissionError::Saturated);
            }
            let correlation = NativeCorrelation::new(key.id.get())
                .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
            let permit = execution
                .reserve_process_watch(&record.native_scope, watch, correlation)
                .map_err(RegistryAdmissionError::Execution)?;
            *lock(&record.shared.permit) = Some(permit);
            record.member_spec = Some((record.native_scope.id(), watch.id));
            record.watched = Some(identity);
            self.cleanup_watches += 1;
        }
        let shared = record.shared.clone();
        let root_exit = record
            .root_exit
            .map(|exit| Identity { pid: exit.process_id, birth: exit.creation_time_100ns });
        let member_exit = record.pending_exit;
        let root_watch_active = record.root_watch.is_some();
        let job = execution.submit_blocking(cleanup_scope, spec, move |context| {
            cleanup_owned(
                shared,
                budget,
                root_exit,
                member_exit,
                candidate,
                root_watch_active,
                &context,
            )
        });
        match job {
            Ok(job) => {
                record.cleanup = Some(job);
                // This step observes every fact retained so far; only a later
                // watch outcome, failure or recheck makes another step useful.
                record.cleanup_progress = CleanupProgress::Live;
                Ok(spec.id)
            }
            Err(cause) => {
                if let Some(permit) = lock(&record.shared.permit).take() {
                    record.member_watch = Some(permit.cancel_unbound());
                    record.watched = None;
                }
                Err(RegistryAdmissionError::Execution(cause))
            }
        }
    }
    /// Select the next current stop record needing cleanup, in fixed round-robin order.
    /// This is a cached scheduling hint, never a reservation or native completion.
    pub fn next_cleanup(&mut self) -> Option<ProcessKey> {
        for _ in 0..self.records.len() {
            let index = self.cursor;
            self.cursor = (self.cursor + 1) % self.records.len();
            if let Some(record) = &self.records[index]
                && record.stop.is_some()
                && !record.settled
                && record.creation.is_none()
                && record.cleanup.is_none()
            {
                return Some(record.ticket.process);
            }
        }
        None
    }
}
fn cleanup_owned(
    shared: SharedOwner,
    budget: ShutdownBudget,
    root_exit: Option<Identity>,
    member_exit: Option<Identity>,
    candidate: Option<(usize, Identity)>,
    root_watch_active: bool,
    context: &JobContext,
) -> Result<(), RegistryError> {
    let mut state = lock(&shared.native);
    let mut force_attempted = false;
    let mut result = cleanup_step(
        &shared,
        &mut state,
        budget,
        root_exit,
        member_exit,
        candidate,
        root_watch_active,
        context,
        &mut force_attempted,
    );
    if let Err(cause) = &mut result
        && let Some(family) = state.family.as_mut()
        && !force_attempted
    {
        force_after_failure(&shared, family, context.now(), budget, cause);
    }
    if let Err(cause) = &result {
        state.failure = Some(cause.clone());
    }
    state.forced |= state.family.as_ref().is_some_and(|family| family.forced);
    result
}
#[allow(clippy::too_many_arguments)]
fn cleanup_step(
    shared: &Shared,
    state: &mut NativeState,
    budget: ShutdownBudget,
    root_exit: Option<Identity>,
    member_exit: Option<Identity>,
    candidate: Option<(usize, Identity)>,
    root_watch_active: bool,
    context: &JobContext,
    force_attempted: &mut bool,
) -> Result<(), RegistryError> {
    let Some(family) = state.family.as_mut() else {
        retire_custody_metadata(state, context)?;
        state.cleanup = Some(CleanupFacts::CLOSED);
        return Ok(());
    };
    if let Some(exit) = root_exit {
        family.record_exit(exit)?;
    }
    if let Some(exit) = member_exit {
        family.record_exit(exit)?;
    }
    if family.count == 0 || family.closure_committed {
        family.closure_committed = true;
        shared.port.close(family)?;
        if !family.preparation_image.as_ref().is_some_and(|launch| launch.retains_settled_payload())
        {
            state.family = None;
        }
        retire_custody_metadata(state, context)?;
        state.cleanup = Some(CleanupFacts::CLOSED);
        return Ok(());
    }
    let now = context.now();
    if now < budget.started {
        return Err(RegistryError::invalid("shutdown_clock_regression"));
    }
    if !family.sealed {
        shared.port.seal(family)?;
    }
    let inventory = shared.port.inventory(family)?;
    let mut watched = None;
    if let Some((index, expected)) = candidate {
        if family.member(index)?.identity != expected {
            return Err(RegistryError::invalid("cleanup_watch_identity"));
        }
        let duplicate = shared.port.duplicate(family, index)?;
        let permit = lock(&shared.permit)
            .take()
            .ok_or_else(|| RegistryError::invalid("cleanup_watch_reservation"))?;
        state.watch = creation::bind(permit, duplicate);
        watched = Some(expected);
    }
    let after_native = context.now();
    if after_native < now {
        return Err(RegistryError::invalid("shutdown_clock_regression"));
    }
    if after_native >= budget.graceful_until && !family.forced && !inventory.empty {
        *force_attempted = true;
        shared.port.terminate(family)?;
    }
    let closed = inventory.empty && family.all_exited() && !family.incomplete;
    let root = family.root;
    state.forced |= family.forced;
    let next = family.next_member(root_watch_active);
    let recheck = !closed && next.is_none() && family.all_exited() && !family.incomplete;
    if closed {
        // Empty membership and every retained actual exit are now immutable:
        // subsequent steps retry only outstanding closes after partial success.
        family.closure_committed = true;
        shared.port.close(family)?;
        if !family.preparation_image.as_ref().is_some_and(|launch| launch.retains_settled_payload())
        {
            state.family = None;
        }
        retire_custody_metadata(state, context)?;
    }
    state.cleanup = Some(CleanupFacts {
        retained: inventory.count,
        root,
        next,
        watched,
        closed,
        empty: inventory.empty,
        recheck,
    });
    Ok(())
}
fn force_after_failure(
    shared: &Shared,
    family: &mut Family,
    now: sugarsubstitute_contracts::execution::Moment,
    budget: ShutdownBudget,
    cause: &mut RegistryError,
) {
    if now >= budget.graceful_until
        && !family.forced
        && family.job.is_some()
        && let Err(next) = shared.port.terminate(family)
    {
        cause.retain(next);
    }
}
/// Physical local family absence precedes retirement of escaped destination outputs.
fn retire_custody_metadata(
    state: &mut NativeState,
    context: &JobContext,
) -> Result<(), RegistryError> {
    if state.custody_destination.is_none() && state.establishment.is_none() {
        return Ok(());
    }
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(RegistryError::invalid("custody_retirement_cutoff"));
    }
    if let Some(destination) = &mut state.custody_destination {
        if let Some(ack) = &state.custody_release {
            let sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::Released(
                resource,
            ) = ack.receipt()
            else {
                return Err(RegistryError::invalid("custody_retirement_receipt"));
            };
            destination.close_released(ack, resource)?;
        } else {
            // A living helper may still own an unacknowledged output. Local family
            // absence permits writer retirement while preserving this exact capsule.
            if !destination.close_after_exit()? {
                return Ok(());
            }
        }
        state.custody_destination = None;
    }
    if let Some(establishment) = &mut state.establishment {
        establishment.close_after_exit()?;
        state.establishment = None;
    }
    state.custody_release = None;
    Ok(())
}
