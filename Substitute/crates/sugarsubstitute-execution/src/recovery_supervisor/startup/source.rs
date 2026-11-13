// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual execution/session/issuer custody outside the fixed receiver claim.
use super::*;
use crate::{
    admission::{Core, State},
    task::ErasedJob,
};
use sugarsubstitute_contracts::execution::{CancelControl, JobPhase, ScopeId, TerminalKind};
pub(crate) struct Source {
    pub(crate) key: RecoveryGrantKey,
    #[cfg(windows)]
    pub(crate) root_desktop: crate::recovery_supervisor::root_desktop::RootDesktopBootstrapLoan,
    /// Portable test builds keep the loan for this source's lifetime without reading it.
    #[cfg(not(windows))]
    _root_desktop: crate::recovery_supervisor::root_desktop::RootDesktopBootstrapLoan,
    pub(crate) creation_taken: std::sync::atomic::AtomicBool,
    pub(crate) enrollment_sequence: std::sync::atomic::AtomicU64,
    pub(crate) enrollment_taken: std::sync::atomic::AtomicBool,
    pub(crate) counter: CounterDeadline,
    pub(crate) until: Moment,
    pub(crate) body: Box<[u8]>,
    pub(crate) root_units_le: Box<[u8]>,
    pub(crate) root_length: usize,
    pub(crate) root_identity: [u8; 24],
    pub(crate) length: usize,
    pub(crate) desktop_identity: Option<sugarsubstitute_contracts::startup::MainIdentity>,
    pub(crate) shared: SharedRef,
    owner: Owner,
    core: Arc<Core>,
    scope: ScopeId,
    issuer: Arc<dyn ErasedJob>,
}
impl Source {
    pub(super) fn execution_id(&self) -> sugarsubstitute_contracts::execution::ExecutionId {
        self.core.id
    }
    /// Construct only through the actual running fixed projection producer.
    pub(super) fn allocate(
        core: Arc<Core>,
        shared: SharedRef,
        owner: Owner,
        context: JobContext,
    ) -> Result<(Self, super::storage::Charge), RecoveryStartupError> {
        if size_of::<Self>()
            + size_of::<super::Assembly>()
            + super::storage::Charge::storage_bytes()
            + 4 * size_of::<usize>()
            > 4096
        {
            return Err(RecoveryStartupError::Binding);
        }
        let claim = context
            .memory()
            .claim(2 * 65536 + 4096)
            .map_err(|e| RecoveryStartupError::Admission(AdmissionError::Memory(e)))?;
        let mut body = Vec::new();
        body.try_reserve_exact(RECOVERY_STARTUP_BYTES).map_err(RecoveryStartupError::Allocation)?;
        body.resize(RECOVERY_STARTUP_BYTES, 0);
        let body = body.into_boxed_slice();
        let mut root = Vec::new();
        root.try_reserve_exact(RECOVERY_ROOT_BYTES).map_err(RecoveryStartupError::Allocation)?;
        root.resize(RECOVERY_ROOT_BYTES, 0);
        let root_units_le = root.into_boxed_slice();
        let now = core
            .clock
            .metadata()
            .map_err(|e| RecoveryStartupError::Admission(AdmissionError::Unexpected(e)))?;
        let (key, _counter, issuer, root_desktop) = {
            let state = lock(&core.state);
            crate::capture_bundle::validate::live(&core, &state, &context, now)
                .map_err(RecoveryStartupError::Admission)?;
            let record = crate::capture_bundle::validate::association(&core, &state, &context)
                .map_err(RecoveryStartupError::Admission)?;
            crate::lifecycle_pipe::current_admission::source_locked(
                &core, &state, &shared, &owner, now,
            )
            .map_err(RecoveryStartupError::Admission)?;
            let mut pipe = lock(&shared.state);
            let peer = pipe.authenticated_peer.ok_or(RecoveryStartupError::Binding)?;
            let key = peer.binding.recovery().ok_or(RecoveryStartupError::Binding)?;
            let deadline = pipe.deadline.ok_or(RecoveryStartupError::Binding)?;
            let counter =
                CounterDeadline::new(deadline.ticks(), deadline.frequency()).map_err(|e| {
                    RecoveryStartupError::Admission(AdmissionError::Unexpected(
                        crate::runtime::error(e),
                    ))
                })?;
            if !shared.child
                || peer.process.is_some()
                || pipe.recovery_startup_taken
                || shared.original_deadline
                    != context.spec().deadline.ok_or(RecoveryStartupError::Binding)?
                || now >= shared.original_deadline
                || !shared.core.ptr_eq(&Arc::downgrade(&core))
                || context.spec().reservation_bytes != 4 * 1024 * 1024
                || context.spec().operation != Operation::NativeLifecycle
                || context.spec().class != JobClass::InfrastructureLifecycle
                || context.spec().replacement.is_some()
            {
                return Err(RecoveryStartupError::Binding);
            }
            let root_desktop =
                pipe.recovery_desktop_bootstrap.take().ok_or(RecoveryStartupError::Binding)?;
            if root_desktop.key() != key {
                pipe.recovery_desktop_bootstrap = Some(root_desktop);
                return Err(RecoveryStartupError::Binding);
            }
            pipe.recovery_startup_taken = true;
            (key, counter, record.job.clone(), root_desktop)
        };
        let charge = super::storage::Charge::new(claim, context.memory().clone());
        Ok((
            Self {
                key,
                creation_taken: std::sync::atomic::AtomicBool::new(false),
                enrollment_sequence: std::sync::atomic::AtomicU64::new(0),
                enrollment_taken: std::sync::atomic::AtomicBool::new(false),
                counter: root_desktop.counter(),
                until: shared.original_deadline.min(root_desktop.until()),
                #[cfg(windows)]
                root_desktop,
                #[cfg(not(windows))]
                _root_desktop: root_desktop,
                body,
                root_units_le,
                root_length: 0,
                root_identity: [0; 24],
                length: 0,
                desktop_identity: None,
                shared,
                owner,
                core,
                scope: context.scope_id(),
                issuer,
            },
            charge,
        ))
    }
    pub(crate) fn check_retirement_context(
        &self,
        context: &JobContext,
    ) -> Result<(), AdmissionError> {
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&self.core.state);
        let record =
            state.jobs.get(&context.job_id()).ok_or(AdmissionError::InvalidCaptureAdmission)?;
        if context.owner_id() != self.core.id
            || record.phase != JobPhase::Running
            || record.scope_id != context.scope_id()
            || record.spec != context.spec()
            || record.memory.as_ref().is_none_or(|memory| !memory.matches_lease(context.memory()))
            || context.spec().operation != Operation::NativeLifecycle
            || context.spec().class != JobClass::InfrastructureLifecycle
            || context.deadline().is_none_or(|until| now >= until)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cutoff = crate::admission_liveness::scope_deadline(&self.core, &state, record)?;
        if let Some(deadline) = cutoff.filter(|until| now >= *until) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        Ok(())
    }
    pub(crate) fn check(&self) -> Result<(), AdmissionError> {
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&self.core.state);
        self.validate_locked(&self.core, &state, self.key.run(), now).map(|_| ())
    }
    /// Revalidate the genuine original peer and successful producer under final Core admission.
    pub(crate) fn validate_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        now: Moment,
    ) -> Result<Moment, AdmissionError> {
        if !Arc::ptr_eq(core, &self.core)
            || run != self.key.run()
            || now >= self.until
            || self.issuer.status().phase != JobPhase::Terminal(TerminalKind::Success)
            || self.issuer.cancellation().is_requested()
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cutoff = crate::coordinator_entry::startup_scope_current(state, self.scope)
            .map_err(|e| AdmissionError::Unexpected(crate::runtime::error(e)))?;
        if let Some(deadline) = cutoff.filter(|until| *until <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        crate::lifecycle_pipe::current_admission::source_locked(
            core,
            state,
            &self.shared,
            &self.owner,
            now,
        )?;
        let pipe = lock(&self.shared.state);
        if !self.shared.child
            || pipe.authenticated_peer.is_none_or(|peer| {
                peer.binding.recovery() != Some(self.key) || peer.process.is_some()
            })
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(cutoff.map_or(self.until, |deadline| deadline.min(self.until)))
    }
}
