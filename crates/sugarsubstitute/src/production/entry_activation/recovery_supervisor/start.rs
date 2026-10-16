// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain every genuine recovery launch input before durable acknowledgement releases creation.
#[path = "start/commit.rs"]
mod commit;
#[path = "start/lifetime.rs"]
mod lifetime;
#[path = "start/preparation.rs"]
mod preparation;
#[cfg(test)]
#[path = "start/tests.rs"]
mod tests;
use super::session::{
    RetiredRootRecoverySession, RootRecoverySession, RootRecoverySessionInput, SessionCode,
};
use crate::production::{
    entry_activation::{
        launch::DesktopStart,
        projection::{ProjectedEntry, ProjectionFailure},
    },
    entry_runner::EntryResources,
    parent_lifecycle::{ParentLifecycle, ParentResources},
};
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::{
    execution::*, incident::DurableAcknowledgement, lifecycle::*, startup::MainIdentity,
};
use sugarsubstitute_crash::CaptureRootSource;
use sugarsubstitute_execution::{
    RegisteredRecoveryGrant,
    recovery_supervisor::*,
    windows::{NativeObservationError, ProcessExit, ProcessWatchPermit},
    windows_recovery_supervisor::{
        RecoveryBootstrap, RecoveryCarrierBootstrap, RecoveryEnrollment,
    },
};
use sugarsubstitute_supervision::{
    BackendCustodyParent, RecoveryDesktopWatch, RecoveryEnrolledDesktop, WindowsLaunchInput,
    profiles::WindowsProfileLease,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Preserve the exact unconsumed projection and registered grant on constructor rejection.
pub(in crate::production::entry_activation) struct RecoveryStartRejection {
    cause: Box<dyn Error>,
    projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
    grant: RegisteredRecoveryGrant,
}
impl RecoveryStartRejection {
    /// Return all original owners; no native admission has occurred.
    pub(in crate::production::entry_activation) fn into_parts(
        self,
    ) -> (Box<dyn Error>, JobCompletion<ProjectedEntry, ProjectionFailure>, RegisteredRecoveryGrant)
    {
        (self.cause, self.projection, self.grant)
    }
}
/// Recovery mode owns the fresh carrier/control without borrowing the old Root capture pin.
/// Common profile, projection, output and direct UI transport remain with DesktopStart.
pub(in crate::production::entry_activation) struct RecoveryStart {
    state: Option<Box<State>>,
    _projection: MemoryLease,
}
/// Keep actual native reservations, full terminal results and their outside allocation funding.
struct State {
    execution: ExecutionId,
    grant: RegisteredRecoveryGrant,
    pipe: Option<RecoveryPipeSession>,
    bootstrap: Option<RecoveryBootstrap>,
    enrollment: Option<RecoveryEnrollment>,
    paired: Option<RecoveryCarrierBootstrap>,
    input: Option<WindowsLaunchInput>,
    desktop_watch: Option<RecoveryDesktopWatch>,
    handler_permit: Option<ProcessWatchPermit>,
    active: Option<RootRecoverySession>,
    retired: Option<RetiredRootRecoverySession>,
    _rejected_root: Option<CaptureRootSource>,
    cause: Option<Box<dyn Error>>,
    cancelled_ui: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    cancelled_handler: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    ui_terminal: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    handler_terminal: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    pipe_terminal: Option<JobCompletion<RecoveryPipeReport, LifecyclePipeFailure>>,
    pipe_retirement: Option<RecoveryPipeRetirement>,
    closing: Option<Closing>,
    committed: Option<DurableAcknowledgement>,
    creation_begun: bool,
    exit_delivered: bool,
    precommit_closed: bool,
    owner_failed: bool,
    session_failed: bool,
    handler_lost: bool,
    failure_reported: bool,
    session_memory: Option<MemoryClaim>,
    _memory: MemoryClaim,
}
/// Freeze the first caller-owned cleanup transaction before any fallible counter conversion.
struct Closing {
    budget: ShutdownBudget,
    counter: Option<CounterDeadline>,
}
impl Closing {
    /// Preserve the first transaction even when later native clock conversion rejects.
    fn latch(current: &mut Option<Self>, budget: ShutdownBudget) -> Result<()> {
        match current {
            Some(original) if original.budget != budget => {
                Err("Original recovery cleanup budget changed".into())
            }
            Some(_) => Ok(()),
            None => {
                *current = Some(Self { budget, counter: None });
                Ok(())
            }
        }
    }
}
impl RecoveryStart {
    /// Exact distinct allocations charged before constructing recovery mode or session metadata.
    /// Account the durable state, largest retained boxed setup error and Box allocation headers.
    const fn start_storage_bytes() -> u64 {
        (size_of::<State>() + size_of::<super::session::SessionCause>() + 2 * size_of::<usize>())
            as u64
    }
    /// Bound both distinct actual allocation owners within the existing startup partition.
    const fn required_storage_bytes() -> u64 {
        Self::start_storage_bytes() + RootRecoverySession::required_storage_bytes()
    }
    /// Claim both actual metadata allocations before constructing either owner.
    fn reserve_memory(memory: &MemoryLease) -> Result<(MemoryClaim, MemoryClaim)> {
        if Self::required_storage_bytes() > 61_440 {
            return Err("Recovery metadata exceeds original startup partition".into());
        }
        let start = memory.claim(Self::start_storage_bytes())?;
        let session = memory.claim(RootRecoverySession::required_storage_bytes())?;
        Ok((start, session))
    }
    /// Construct only inert charged state; every native admission follows retained ownership.
    fn new(
        grant: RegisteredRecoveryGrant,
        memory: (MemoryClaim, MemoryClaim),
        projection: MemoryLease,
        execution: ExecutionId,
    ) -> Self {
        Self {
            state: Some(Box::new(State {
                execution,
                grant,
                pipe: None,
                bootstrap: None,
                enrollment: None,
                paired: None,
                input: None,
                desktop_watch: None,
                handler_permit: None,
                active: None,
                retired: None,
                _rejected_root: None,
                cause: None,
                cancelled_ui: None,
                cancelled_handler: None,
                ui_terminal: None,
                handler_terminal: None,
                pipe_terminal: None,
                pipe_retirement: None,
                closing: None,
                committed: None,
                creation_begun: false,
                exit_delivered: false,
                precommit_closed: false,
                owner_failed: false,
                session_failed: false,
                handler_lost: false,
                failure_reported: false,
                session_memory: Some(memory.1),
                _memory: memory.0,
            })),
            _projection: projection,
        }
    }
    /// Borrow retained mode custody without projecting native authority.
    fn state(&self) -> Result<&State> {
        self.state.as_deref().ok_or_else(|| "Recovery mode ownership unavailable".into())
    }
    /// Mutably borrow the one actual mode owner on Root.
    fn state_mut(&mut self) -> Result<&mut State> {
        self.state.as_deref_mut().ok_or_else(|| "Recovery mode ownership unavailable".into())
    }
    /// Borrow only the original registered grant; passive facts cannot rebuild it.
    pub(in crate::production::entry_activation) fn grant(
        &self,
    ) -> Option<&RegisteredRecoveryGrant> {
        Some(&self.state.as_ref()?.grant)
    }
    /// Report checked local control/watch retirement independently of outer family/profile cleanup.
    pub(in crate::production::entry_activation) fn settled(&self) -> bool {
        self.state.as_ref().is_none_or(|state| state.settled())
    }
}
impl State {
    /// Require exact original watcher terminals and checked physical control retirement.
    fn settled(&self) -> bool {
        self.retired.is_some()
            || (self.active.is_none()
                && self.precommit_closed
                && self.input.is_none()
                && self.paired.is_none()
                && self.bootstrap.is_none()
                && self.enrollment.is_none()
                && self.cancelled_ui.as_ref().is_none_or(|job| {
                    self.ui_terminal.as_ref().is_some_and(|done| {
                        done.scope_id == job.scope_id() && done.spec == job.spec()
                    })
                })
                && self.cancelled_handler.as_ref().is_none_or(|job| {
                    self.handler_terminal.as_ref().is_some_and(|done| {
                        done.scope_id == job.scope_id() && done.spec == job.spec()
                    })
                })
                && self.desktop_watch.as_ref().is_none_or(|_| self.cancelled_ui.is_some())
                && self.handler_permit.is_none()
                && self.pipe.as_ref().is_none_or(|pipe| {
                    self.pipe_terminal.as_ref().zip(self.pipe_retirement.as_ref()).is_some_and(
                        |(done, proof)| {
                            done.scope_id == pipe.scope_id()
                                && done.spec == pipe.spec()
                                && proof.matches(pipe.control())
                                && proof.key() == self.grant.key()
                                && proof.scope_id() == done.scope_id
                                && proof.spec() == done.spec
                                && proof.execution_id() == self.execution
                        },
                    )
                }))
    }
}
impl Drop for RecoveryStart {
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
impl DesktopStart {
    /// Validate and retain the exact successor projection/grant before any native setup.
    #[expect(
        clippy::result_large_err,
        reason = "Return complete original projection and grant custody"
    )]
    pub(in crate::production::entry_activation) fn new_recovery(
        identity: MainIdentity,
        qualification: crate::production::qualification::QualificationIntent,
        connection: ConnectionId,
        projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
        grant: RegisteredRecoveryGrant,
        resources: &mut EntryResources<'_>,
    ) -> std::result::Result<Self, RecoveryStartRejection> {
        let prepared = (|| -> Result<_> {
            grant.check()?;
            let execution = grant.execution_id()?;
            if execution != resources.native.owner_id() || execution != resources.finite.owner_id()
            {
                return Err("Actual recovery execution source mismatch".into());
            }
            let value = match &projection.outcome {
                JobOutcome::Success(value) => value,
                _ => return Err("Actual recovery projection did not succeed".into()),
            };
            if resources.authority.key() != Some(identity.run.owner)
                || !resources.authority.serving()
                || identity.run != grant.key().run()
                || value.until != grant.original_budget().until()
                || !grant.same_startup_source(&value.registered)
                || value.snapshot.is_some()
                || resources.client.now()? >= value.until
            {
                return Err("Actual recovery projection/grant authority mismatch".into());
            }
            let memory = RecoveryStart::reserve_memory(&projection.memory)?;
            let registry =
                resources.authority.registry().ok_or("Actual recovery registry unavailable")?;
            let profile =
                WindowsProfileLease::new(identity.run, value.requested_profile.clone(), registry)?;
            Ok((profile, memory, execution))
        })();
        match prepared {
            Ok((profile, memory, execution)) => {
                let recovery =
                    RecoveryStart::new(grant, memory, projection.memory.clone(), execution);
                Ok(Self::initial(
                    identity,
                    qualification,
                    connection,
                    projection,
                    profile,
                    None,
                    Some(recovery),
                ))
            }
            Err(cause) => Err(RecoveryStartRejection { cause, projection, grant }),
        }
    }
    /// Borrow the actual grant only for recovery mode.
    pub(in crate::production::entry_activation) fn grant(
        &self,
    ) -> Option<&RegisteredRecoveryGrant> {
        self.recovery.as_ref()?.grant()
    }
    /// Observe that the exact acknowledged held creation began once.
    pub(in crate::production::entry_activation) fn recovery_committed(&self) -> bool {
        self.recovery
            .as_ref()
            .and_then(|mode| mode.state.as_ref())
            .is_some_and(|state| state.creation_begun)
    }
    /// Borrow original constructor/session failure while retaining its complete native owners.
    pub(in crate::production::entry_activation) fn recovery_failure(
        &self,
    ) -> Option<&(dyn Error + 'static)> {
        let state = self.recovery.as_ref()?.state.as_ref()?;
        state
            .cause
            .as_deref()
            .or_else(|| state.retired.as_ref().and_then(RetiredRootRecoverySession::failure))
            .or_else(|| state.active.as_ref().and_then(RootRecoverySession::failure))
            .or_else(|| state.ui_terminal.as_ref().and_then(watch_failure))
            .or_else(|| state.handler_terminal.as_ref().and_then(watch_failure))
            .or_else(|| {
                state.pipe_terminal.as_ref().and_then(|done| match &done.outcome {
                    JobOutcome::Failure(cause) => Some(cause as &dyn Error),
                    _ => None,
                })
            })
    }
    /// Require actual recovery control/watch settlement before releasing the containing record.
    pub(in crate::production::entry_activation) fn recovery_settled(&self) -> bool {
        self.recovery.as_ref().is_none_or(RecoveryStart::settled)
    }
}
const _: () = assert!(RecoveryStart::required_storage_bytes() <= 61_440);

/// Borrow a retained original native cause without manufacturing a fresh failure owner.
fn watch_failure(
    done: &JobCompletion<ProcessExit, NativeObservationError>,
) -> Option<&(dyn Error + 'static)> {
    match &done.outcome {
        JobOutcome::Failure(cause) => Some(cause),
        _ => None,
    }
}
