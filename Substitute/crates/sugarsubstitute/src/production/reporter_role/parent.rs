// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enroll the actual suspended Reporter before resume and accept only its same native session.
use super::channel::Channel;
#[path = "parent/recycling.rs"]
pub(super) mod recycling;
#[path = "parent/relay.rs"]
mod relay;
use std::error::Error;
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_execution::{ExecutionClient, windows_purpose_pipe::*};
use sugarsubstitute_supervision::{
    LaunchTicket, RegistryAdmissionError, RegistryInputError, WindowsLaunchInput,
    WindowsProcessRegistry,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ReporterIssue {
    Role,
    Busy,
    Transport,
    Launch,
    Wire,
}
/// Data precedes its original controller claim; no child pipe or family is inferred settled.
pub(in crate::production) struct ReporterParent {
    channel: Channel,
    bootstrap: Option<PurposePipeBootstrap>,
    prepared: bool,
    enrollment: Option<PurposeEnrollment>,
    input: Option<WindowsLaunchInput>,
    launch: Option<LaunchTicket>,
    input_error: Option<RegistryInputError>,
    launch_error: Option<RegistryAdmissionError>,
    send_error: Option<sugarsubstitute_crash::diagnostic_client::DiagnosticSendError>,
    rejected_output: Option<sugarsubstitute_execution::purpose_pipe::LifecyclePipeWriteBuffer>,
    checked_retirement:
        Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeRetirement>,
    execution: ExecutionId,
    failure_transferred: bool,
    retired_terminal: Option<(ScopeId, JobSpec, TerminalKind)>,
    _claim: Option<MemoryClaim>,
}
impl ReporterParent {
    /// Adopt the genuine session after complete borrowed validation of both original slots.
    /// Rejection leaves funding, session and output unchanged; transfer allocates nothing.
    pub(in crate::production) fn install_owner(
        output: &mut Option<Self>,
        funding: &mut Option<ReporterFunding>,
        session: &mut Option<PurposePipeSession>,
        key: DiagnosticKey,
    ) -> Result<(), ReporterIssue> {
        if output.is_some() || funding.is_none() || session.is_none() {
            return Err(ReporterIssue::Busy);
        }
        if session
            .as_ref()
            .and_then(|session| session.control().prepared_key())
            .is_some_and(|actual| actual != PurposeKey::Diagnostic(key))
        {
            return Err(ReporterIssue::Role);
        }
        if key.role() != DiagnosticRole::Reporter {
            return Err(ReporterIssue::Role);
        }
        let Some(funding) = funding.take() else { return Err(ReporterIssue::Busy) };
        let Some(session) = session.take() else { return Err(ReporterIssue::Busy) };
        *output = Some(Self {
            channel: Channel::new(session, key),
            bootstrap: None,
            prepared: false,
            enrollment: None,
            input: None,
            launch: None,
            input_error: None,
            launch_error: None,
            send_error: None,
            rejected_output: None,
            checked_retirement: None,
            execution: funding.owner_id(),
            failure_transferred: false,
            retired_terminal: None,
            _claim: Some(funding.claim),
        });
        Ok(())
    }
    /// Borrow only genuine bootstrap pins for the fixed child invocation arguments.
    pub(in crate::production) fn prepare(&mut self) -> Result<bool, ReporterIssue> {
        if self.failure_transferred {
            return Err(ReporterIssue::Transport);
        }
        if self.prepared {
            return Ok(true);
        }
        if self.bootstrap.is_none()
            && let Some((bootstrap, enrollment)) =
                self.channel.session.take_bootstrap().map_err(|cause| {
                    self.channel.first.get_or_insert(cause);
                    ReporterIssue::Transport
                })?
        {
            self.bootstrap = Some(bootstrap);
            self.enrollment = Some(enrollment);
            if self.bootstrap.as_ref().is_none_or(|bootstrap| {
                bootstrap.key() != PurposeKey::Diagnostic(self.channel.key())
            }) {
                return Err(ReporterIssue::Role);
            }
            self.prepared = true;
        }
        Ok(self.bootstrap.is_some())
    }
    pub(in crate::production) fn bootstrap(&self) -> Option<&PurposePipeBootstrap> {
        self.bootstrap.as_ref()
    }
    /// Retain all original launch inputs through native admission rejection and real family cleanup.
    pub(in crate::production) fn install(
        &mut self,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<(), ReporterIssue> {
        if self.failure_transferred || self.input.is_some() || self.launch.is_some() {
            return Err(ReporterIssue::Busy);
        }
        let value = input.as_mut().ok_or(ReporterIssue::Busy)?;
        if let Err(cause) = value.with_purpose_pipe(&mut self.bootstrap, &mut self.enrollment) {
            self.input_error.get_or_insert(cause);
            return Err(ReporterIssue::Launch);
        }
        self.input = input.take();
        Ok(())
    }
    /// Submit genuine suspended enrollment through the one owned process registry.
    pub(in crate::production) fn launch(
        &mut self,
        registry: &mut WindowsProcessRegistry,
        client: &ExecutionClient,
        scope: &Scope,
        specs: ReporterLaunch,
    ) -> Result<(), ReporterIssue> {
        if self.failure_transferred || self.launch.is_some() {
            return Err(ReporterIssue::Busy);
        }
        let key = self.channel_key()?;
        match registry.submit_diagnostic_launch(
            client,
            scope,
            specs.native,
            specs.creation,
            specs.watch,
            key,
            &mut self.input,
        ) {
            Ok(ticket) => {
                let process = ticket.process();
                self.launch = Some(ticket);
                self.channel.bind(process).map_err(|_| ReporterIssue::Transport)?;
                Ok(())
            }
            Err(cause) => {
                self.launch_error.get_or_insert(cause);
                Err(ReporterIssue::Launch)
            }
        }
    }
    fn channel_key(&self) -> Result<DiagnosticKey, ReporterIssue> {
        let actual = self
            .channel
            .session
            .control()
            .prepared_key()
            .and_then(PurposeKey::diagnostic)
            .ok_or(ReporterIssue::Transport)?;
        if actual != self.channel.key() {
            return Err(ReporterIssue::Role);
        }
        Ok(actual)
    }
    /// Consume actual same-session observations; Connected and verification admission grant nothing.
    pub(in crate::production) fn poll(&mut self) -> Result<bool, ReporterIssue> {
        let result = self.channel.poll().map_err(|_| ReporterIssue::Transport);
        if self.checked_retirement.is_none() {
            self.checked_retirement = self.channel.session.control().take_checked_retirement();
        }
        result
    }
    pub(in crate::production) fn authenticated(&self) -> bool {
        self.channel.authenticated()
    }
    pub(in crate::production) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(JobOutcome::Failure(cause)) =
            self.channel.terminal.as_ref().map(|done| &done.outcome)
        {
            return Some(cause);
        }
        if let Some(cause) = &self.channel.wire {
            return Some(cause);
        }
        if let Some(cause) = &self.launch_error {
            return Some(cause);
        }
        if let Some(cause) = &self.input_error {
            return Some(cause);
        }
        if let Some(cause) = &self.send_error {
            return Some(cause);
        }
        self.channel.first.as_ref().map(|cause| cause as _)
    }
    pub(in crate::production) fn close(&self) {
        self.channel.close();
    }
    /// Return completed read/pre-issue rejected write loans after actual registered family settlement.
    /// No launch ticket means the genuine parent never admitted native creation.
    pub(in crate::production) fn return_closed_inputs(
        &mut self,
        registry: &WindowsProcessRegistry,
    ) -> Result<bool, ReporterIssue> {
        if let Some(process) = self.process() {
            let actual = registry.snapshot(process).map_err(|cause| {
                self.launch_error.get_or_insert(cause);
                ReporterIssue::Launch
            })?;
            if actual.binding
                != sugarsubstitute_supervision::FamilyBinding::Diagnostic(self.channel.key())
            {
                return Err(ReporterIssue::Role);
            }
            if !actual.settled {
                return Ok(false);
            }
        }
        self.channel.return_closed_incoming();
        drop(self.rejected_output.take());
        if self.launch.is_none() {
            drop(self.input.take());
            drop(self.bootstrap.take());
            drop(self.enrollment.take());
        }
        Ok(true)
    }
    /// Return inert same-session correlation without granting native admission.
    pub(in crate::production) fn key(&self) -> DiagnosticKey {
        self.channel.key()
    }
    /// Preserve failure classification after its original cause moves to the checked owner.
    pub(in crate::production) fn failed(&self) -> bool {
        self.failure_transferred || self.failure().is_some()
    }
    /// Move the exact first cause after checked pipe closure; the relay separately verifies its family.
    /// Actual failed native completion metadata is cached before its whole owner moves unchanged.
    pub(in crate::production) fn take_retired_failure(
        &mut self,
    ) -> Option<super::parent_source::ReporterParentFailure> {
        use super::parent_source::ReporterParentFailure as Cause;
        if self.failure_transferred || !self.pipe_settled() {
            return None;
        }
        let cause = if self
            .channel
            .terminal
            .as_ref()
            .is_some_and(|terminal| matches!(terminal.outcome, JobOutcome::Failure(_)))
        {
            let terminal = self.channel.terminal.take()?;
            self.retired_terminal =
                Some((terminal.scope_id, terminal.spec, terminal.outcome.kind()));
            Cause::Native(terminal)
        } else if let Some(cause) = self.channel.wire.take() {
            Cause::Wire(cause)
        } else if let Some(cause) = self.launch_error.take() {
            Cause::Launch(cause)
        } else if let Some(cause) = self.input_error.take() {
            Cause::Input(cause)
        } else if let Some(cause) = self.send_error.take() {
            Cause::Send(cause)
        } else if let Some(cause) = self.channel.first.take() {
            Cause::Transport(cause)
        } else {
            return None;
        };
        self.failure_transferred = true;
        Some(cause)
    }
    /// Borrow actual registered process correlation without granting family settlement.
    pub(in crate::production) fn process(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::ProcessKey> {
        self.launch.as_ref().map(|ticket| ticket.process())
    }
    /// Only the original transport terminal establishes this pipe worker's settlement.
    pub(in crate::production) fn pipe_settled(&self) -> bool {
        let Some(proof) = &self.checked_retirement else { return false };
        let actual = self
            .channel
            .terminal
            .as_ref()
            .map(|terminal| (terminal.scope_id, terminal.spec, terminal.outcome.kind()))
            .or(self.retired_terminal);
        let Some((scope, spec, _kind)) = actual else { return false };
        proof.matches_purpose(self.channel.session.control())
            && proof.execution_id() == self.execution
            && proof.scope_id() == scope
            && proof.spec() == spec
    }
}
/// Actual registry jobs and the independently retained no-deadline native scope.
pub(in crate::production) struct ReporterLaunch {
    pub(in crate::production) native: ScopeSpec,
    pub(in crate::production) creation: JobSpec,
    pub(in crate::production) watch: JobSpec,
}
impl ReporterLaunch {
    /// Allocate in actual registry admission order: native root observation precedes creation.
    /// The original finite cutoff, required input bytes, independent native scope and fixed
    ///4096-byte no-deadline watch remain unchanged. Exhaustion burns already consumed IDs.
    /// # Errors
    /// Returns original sequence exhaustion before submitting either job or moving native input.
    pub(in crate::production) fn allocate(
        native: ScopeSpec,
        jobs: &crate::job_sequence::JobSequence,
        until: Moment,
        bytes: u64,
    ) -> Result<Self, IdentityError> {
        let watch = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 4096,
            replacement: None,
        };
        let creation = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        };
        Ok(Self { native, creation, watch })
    }
}
const _: () = assert!(size_of::<ReporterParent>() + 2 * size_of::<usize>() < 16384);

/// Allocation-only fixed metadata admitted inside the original pre-capture controller.
pub(in crate::production) struct ReporterFunding {
    claim: MemoryClaim,
    original: MemoryLease,
    owner: ExecutionId,
}
pub(in crate::production) const REPORTER_METADATA_BYTES: u64 = 16384;
impl ReporterFunding {
    /// Retain the same original controller lease outside any relay Arc header/loan.
    pub(in crate::production) fn original_memory(&self) -> &MemoryLease {
        &self.original
    }
    /// Correlate finite relay admissions with the genuine funding executor.
    pub(in crate::production) fn owner_id(&self) -> ExecutionId {
        self.owner
    }

    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            claim: context.memory().claim(REPORTER_METADATA_BYTES)?,
            original: context.memory().clone(),
            owner: context.owner_id(),
        })
    }
}
