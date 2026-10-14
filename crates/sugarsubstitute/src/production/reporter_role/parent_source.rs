// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Loan the genuine Reporter parent outside a fixed original-funded relay cell.
#[path = "parent_source/controller.rs"]
mod controller;
#[path = "parent_source/recycling.rs"]
mod recycling;
pub(in crate::production) use recycling::{DisposedReporter, RecycledReporter};
#[path = "parent_source/failure.rs"]
mod failure;
#[path = "parent_source/lifetime.rs"]
mod lifetime;
use failure::OriginalCause;
pub(in crate::production) use failure::{OwnedReporterFailure, ReporterParentFailure};
#[path = "parent_source/stream.rs"]
mod stream;
#[cfg(test)]
#[path = "parent_source/tests.rs"]
mod tests;
#[path = "parent_source/time.rs"]
mod time;
#[path = "parent_source/worker.rs"]
mod worker;
use super::parent::{ReporterFunding, ReporterLaunch, ReporterParent};
use crate::production::diagnostic_helper::reporting::PreparedReportView;
pub(in crate::production) use controller::ReporterRelay;
use std::sync::{Arc, Mutex, TryLockError};
use sugarsubstitute_contracts::{execution::*, incident::*, lifecycle::ProcessKey};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_execution::{
    ExecutionClient, windows::NativeObservationError, windows_diagnostic_pipe::*,
};
use sugarsubstitute_supervision::{WindowsLaunchInput, WindowsProcessRegistry};
pub(in crate::production) use time::ReporterRequestContext;

/// Closed relay failures retain original native/wire causes with their original buffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ReporterSourceIssue {
    Busy,
    Identity,
    Admission,
    Parent,
    Wire,
    Cancelled,
    Deadline,
    Clock,
}
impl std::fmt::Display for ReporterSourceIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reporter relay rejected: {self:?}")
    }
}
impl std::error::Error for ReporterSourceIssue {}

/// Preserve the complete genuine received frame and the original nonclone router token.
pub(in crate::production) enum ReporterOutput {
    Received {
        received: ReceivedDiagnosticRequest,
        admission: DiagnosticAdmission,
        observation: ReporterObservation,
        request_context: ReporterRequestContext,
        observer: DiagnosticReply,
    },
    Completed {
        received: ReceivedDiagnosticRequest,
        issued: IssuedDiagnosticRequest,
        accepted: bool,
    },
    ControlWritten(LifecyclePipeOperationId),
}
/// Strictly decoded inert use-case facts alongside their unchanged authenticated frame owner.
pub(in crate::production) enum ReporterObservation {
    Presented { binding: ReportBinding, replacement: bool },
    Action(ReportCommand),
    Copy { binding: ReportBinding, result: DiagnosticActionResult },
    Selected { binding: ReportBinding, selection: ExportSelection },
    SelectionCancelled(ReportBinding),
    NativeRetired(ReportBinding),
    Failed { binding: ReportBinding, failure: ReportFailure },
}
/// Caller-owned original authentication remains intact through actual router completion.
pub(in crate::production) struct ReporterCompletion {
    pub(in crate::production) received: ReceivedDiagnosticRequest,
    pub(in crate::production) issued: IssuedDiagnosticRequest,
    pub(in crate::production) reply: DiagnosticReply,
}
/// Only closed host decisions can be emitted; selected path bytes never escape into this API.
pub(in crate::production) enum ReporterControl {
    Copy(DiagnosticActionResult),
    Export(DiagnosticActionResult),
    Close(DurableAcknowledgement),
}
/// Fixed inert wire correlation supplied by the coordinator, never a renewed stream deadline.
pub(in crate::production) struct ReporterControlInput {
    pub(in crate::production) key: DiagnosticKey,
    pub(in crate::production) request: DiagnosticRequestId,
    pub(in crate::production) context: DiagnosticContext,
    pub(in crate::production) session: ReportSessionId,
    pub(in crate::production) until: Moment,
    pub(in crate::production) command: ReporterControl,
}
struct State {
    parent: Option<Box<ReporterParent>>,
    funding: Option<ReporterFunding>,
    views: [Option<stream::Stream>; 2],
    completion: Option<ReporterCompletion>,
    control: Option<ReporterControlInput>,
    control_write: Option<LifecyclePipeOperationId>,
    control_written: bool,
    output: Option<ReporterOutput>,
    first: Option<ReporterSourceIssue>,
    wire: Option<DiagnosticWireError>,
    rejected: Option<(ReceivedDiagnosticRequest, DiagnosticAdmission)>,
    read_after_complete: bool,
    core_scope: Option<ScopeId>,
    last_observed: Option<Moment>,
    time_cause: Option<time::ReporterTimeCause>,
    closing: bool,
    failure_transferred: bool,
    registry_cause: Option<sugarsubstitute_supervision::RegistryAdmissionError>,
}
struct Shared {
    state: Mutex<Option<Box<State>>>,
    owner: ExecutionId,
}
/// Each alias retains the original controller lease outside the final Arc header allocation.
pub(in crate::production) struct ReporterSource {
    shared: Arc<Shared>,
    outside: MemoryLease,
}
const fn parent_or_recycling_storage_bytes() -> usize {
    // One actual Parent allocation persists through detach and accepted disposal.
    // Count both receipt transfer handles, original funding return and temporary lease.
    size_of::<ReporterParent>()
        + 2 * size_of::<RecycledReporter>()
        + size_of::<ReporterFunding>()
        + size_of::<MemoryLease>()
}
const _: () = assert!(
    size_of::<State>()
        + size_of::<Shared>()
        + parent_or_recycling_storage_bytes()
        + size_of::<ReporterRelay>()
        + size_of::<[usize; 8]>()
        < super::parent::REPORTER_METADATA_BYTES as usize
);
impl ReporterSource {
    /// Fixed owning metadata size, including both Arc headers and the genuine parent cell.
    pub(in crate::production) const fn storage_bytes() -> usize {
        size_of::<State>()
            + size_of::<Shared>()
            + parent_or_recycling_storage_bytes()
            + size_of::<ReporterRelay>()
            + size_of::<[usize; 8]>()
    }
    /// Adopt exactly the already admitted16KiB metadata; missing input is unchanged.
    pub(in crate::production) fn new(
        funding: &mut Option<ReporterFunding>,
    ) -> Result<Self, ReporterSourceIssue> {
        let original = funding.as_ref().ok_or(ReporterSourceIssue::Busy)?;
        let outside = original.original_memory().clone();
        let owner = original.owner_id();
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(Some(Box::new(State {
                    parent: None,
                    funding: funding.take(),
                    views: [None, None],
                    completion: None,
                    control: None,
                    control_write: None,
                    control_written: false,
                    output: None,
                    first: None,
                    wire: None,
                    rejected: None,
                    read_after_complete: false,
                    core_scope: None,
                    last_observed: None,
                    time_cause: None,
                    closing: false,
                    failure_transferred: false,
                    registry_cause: None,
                }))),
                owner,
            }),
            outside,
        })
    }
    /// Retain the same admitted allocation; this creates no grant or worker.
    pub(in crate::production) fn alias(&self) -> Self {
        Self { shared: self.shared.clone(), outside: self.outside.clone() }
    }
    fn loan(&self) -> Result<Loan<'_>, ReporterSourceIssue> {
        let mut slot = match self.shared.state.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::Poisoned(cause)) => cause.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(ReporterSourceIssue::Busy),
        };
        let state = slot.take().ok_or(ReporterSourceIssue::Busy)?;
        drop(slot);
        Ok(Loan { source: self, state: Some(state) })
    }
    /// Transfer only the genuine separately funded pipe session into the same retained parent.
    pub(in crate::production) fn install_owner(
        &self,
        session: &mut Option<DiagnosticPipeSession>,
        key: DiagnosticKey,
    ) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if state.parent.is_some() || state.closing {
            return Err(ReporterSourceIssue::Busy);
        }
        let mut parent = None;
        ReporterParent::install_owner(&mut parent, &mut state.funding, session, key)
            .map_err(|_| ReporterSourceIssue::Parent)?;
        state.parent = parent.map(Box::new);
        Ok(())
    }
    /// Borrow actual prepared bootstrap only while its owned parent is outside the mutex.
    pub(in crate::production) fn prepare<R>(
        &self,
        build: impl FnOnce(&DiagnosticPipeBootstrap) -> R,
    ) -> Result<Option<R>, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let parent = loan.parent()?;
        if !parent.prepare().map_err(|_| ReporterSourceIssue::Parent)? {
            return Ok(None);
        }
        Ok(parent.bootstrap().map(build))
    }
    /// Native launch input remains in the genuine parent on rejection.
    pub(in crate::production) fn install_input(
        &self,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<(), ReporterSourceIssue> {
        self.loan()?.parent()?.install(input).map_err(|_| ReporterSourceIssue::Parent)
    }
    /// Only the owned process registry performs actual suspended enrollment and resume.
    pub(in crate::production) fn launch(
        &self,
        registry: &mut WindowsProcessRegistry,
        client: &ExecutionClient,
        scope: &Scope,
        specs: ReporterLaunch,
    ) -> Result<(), ReporterSourceIssue> {
        self.loan()?
            .parent()?
            .launch(registry, client, scope, specs)
            .map_err(|_| ReporterSourceIssue::Parent)
    }
    /// Install one genuine prepared view in a fixed slot; denial leaves its exact owner untouched.
    pub(in crate::production) fn install_view(
        &self,
        view: &mut Option<PreparedReportView>,
        plan: ReporterViewPlan,
    ) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if state.first.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        let original = view.as_ref().ok_or(ReporterSourceIssue::Busy)?;
        stream::Stream::validate(original, &plan)?;
        let slot =
            state.views.iter_mut().find(|slot| slot.is_none()).ok_or(ReporterSourceIssue::Busy)?;
        *slot = Some(stream::Stream::new(view.take().ok_or(ReporterSourceIssue::Busy)?, plan));
        Ok(())
    }
    /// Queue complete original authentication; rejection never takes either original token.
    pub(in crate::production) fn install_completion(
        &self,
        input: &mut Option<ReporterCompletion>,
    ) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if state.completion.is_some() || state.output.is_some() || state.first.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        state.completion = Some(input.take().ok_or(ReporterSourceIssue::Busy)?);
        Ok(())
    }
    /// Queue a closed actual host decision; encoding remains exclusively finite Core work.
    pub(in crate::production) fn install_control(
        &self,
        input: &mut Option<ReporterControlInput>,
    ) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if state.control.is_some() || state.control_write.is_some() || state.first.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        state.control = Some(input.take().ok_or(ReporterSourceIssue::Busy)?);
        Ok(())
    }
}
/// Authenticated and settled are independent actual transport facts, not durable storage proof.
pub(in crate::production) struct ReporterFacts {
    pub(in crate::production) changed: bool,
    pub(in crate::production) failed: bool,
    pub(in crate::production) authenticated: bool,
    pub(in crate::production) process: Option<ProcessKey>,
    pub(in crate::production) pipe_settled: bool,
}
/// One immutable original context for the complete begin/chunks/end transfer.
#[derive(Clone, Copy)]
pub(in crate::production) struct ReporterViewPlan {
    pub(in crate::production) key: DiagnosticKey,
    pub(in crate::production) first_request: DiagnosticRequestId,
    pub(in crate::production) context: DiagnosticContext,
    pub(in crate::production) until: Moment,
    pub(in crate::production) previous: Option<IncidentRevision>,
}
struct Loan<'a> {
    source: &'a ReporterSource,
    state: Option<Box<State>>,
}
impl Loan<'_> {
    fn state(&mut self) -> Result<&mut State, ReporterSourceIssue> {
        self.state.as_deref_mut().ok_or(ReporterSourceIssue::Busy)
    }
    fn parent(&mut self) -> Result<&mut ReporterParent, ReporterSourceIssue> {
        self.state()?.parent.as_deref_mut().ok_or(ReporterSourceIssue::Busy)
    }
}
impl Drop for Loan<'_> {
    fn drop(&mut self) {
        *self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            self.state.take();
    }
}
impl State {
    fn poll(&mut self) -> Result<bool, ReporterSourceIssue> {
        let parent = self.parent.as_mut().ok_or(ReporterSourceIssue::Busy)?;
        let changed = parent.poll().map_err(|_| ReporterSourceIssue::Parent)?;
        if let Some(operation) = parent.take_written() {
            if self.control_write == Some(operation) {
                self.control_written = true;
            } else {
                for slot in self.views.iter_mut().flatten() {
                    slot.written(operation);
                }
            }
        }
        Ok(changed)
    }
}
