// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the surviving Root's authenticated recovery control without duplicating ParentLifecycle.
#[path = "session/failures.rs"]
mod failures;
#[path = "session/framing.rs"]
mod framing;
#[cfg(test)]
#[path = "session/funding_tests.rs"]
mod funding_tests;
#[path = "session/observing.rs"]
mod observing;
#[path = "session/retirement.rs"]
mod retirement;
#[cfg(test)]
#[path = "session/tests.rs"]
mod tests;
#[path = "session/types.rs"]
mod types;
#[path = "session/workers.rs"]
mod workers;
use framing::SendPlan;
use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Mutex, TryLockError},
};
use sugarsubstitute_contracts::{
    execution::*,
    incident::DurableAcknowledgement,
    recovery_supervisor::{RecoveryProcessIdentity, wire::*},
    startup::MainIdentity,
};
use sugarsubstitute_crash::CaptureRootSource;
use sugarsubstitute_execution::{
    ExecutionClient, RegisteredRecoveryGrant,
    recovery_supervisor::*,
    windows::{NativeObservationError, ProcessExit, ProcessWatchPermit},
    windows_lifecycle_pipe::LifecycleDesktopEnrollment,
};
use sugarsubstitute_supervision::{
    LaunchTicket, RecoveryDesktopWatch, RecoveryEnrolledDesktop, RecoveryHandlerWatch,
    RegistryError, WindowsProcessRegistry,
};
use types::*;
pub(super) use types::{
    RootRecoverySessionInput, RootRecoverySessionRejection, SessionCause, SessionCode,
    SessionProgress,
};
/// One admitted Root control and its original native/watch/source ownership.
/// The original projection lease lives outside the Box through actual allocation destruction.
/// Unsettled observing-thread Drop retains its existing durable Box without native calls/allocation.
pub(super) struct RootRecoverySession {
    state: Option<Box<State>>,
    _outside: Option<MemoryLease>,
}
/// Preserve complete retired observations and original causes until the caller finishes consuming them.
/// This never represents registry family settlement or application capture coverage.
pub(super) struct RetiredRootRecoverySession {
    _state: Box<State>,
    _outside: Option<MemoryLease>,
}
impl RootRecoverySession {
    /// Exact bounded Root metadata envelope, separate from existing charged pipe/startup buffers.
    pub(super) const fn required_storage_bytes() -> u64 {
        (size_of::<State>()
            + size_of::<RootRecoverySessionInput>()
            + size_of::<Mutex<Work>>()
            + size_of::<RecoveryPipeObservations>()
            + size_of::<Funding>()
            + size_of::<[usize; 8]>()
            + 4096) as u64
    }
    /// Finite worker closure and fixed unit-result envelope, in bytes.
    pub(super) const fn worker_reservation_bytes() -> u64 {
        4096
    }
    /// Consume only actual pre-admitted owners; constructor rejection returns all originals.
    #[expect(
        clippy::result_large_err,
        reason = "Return every original unconsumed native/session input on rejection without allocating."
    )]
    pub(super) fn new(
        input: RootRecoverySessionInput,
    ) -> Result<Self, RootRecoverySessionRejection> {
        let checked = (|| {
            if input.memory.bytes() < Self::required_storage_bytes() || input.memory.bytes() > 65536
            {
                return Err(SessionCause::Protocol(SessionCode::Binding));
            }
            input.grant.check().map_err(SessionCause::Admission)?;
            let execution = input.grant.execution_id().map_err(SessionCause::Admission)?;
            let root_identity =
                input.root.root_identity().ok_or(SessionCause::Protocol(SessionCode::Binding))?;
            let _same_actual_source = input
                .root
                .recovery_finished_source(&input.grant)
                .map_err(SessionCause::Incident)?;
            if input.memory.bytes() < Self::required_storage_bytes()
                || input.memory.bytes() > 65536
                || input.identity.run != input.grant.key().run()
                || input.carrier.binding()
                    != sugarsubstitute_supervision::FamilyBinding::Desktop(input.identity.run)
                || input.pipe.control().prepared_key() != Some(input.grant.key())
                || input.desktop_watch.key() != input.grant.key()
                || input.handler_permit.owner_id() != execution
                || input.handler_permit.deadline().is_some()
                || input.enrollment.is_none()
                || input.root.root_identity().is_none()
                || input.ack.incident() != input.grant.key().report().incident()
                || input
                    .grant
                    .key()
                    .report()
                    .revision()
                    .next()
                    .map_err(|_| SessionCause::Protocol(SessionCode::Binding))?
                    != input.ack.revision()
            {
                return Err(SessionCause::Protocol(SessionCode::Binding));
            }
            Ok((execution, root_identity))
        })();
        let (execution, root_identity) = match checked {
            Ok(actual) => actual,
            Err(cause) => return Err(RootRecoverySessionRejection { cause, input }),
        };
        let pipe_scope = input.pipe.scope_id();
        let pipe_spec = input.pipe.spec();
        let operation = input.pipe.control().last_operation();
        let funding = Arc::new(Funding { _memory: input.memory });
        let work = Arc::new(Mutex::new(Work {
            desktop_watch: Some(input.desktop_watch),
            desktop: None,
            enrollment: input.enrollment,
            desktop_prepared: false,
            handler_permit: Some(input.handler_permit),
            handler: None,
            cancelled_ui: None,
            cancelled_handler: None,
            ui_terminal: None,
            handler_terminal: None,
            failure: [None, None],
            _funding: funding.clone(),
        }));
        Ok(Self {
            state: Some(Box::new(State {
                execution,
                pipe: input.pipe,
                pipe_scope,
                pipe_spec,
                grant: input.grant,
                identity: input.identity,
                carrier: input.carrier,
                _root: input.root,
                root_identity,
                ack: input.ack,
                peer: None,
                connected: false,
                hello: false,
                prepared: false,
                plan: SendPlan::default(),
                operation,
                read: None,
                admission: None,
                sequence: 0,
                startup_accepted: false,
                handler_advisory: None,
                desktop_advisory: None,
                desktop_exit_advisory: None,
                enrolled_reply: false,
                subordinate_retired: framing::RetirementReceipt::default(),
                closing: None,
                pipe_terminal: None,
                pipe_verified: false,
                pipe_retirement: None,
                job: None,
                job_terminals: [None, None],
                failures: failures::Failures::default(),
                exit_mismatch: false,
                watch_reported: [false; 2],
                worker_reported: false,
                last_now: None,
                work,
                _funding: funding,
            })),
            _outside: Some(input.outside),
        })
    }
    /// Drain actual bounded transport/worker facts; root_path borrows original funded configuration.
    /// Root's common pump supplies its actual execution-clock sample and global completion drain.
    pub(super) fn poll(&mut self, root_path: &Path, now: Moment) -> SessionProgress {
        let Some(state) = self.state.as_mut() else {
            return SessionProgress::default();
        };
        if state.last_now.is_some_and(|last| now < last) {
            state.record(SessionCause::Protocol(SessionCode::Clock));
        }
        state.last_now = Some(now);
        if state.closing.is_none()
            && !state.enrolled_reply
            && state.failures.first.is_none()
            && now >= state.grant.original_budget().until()
        {
            state.record(SessionCause::Protocol(SessionCode::Deadline));
        }
        state.poll_worker();
        let observations = state.pipe.drain(NonZeroUsize::new(16).unwrap_or(NonZeroUsize::MIN));
        for observation in observations {
            if let Err(cause) = state.observe(observation) {
                state.record(cause);
            }
        }
        state.observe_pipe_terminal();
        state.observe_watches();
        state.progress_retirement(now);
        if state.failures.first.is_none() || state.closing.is_some() {
            if let Err(cause) = state.send_next(root_path) {
                state.record(cause);
            }
            if let Err(cause) = state.read_next() {
                state.record(cause);
            }
        }
        state.snapshot()
    }
    /// Borrow genuine independently enrolled UI authority during one nonblocking caller turn.
    /// # Errors
    /// Busy native ownership or absent physical enrollment cannot provide a proof.
    pub(super) fn with_desktop<T>(
        &self,
        read: impl FnOnce(&RecoveryEnrolledDesktop) -> T,
    ) -> Result<T, SessionCode> {
        read_desktop(self.state.as_ref().ok_or(SessionCode::Closed)?, read)
    }
    /// Borrow actual original first cause; terminal owners retain panic and native failure metadata.
    pub(super) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.state.as_ref()?.failures.first.as_ref().map(|cause| cause as &dyn std::error::Error)
    }
    /// Borrow the fixed secondary cause and saturating number of later omitted causes.
    pub(super) fn secondary_failure(&self) -> (Option<&(dyn std::error::Error + 'static)>, u64) {
        self.state.as_ref().map_or((None, 0), |state| {
            (
                state.failures.secondary.as_ref().map(|cause| cause as &dyn std::error::Error),
                state.failures.omitted,
            )
        })
    }
    /// Transfer only actually retired control/watch/finite-worker custody, keeping causes/backing.
    pub(super) fn take_retired(&mut self) -> Result<RetiredRootRecoverySession, SessionCode> {
        if !self.state.as_ref().is_some_and(|state| state.settled()) {
            return Err(SessionCode::Pending);
        }
        Ok(RetiredRootRecoverySession {
            _state: self.state.take().ok_or(SessionCode::Closed)?,
            _outside: self._outside.take(),
        })
    }
}
impl Drop for RootRecoverySession {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            if state.settled() {
                drop(state);
            } else {
                std::mem::forget(state);
            }
        }
    }
}
impl State {
    fn record(&mut self, cause: SessionCause) {
        self.failures.record(cause);
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    fn next_operation(&mut self) -> Result<LifecyclePipeOperationId, SessionCause> {
        let next =
            self.operation.checked_add(1).ok_or(SessionCause::Protocol(SessionCode::Exhausted))?;
        let operation = LifecyclePipeOperationId::new(next).map_err(SessionCause::Pipe)?;
        self.operation = next;
        Ok(operation)
    }
    fn snapshot(&self) -> SessionProgress {
        let (desktop, handler, handler_exit) = match self.work.try_lock() {
            Ok(work) => (
                work.desktop.as_ref().map(|desktop| desktop.actual()),
                work.handler.as_ref().and_then(|handler| handler.actual()),
                work.handler.as_ref().and_then(|handler| handler.actual_exit()),
            ),
            Err(_) => (None, None, None),
        };
        SessionProgress {
            handler_needed: self.handler_advisory.is_some()
                && handler.is_none()
                && self.job.is_none()
                && self.failures.first.is_none()
                && self.closing.is_none(),
            desktop_needed: self.desktop_advisory.is_some()
                && handler.is_some()
                && handler_exit.is_none()
                && desktop.is_none()
                && self.job.is_none()
                && self.failures.first.is_none()
                && self.closing.is_none(),
            handler_lost: handler_exit.is_some() && self.closing.is_none(),
            owner_failed: self.owner_failed(),
            failed: self.failures.first.is_some(),
            retired: self.settled(),
        }
    }
}
const _: () = assert!(RootRecoverySession::required_storage_bytes() <= 65536);

impl RetiredRootRecoverySession {
    /// Retain actual UI authority while Root consumes its once-only lifecycle exit after transport close.
    /// This grants no new binding or forwarding admission.
    pub(super) fn with_desktop<T>(
        &self,
        read: impl FnOnce(&RecoveryEnrolledDesktop) -> T,
    ) -> Result<T, SessionCode> {
        read_desktop(&self._state, read)
    }
    /// Borrow the actual first retained native/protocol cause after checked observation retirement.
    pub(super) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self._state.failures.first.as_ref().map(|cause| cause as &dyn std::error::Error)
    }
    /// Borrow the bounded secondary cause and saturating omission count without new allocations.
    pub(super) fn secondary_failure(&self) -> (Option<&(dyn std::error::Error + 'static)>, u64) {
        (
            self._state.failures.secondary.as_ref().map(|cause| cause as &dyn std::error::Error),
            self._state.failures.omitted,
        )
    }
}
fn read_desktop<T>(
    state: &State,
    read: impl FnOnce(&RecoveryEnrolledDesktop) -> T,
) -> Result<T, SessionCode> {
    let work = state.work.try_lock().map_err(|_| SessionCode::Pending)?;
    let desktop = work.desktop.as_ref().ok_or(SessionCode::Pending)?;
    Ok(read(desktop))
}
