// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow actual completed registered restart custody without issuing another generation.
use super::*;
use crate::ExecutionClient;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobCompletion, JobPhase, JobSpec, MemoryLease, Operation,
    TerminalKind,
};

/// Borrow the same issued restart snapshot, original archive and actual completed projection.
/// Construction requires the actual private issuer's successful terminal and original memory;
/// copied completion metadata alone cannot authorize this witness. Borrowing is idempotent:
/// the consumer owns once-only adoption and must preserve this custody through Pending.
/// This witness grants no native effects, source retirement, durable acknowledgement or
/// successful frontend qualification. The Host separately proves actual ingress installation.
/// It allocates nothing and keeps the snapshot and whole completion immutably borrowed.
pub struct RegisteredRestartAdmission<'a> {
    registered: &'a RegisteredSnapshot,
    source: &'a RegisteredStartupSource,
    completed_memory: &'a MemoryLease,
    until: Moment,
}
impl RegisteredRestartAdmission<'_> {
    /// Return the exact previous generation stamped by the genuine original issuer.
    pub const fn previous(&self) -> RunKey {
        self.registered.previous
    }
    /// Return the actual issued next generation, with unchanged owner and instance.
    pub const fn run(&self) -> RunKey {
        self.registered.run
    }
    /// Borrow the identical original authenticated startup archive and environment.
    pub const fn source(&self) -> &RegisteredStartupSource {
        self.source
    }
    /// Return the original central projection job identity.
    pub const fn job_id(&self) -> JobId {
        self.registered.job
    }
    /// Return the actual scope stamped by the original projection issuer.
    pub const fn scope_id(&self) -> ScopeId {
        self.registered.scope
    }
    /// Return the immutable specification of the actual retained issuing job.
    pub fn spec(&self) -> JobSpec {
        self.registered._issuer.spec()
    }
    /// Return the original once-issued 30-second restart budget, without sampling a clock.
    pub const fn original_budget(&self) -> EntrySetupBudget {
        self.registered.original
    }
    /// Return the fixed original cutoff in monotonic nanoseconds, narrowed at this borrow.
    /// Borrowing again never decodes a counter or starts another duration.
    pub const fn effective_until(&self) -> Moment {
        self.until
    }
    /// Recheck the same issuer, source, current coordinator, scopes and original cutoff.
    /// Samples only the genuine original Core clock before taking Core-before-session locks.
    /// # Errors
    /// Rejects shutdown, cancellation, failed clock, stale generation, stopped/closed owner,
    /// changed original memory or issuer terminal, and expiry or widening of the fixed cutoff.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let source = self.source.state().map_err(admission::transport)?;
        let now = source.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&source.core.state);
        let until = self.validate_locked(&source.core, &state, now)?;
        if until > self.until {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(())
    }
    fn validate_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        now: Moment,
    ) -> Result<Moment, AdmissionError> {
        let original = self.registered;
        let spec = original._issuer.spec();
        let actual = original._issuer.status();
        if !self.source.same_source(&original.source)
            || original.previous.owner != original.run.owner
            || original.previous.instance != original.run.instance
            || original.previous.generation.next().ok() != Some(original.run.generation)
            || spec.id != original.job
            || spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::InfrastructureLifecycle
            || spec.reservation_bytes != PROJECTION_BYTES
            || spec.deadline != Some(original.original.until())
            || spec.replacement.is_some()
            || actual.phase != JobPhase::Terminal(TerminalKind::Success)
            || actual.cancellation_requested
            || !original._memory.same_reservation(self.completed_memory)
            || !Arc::ptr_eq(&original.cancel, &original._issuer.cancellation())
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        original
            .validate_current_locked(core, state, original.run, now)
            .map_err(admission::transport)
    }
}
impl ExecutionClient {
    /// Borrow a registered restart's original completed projection after actual installation.
    /// Generic result/error values are never invoked or projected: only the passive Success
    /// kind and completion envelope are inspected. The constructor-private retained issuer
    /// independently proves its actual successful terminal after Core record acknowledgement.
    /// Equal IDs/specification with another lease or source cannot authorize this borrow.
    /// Repeated same-source borrows preserve the same original budget and perform no effects.
    /// # Errors
    /// Preserves inputs on foreign/stale/unregistered source, wrong completion/role/memory,
    /// unsuccessful actual issuer, cancellation, original expiry, closed owner or clock failure.
    pub fn borrow_registered_restart_admission<'a, T, E>(
        &self,
        snapshot: &'a PreparedStartupSnapshot,
        source: &'a RegisteredStartupSource,
        completed: &'a JobCompletion<T, E>,
    ) -> Result<RegisteredRestartAdmission<'a>, AdmissionError> {
        let registered =
            snapshot.registered.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
        let original = source.state().map_err(admission::transport)?;
        if snapshot.launch.is_some()
            || !Arc::ptr_eq(&original.core, &self.core)
            || !source.same_source(&registered.source)
            || completed.outcome.kind() != TerminalKind::Success
            || completed.scope_id != registered.scope
            || completed.spec != registered._issuer.spec()
            || completed.native_handles.is_some()
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&self.core.state);
        let mut witness = RegisteredRestartAdmission {
            registered,
            source,
            completed_memory: &completed.memory,
            until: registered.original.until(),
        };
        witness.until = witness.validate_locked(&self.core, &state, now)?;
        Ok(witness)
    }
}
