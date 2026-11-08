// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue a once-only recovery grant from the genuine completed Root startup archive.
//! Serialized keys remain inert. This source retains actual issuer, memory, Root Entry
//! and scope custody; only the same execution owner may admit its native consumers.
use super::*;
use crate::ExecutionClient;
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobCompletion, JobContext, JobPhase, MemoryClaim, TerminalKind},
    recovery_supervisor::RecoveryGrantKey,
};

/// Return the complete unchanged input when the once-only Root grant cannot issue.
pub struct RecoveryGrantRejection {
    /// The genuine source, clock, scope, memory or terminal association rejection.
    pub error: AdmissionError,
    /// The original prepared snapshot, still responsible for its retained source.
    pub snapshot: PreparedStartupSnapshot,
}
impl std::fmt::Debug for RecoveryGrantRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryGrantRejection { redacted }")
    }
}
impl std::fmt::Display for RecoveryGrantRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Registered recovery grant rejected")
    }
}
impl std::error::Error for RecoveryGrantRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

pub(crate) struct GrantState {
    pub(crate) key: RecoveryGrantKey,
    registered: RegisteredSnapshot,
    completed_memory: sugarsubstitute_contracts::execution::MemoryLease,
    pub(crate) stages: Mutex<Stages>,
    _claim: MemoryClaim,
}
#[derive(Default)]
pub(crate) struct Stages {
    pub(crate) pipe: Option<JobId>,
    pub(crate) creation: Option<JobId>,
    pub(crate) committed: bool,
    pub(crate) closed: bool,
}
/// Retain one genuine Root-only successor grant and its original projection credit.
/// Clones alias this same source and never create another pipe, launch or generation.
/// Dropping aliases is not process, profile, capture or transport retirement proof.
/// Every final control allocation is destroyed before its claimed payload; no raw
/// Arc or Weak alias may bypass this closed destruction order.
#[derive(Clone)]
pub struct RegisteredRecoveryGrant {
    owner: Option<Arc<GrantState>>,
    key: RecoveryGrantKey,
    original: EntrySetupBudget,
    startup_bytes: usize,
}
/// Retain the exact genuine grant allocation behind a dependent control allocation.
/// This closed alias grants no admission, identity projection or retirement proof.
/// Its final Arc control is destroyed before the original claimed payload; aliases
/// cannot project a raw Arc or Weak that would bypass that destruction order.
#[derive(Clone)]
pub struct RecoveryGrantCustody(Option<Arc<GrantState>>);
impl RecoveryGrantCustody {
    fn take_owner(&mut self) -> Option<GrantState> {
        self.0.take().and_then(Arc::into_inner)
    }
}
impl Drop for RecoveryGrantCustody {
    fn drop(&mut self) {
        drop(self.take_owner());
    }
}
impl Drop for RegisteredRecoveryGrant {
    fn drop(&mut self) {
        drop(self.take_owner());
    }
}
impl RegisteredRecoveryGrant {
    /// Transfer this genuine alias into compact custody without adding authority.
    /// Keep the returned owner outside dependent allocations so their complete
    /// destruction precedes release of the same original grant/control claim.
    pub fn into_custody(mut self) -> RecoveryGrantCustody {
        RecoveryGrantCustody(self.owner.take())
    }
    /// Retire this alias's control before dropping the returned final claimed payload.
    /// Every alias stays inside this closed owner; no Weak or raw Arc is projected.
    pub(super) fn take_owner(&mut self) -> Option<GrantState> {
        self.owner.take().and_then(Arc::into_inner)
    }
    /// Borrow actual private source custody without exposing a raw Arc alias.
    pub(crate) fn state(&self) -> Result<&GrantState, AdmissionError> {
        self.owner.as_deref().ok_or(AdmissionError::InvalidCaptureAdmission)
    }
    /// Return the exact retained startup archive length without copying or granting authority.
    pub fn startup_bytes(&self) -> usize {
        self.startup_bytes
    }
    /// Compare the identical once-only process-local grant allocation without provider effects.
    pub fn same_grant(&self, other: &Self) -> bool {
        self.owner.as_ref().zip(other.owner.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }
    /// Return actual compiled grant/control storage for its existing4KiB preflight.
    /// The same original grant custody stays outside any dependent held-control Box.
    pub const fn control_storage_bytes() -> usize {
        size_of::<GrantState>() + 2 * size_of::<usize>()
    }
    /// Return passive complete binding facts; these cannot reconstruct this grant.
    pub fn key(&self) -> RecoveryGrantKey {
        self.key
    }
    /// Return the actual validated execution owner as passive correlation, never a new grant.
    /// # Errors
    /// Preserves actual source/provider/cancellation rejection.
    pub fn execution_id(
        &self,
    ) -> Result<sugarsubstitute_contracts::execution::ExecutionId, AdmissionError> {
        self.check()?;
        let source = self.state()?.registered.source.state().map_err(admission::transport)?;
        Ok(source.core.id)
    }
    /// Compare the identical genuine original startup archive without manufacturing admission.
    pub fn same_startup_source(&self, source: &RegisteredStartupSource) -> bool {
        self.state().is_ok_and(|owner| owner.registered.source.same_source(source))
    }
    /// Return the original coordinator-issued thirty-second setup interval.
    pub fn original_budget(&self) -> EntrySetupBudget {
        self.original
    }
    /// Borrow only the unchanged authenticated Launch shape, without new authority.
    /// # Errors
    /// Rejects an invalid original archive shape. Native consumers separately fence
    /// actual execution, Root Entry, cancellation and current generation.
    pub fn view(
        &self,
    ) -> Result<
        sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequestView<'_>,
        EntryTransportError,
    > {
        let owner = self.owner.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
        owner.registered.source.view()
    }
    /// Bind this genuine Root grant to one actually running finite owner on its own Core.
    /// # Errors
    /// Rejects caller-built/foreign contexts, stopped source or cancelled/expired records.
    pub fn check_context(&self, context: &JobContext) -> Result<(), AdmissionError> {
        let source = self.state()?.registered.source.state().map_err(admission::transport)?;
        let now = source.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&source.core.state);
        self.state()?.validate_locked(&source.core, &state, now)?;
        let record =
            state.jobs.get(&context.job_id()).ok_or(AdmissionError::InvalidCaptureAdmission)?;
        if context.owner_id() != source.core.id
            || record.phase != JobPhase::Running
            || record.retiring
            || record.scope_id != context.scope_id()
            || record.spec != context.spec()
            || record.memory.as_ref().is_none_or(|memory| !memory.matches_lease(context.memory()))
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cutoff = crate::admission_liveness::scope_deadline(&source.core, &state, record)?;
        if let Some(deadline) = cutoff.filter(|until| *until <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        Ok(())
    }
    /// Close new effect admission while retaining every genuine native consumer.
    /// The execution owner still retires queued factories and active workers normally.
    pub fn close_admission(&self) {
        let Some(owner) = self.owner.as_deref() else {
            return;
        };
        let source = owner.registered.source.0.as_ref();
        if let Some(source) = source {
            let state = lock(&source.core.state);
            let mut stages = lock(&owner.stages);
            stages.closed = true;
            let jobs = [stages.pipe, stages.creation];
            drop(stages);
            drop(state);
            for job in jobs.into_iter().flatten() {
                source.core.cancel_job(job);
            }
            source.core.changed();
        }
    }
    /// Check the same original Root custody immediately before a use-case effect.
    /// # Errors
    /// Preserves actual clock/source rejection. No deadline is renewed.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let source = self.state()?.registered.source.state().map_err(admission::transport)?;
        let now = source.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&source.core.state);
        self.state()?.validate_locked(&source.core, &state, now).map(|_| ())
    }
}
impl GrantState {
    pub(crate) fn startup_body(&self) -> &[u8] {
        self.registered.body()
    }
    pub(crate) fn original_until(&self) -> Moment {
        self.registered.original.until()
    }

    /// Revalidate actual retained Root/issuer custody inside final Core admission.
    pub(crate) fn validate_locked(
        &self,
        core: &Core,
        state: &State,
        now: Moment,
    ) -> Result<Moment, AdmissionError> {
        let original = &self.registered;
        let source = original.source.state().map_err(admission::transport)?;
        let status = original._issuer.status();
        if state.stopping
            || core.clock.failed()
            || self.key.previous() != original.previous
            || self.key.run() != original.run
            || status.phase != JobPhase::Terminal(TerminalKind::Success)
            || status.cancellation_requested
            || !original._memory.same_reservation(&self.completed_memory)
            || !Arc::ptr_eq(&original.cancel, &original._issuer.cancellation())
            || lock(&source.progress).recovery_issued != Some(self.key)
            || lock(&self.stages).closed
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        if !std::ptr::eq(source.core.as_ref(), core) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        if !lock(&self.stages).committed {
            return original
                .validate_current_locked(&source.core, state, original.run, now)
                .map_err(admission::transport);
        }
        let progress = lock(&source.progress);
        if progress.current != original.run
            || progress.issued != Some((original.job, original.scope, original.original))
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        drop(progress);
        let owner = source
            .entry
            .validate_registered_owner_locked(&source.core, state, original.run, now)
            .map_err(admission::transport)?;
        let original_scope =
            admission::scope_current(state, source.original_scope).map_err(admission::transport)?;
        let receiving_scope =
            admission::scope_current(state, original.scope).map_err(admission::transport)?;
        let cutoff = [owner, original_scope, receiving_scope].into_iter().flatten().min();
        if let Some(deadline) = cutoff.filter(|until| *until <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        Ok(cutoff.unwrap_or(Moment::from_nanos(u64::MAX)))
    }
}
impl ExecutionClient {
    /// Consume the real completed next-generation snapshot into one recovery grant.
    /// The original Root Entry remains live and authoritative. This grants no ACK,
    /// original-family retirement, profile claim, process creation or native capture.
    /// Those owners must independently admit and settle before the grant is committed.
    /// The fixed 4096-byte control claim comes from the original four-MiB projection.
    /// # Errors
    /// Returns the identical snapshot on foreign owner/source/completion, repeat key
    /// or generation, failed actual issuer, cancellation, current expiry or exhaustion.
    #[expect(clippy::result_large_err, reason = "Preserve original snapshot custody inline")]
    pub fn issue_registered_recovery_grant<T, E>(
        &self,
        snapshot: PreparedStartupSnapshot,
        source: &RegisteredStartupSource,
        completed: &JobCompletion<T, E>,
        key: RecoveryGrantKey,
    ) -> Result<RegisteredRecoveryGrant, RecoveryGrantRejection> {
        let decision = (|| {
            let witness = self.borrow_registered_restart_admission(&snapshot, source, completed)?;
            witness.check()?;
            if key.previous() != witness.previous()
                || key.run() != witness.run()
                || size_of::<GrantState>() + 2 * size_of::<usize>() > 4096
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
            let state = lock(&self.core.state);
            let original =
                snapshot.registered.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
            original
                .validate_current_locked(&self.core, &state, key.run(), now)
                .map_err(admission::transport)?;
            let data = source.state().map_err(admission::transport)?;
            let mut progress = lock(&data.progress);
            if progress.recovery_issued.is_some_and(|issued| issued.run() == key.run()) {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let claim = completed.memory.claim(4096).map_err(AdmissionError::Memory)?;
            progress.recovery_issued = Some(key);
            Ok((original.clone(), claim))
        })();
        match decision {
            Err(error) => Err(RecoveryGrantRejection { error, snapshot }),
            Ok((registered, claim)) => {
                drop(snapshot);
                let original = registered.original;
                let startup_bytes = registered.body().len();
                Ok(RegisteredRecoveryGrant {
                    owner: Some(Arc::new(GrantState {
                        key,
                        registered,
                        completed_memory: completed.memory.clone(),
                        stages: Mutex::new(Stages::default()),
                        _claim: claim,
                    })),
                    key,
                    original,
                    startup_bytes,
                })
            }
        }
    }
}

/// Observe the genuine original Root grant while an actual native consumer is running.
pub(crate) fn original_stop(
    core: &Core,
    state: &State,
    grant: Option<&RegisteredRecoveryGrant>,
    now: Moment,
) -> Option<crate::task::Stop> {
    let grant = grant?;
    match grant.state().and_then(|grant| grant.validate_locked(core, state, now)) {
        Ok(_) => None,
        Err(AdmissionError::DeadlineExpired { .. }) => Some(crate::task::Stop::TimedOut),
        Err(_) => Some(crate::task::Stop::Cancelled),
    }
}

#[cfg(test)]
#[path = "recovery/control_tests.rs"]
pub(in crate::coordinator_entry) mod control_tests;
