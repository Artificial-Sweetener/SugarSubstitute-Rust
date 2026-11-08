// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the real captured child and authenticated configuration before native UI.
use super::{
    capture::ChildCapture,
    channel::{
        ChildChannel,
        preparation::{CaptureInput, HandshakePhase},
    },
    configuration::{
        self, ChildConfigurationFailure, PreparedChildConfiguration, PreparedChildStartup,
    },
};
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::{
        CompletionBatch, Generation, JobCompletion, JobOutcome, Moment, OwnedJob, Scope, ScopeSpec,
        TerminalKind,
    },
    lifecycle::ShutdownBudget,
    startup::LaunchId,
};
use sugarsubstitute_desktop::StartupPreparationStep;
use sugarsubstitute_execution::{
    ExecutionClient, ProcessLifetimeCapture,
    windows_capture_attach::{CaptureAttachFailure, CaptureCleanupSnapshot},
};

#[path = "bootstrap/observations.rs"]
mod observations;
#[path = "bootstrap/qualification.rs"]
mod qualification;

#[path = "bootstrap/transport.rs"]
mod transport;

type ProjectionJob = OwnedJob<PreparedChildConfiguration, ChildConfigurationFailure>;
type ProjectionCompletion = JobCompletion<PreparedChildConfiguration, ChildConfigurationFailure>;

#[cfg(feature = "k1-native-qualification")]
#[path = "bootstrap/preview_hooks.rs"]
mod preview_hooks;
#[path = "bootstrap/projection.rs"]
mod projection;

/// Identify a rejected phase; complete underlying failures stay at their real owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChildBootstrapIssue {
    State,
    Identity,
    Transport,
    Capture,
    Configuration,
    Deadline,
}

/// Closed diagnostic facts; none grants native, configuration or readiness authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparationObservation {
    channel_phase: HandshakePhase,
    channel_terminal: Option<TerminalKind>,
    capture_input: CaptureInput,
    capture_admitted: bool,
    capture: bool,
    registered: bool,
    configuration: bool,
    original_failure: Option<ChildBootstrapIssue>,
    capture_failure: CaptureFailureKind,
    failure_retained: bool,
}

/// Redact the first owned capture failure to a closed data-only category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureFailureKind {
    None,
    State,
    Identity,
    Admission,
    Data,
    Source,
    Native,
    TimedOut,
    Cancelled,
    Uncertain,
    Other,
}

#[cfg(test)]
#[path = "bootstrap/tests.rs"]
mod tests;
impl fmt::Display for ChildBootstrapIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Child bootstrap failed: {self:?}")
    }
}
impl Error for ChildBootstrapIssue {}

/// Retain original finite/native scopes, admitted capture and configuration custody.
/// Capture readiness does not establish backend, browser or frontend readiness.
pub(super) struct ChildBootstrap {
    #[cfg(feature = "k1-native-qualification")]
    qualification: super::qualification::ChildQualification,
    #[cfg(feature = "k1-native-qualification")]
    qualification_run: Option<sugarsubstitute_contracts::lifecycle::RunKey>,
    channel: ChildChannel,
    capture: ChildCapture,
    native_spec: ScopeSpec,
    identity: (LaunchId, Generation),
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    native_scope: Option<Scope>,
    jobs: Option<JobSequence>,
    capture_started: bool,
    original: Option<Moment>,
    projection: Option<ProjectionJob>,
    notice: Option<TerminalKind>,
    failed_projection: Option<ProjectionCompletion>,
    prepared: Option<PreparedChildStartup>,
    delivered: bool,
    failure: Option<Box<dyn Error>>,
    issue: Option<ChildBootstrapIssue>,
    started: bool,
    closing: bool,
    last_preparation: Option<PreparationObservation>,
}
impl ChildBootstrap {
    /// Compose inert owners and application-assigned identities without native effects.
    /// # Errors
    /// Rejects a deadline on the persistent native channel scope before any admission.
    pub(super) fn new(
        channel: ChildChannel,
        capture: ChildCapture,
        native_spec: ScopeSpec,
        identity: (LaunchId, Generation),
    ) -> Result<Self, ChildBootstrapIssue> {
        if native_spec.deadline.is_some() {
            return Err(ChildBootstrapIssue::Identity);
        }
        Ok(Self {
            #[cfg(feature = "k1-native-qualification")]
            qualification: super::qualification::ChildQualification::new(Default::default()),
            #[cfg(feature = "k1-native-qualification")]
            qualification_run: None,
            channel,
            capture,
            native_spec,
            identity,
            client: None,
            scope: None,
            native_scope: None,
            jobs: None,
            capture_started: false,
            original: None,
            projection: None,
            notice: None,
            failed_projection: None,
            prepared: None,
            delivered: false,
            failure: None,
            issue: None,
            started: false,
            closing: false,
            last_preparation: None,
        })
    }

    /// Retain the real scope before native-scope construction or transport admission.
    /// # Errors
    /// Preserves the actual cause and original scopes when partial setup fails.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: Scope,
        jobs: &JobSequence,
    ) -> Result<(), ChildBootstrapIssue> {
        if self.started {
            return Err(ChildBootstrapIssue::State);
        }
        self.started = true;
        self.scope = Some(scope);
        self.client = Some(client.clone());
        self.jobs = Some(jobs.clone());
        if self.closing {
            return Err(ChildBootstrapIssue::State);
        }
        let state = self.scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?.snapshot();
        if state.closed || state.cancellation_requested {
            self.closing = true;
            return Err(ChildBootstrapIssue::State);
        }
        let native = client
            .create_native_scope(self.native_spec, None)
            .map_err(|cause| self.record(ChildBootstrapIssue::Identity, Box::new(cause)))?;
        self.native_scope = Some(native);
        let id = jobs
            .allocate()
            .map_err(|cause| self.record(ChildBootstrapIssue::Identity, Box::new(cause)))?;
        let scope = self.native_scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
        if let Err(cause) = self.channel.start(client, scope, id) {
            return Err(self.record(ChildBootstrapIssue::Transport, cause));
        }
        Ok(())
    }

    /// Route the same immutable batch to transport and capture before resolving either error.
    /// # Errors
    /// Retains complete configuration terminal outcomes on failure or inconsistent metadata.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), ChildBootstrapIssue> {
        let channel = self.channel.consume(batch);
        let capture = self.capture.consume(batch);
        #[cfg(feature = "k1-native-qualification")]
        let qualification = self.qualification.capture(batch);
        let projection = self.capture_projection(batch);
        #[cfg(feature = "k1-native-qualification")]
        self.poll_registered_preview();
        let mut issue = None;
        if let Err(cause) = channel {
            issue = Some(self.record(ChildBootstrapIssue::Transport, cause));
        }
        if let Some(transport_issue) = self.capture_transport_failure() {
            issue.get_or_insert(transport_issue);
        }
        if let Err(cause) = capture {
            let capture_issue = self.record(ChildBootstrapIssue::Capture, Box::new(cause));
            issue.get_or_insert(capture_issue);
        }
        #[cfg(feature = "k1-native-qualification")]
        if let Err(cause) = qualification {
            let qualification_issue = self.record(ChildBootstrapIssue::Capture, cause);
            issue.get_or_insert(qualification_issue);
        }
        issue.map_or(projection, Err)
    }

    /// Drive genuine auth/capture/configuration; no surface exists until all prerequisites pass.
    /// # Errors
    /// Rejects expired or closed original scope/source without publishing prepared application state.
    pub(super) fn prepare_native_ui(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<StartupPreparationStep, ChildBootstrapIssue> {
        if !self.started {
            return Err(ChildBootstrapIssue::State);
        }
        if let Err(cause) = self.channel.activity(maximum.min(NonZeroUsize::MIN.saturating_add(31)))
        {
            return Err(self.record(ChildBootstrapIssue::Transport, cause));
        }
        if self.closing {
            return Ok(StartupPreparationStep::Closing);
        }
        self.check_current()?;
        if !self.capture_started
            && let Some(input) = self.channel.take_captured_startup()
        {
            self.original = Some(input.capture.original_deadline());
            self.capture_started = true;
            let id = self
                .jobs
                .as_ref()
                .ok_or(ChildBootstrapIssue::Identity)?
                .allocate()
                .map_err(|cause| self.record(ChildBootstrapIssue::Identity, Box::new(cause)))?;
            let client = self.client.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
            let scope = self.scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
            #[cfg(feature = "k1-native-qualification")]
            let mut input = input;
            #[cfg(feature = "k1-native-qualification")]
            self.qualification.declare_preview_storage(&mut input.capture);
            if self.capture.start(client, scope, id, input).is_err() {
                self.issue.get_or_insert(ChildBootstrapIssue::Capture);
                self.close_initialization();
                return Err(ChildBootstrapIssue::Capture);
            }
        }
        if self.capture.terminal().is_some() && !self.capture.ready() {
            self.issue.get_or_insert(ChildBootstrapIssue::Capture);
            self.close_initialization();
            return Err(ChildBootstrapIssue::Capture);
        }
        #[cfg(feature = "k1-native-qualification")]
        if let Some(step) = self.registered_fault_step()? {
            return Ok(step);
        }
        #[cfg(feature = "k1-native-qualification")]
        self.prepare_registered_preview();
        if self.projection.is_none()
            && self.prepared.is_none()
            && !self.delivered
            && let Some(disposition) = self.channel.backend_data_disposition()
            && let Some(source) = self.capture.take_startup()
        {
            let id = self
                .jobs
                .as_ref()
                .ok_or(ChildBootstrapIssue::Identity)?
                .allocate()
                .map_err(|cause| self.record(ChildBootstrapIssue::Identity, Box::new(cause)))?;
            let client = self.client.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
            let scope = self.scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
            let until = self.original.ok_or(ChildBootstrapIssue::Identity)?;
            self.projection = Some(
                configuration::submit(client, scope, id, source, disposition, until).map_err(
                    |cause| self.record(ChildBootstrapIssue::Configuration, Box::new(cause)),
                )?,
            );
        }
        if self.capture.ready()
            && self.channel.registered()
            && (self.prepared.is_some() || self.delivered)
        {
            Ok(StartupPreparationStep::Ready)
        } else {
            Ok(StartupPreparationStep::Waiting)
        }
    }

    /// Check the original finite scope and source cutoff before a child transaction.
    /// # Errors
    /// Fences initialization on closure, cancellation, expiration or clock failure.
    pub(super) fn check_current(&mut self) -> Result<(), ChildBootstrapIssue> {
        let scope = self.scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
        let state = scope.snapshot();
        if state.closed || state.cancellation_requested {
            self.issue.get_or_insert(ChildBootstrapIssue::State);
            self.close_initialization();
            return Err(ChildBootstrapIssue::State);
        }
        let until = self
            .original
            .or_else(|| self.channel.original_deadline())
            .ok_or(ChildBootstrapIssue::Identity)?;
        let now = self
            .client
            .as_ref()
            .ok_or(ChildBootstrapIssue::Identity)?
            .now()
            .map_err(|cause| self.record(ChildBootstrapIssue::Deadline, Box::new(cause)))?;
        if now >= until {
            self.issue.get_or_insert(ChildBootstrapIssue::Deadline);
            self.close_initialization();
            return Err(ChildBootstrapIssue::Deadline);
        }
        Ok(())
    }
    fn record(&mut self, issue: ChildBootstrapIssue, cause: Box<dyn Error>) -> ChildBootstrapIssue {
        self.issue.get_or_insert(issue);
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
        self.close_initialization();
        issue
    }
    /// Move the complete guarded native-thread policy once after actual prerequisites.
    pub(super) fn take_prepared_startup(&mut self) -> Option<PreparedChildStartup> {
        if self.closing || !self.capture.ready() || !self.channel.registered() {
            return None;
        }
        let prepared = self.prepared.take()?;
        self.delivered = true;
        Some(prepared)
    }
    /// Fence initialization and latch actual capture cleanup before the outer budget owner.
    /// The transport remains alive for authenticated final lifecycle communication.
    pub(super) fn close_initialization(&mut self) {
        self.closing = true;
        #[cfg(feature = "k1-native-qualification")]
        self.qualification.close();
        if let Some(job) = &self.projection {
            job.cancel();
        }
        self.capture.request_cleanup();
    }
    /// Borrow the capture owner's one original cleanup transaction without a new sample.
    pub(super) fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        match self.capture.cleanup_snapshot()? {
            CaptureCleanupSnapshot::Latched { budget, .. } => Some(budget),
            _ => None,
        }
    }
    /// Borrow the actual channel for output, profile and native challenge transactions.
    pub(super) fn channel_mut(&mut self) -> &mut ChildChannel {
        &mut self.channel
    }
    pub(super) fn ui_deadline(&self) -> Option<Moment> {
        self.channel.ui_deadline()
    }
    /// Borrow the actual retained ordinary scope for finite application admissions.
    pub(super) fn scope(&self) -> Option<&Scope> {
        self.scope.as_ref()
    }
    /// Borrow the retained persistent transport scope for checked shutdown joins.
    #[cfg(test)]
    pub(super) fn native_scope(&self) -> Option<&Scope> {
        self.native_scope.as_ref()
    }
    /// Borrow the original capture or authenticated handshake cutoff without renewal.
    pub(super) fn original_deadline(&self) -> Option<Moment> {
        self.original.or_else(|| self.channel.original_deadline())
    }

    /// Keep every admitted qualification job observable until its actual terminal is consumed.
    /// This cache-only observation creates no cutoff, effect, or retirement authority.
    pub(super) fn qualification_pending(&self) -> bool {
        #[cfg(feature = "k1-native-qualification")]
        {
            self.qualification.pending()
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        {
            false
        }
    }
    /// Observe configuration work independently of transport or native retirement.
    pub(super) fn projection_pending(&self) -> bool {
        self.projection.is_some()
    }
    /// Borrow native cleanup facts without sampling a new shutdown transaction.
    pub(super) fn capture_cleanup_snapshot(&self) -> Option<CaptureCleanupSnapshot> {
        self.capture.cleanup_snapshot()
    }
    /// Observe whether an actual admitted capture job still lacks its terminal result.
    #[cfg(test)]
    pub(super) fn capture_pending(&self) -> bool {
        self.capture.admitted() && self.capture.terminal().is_none()
    }
    /// Observe genuine capture admission independently of completion and cleanup.
    pub(super) fn capture_admitted(&self) -> bool {
        self.capture.admitted()
    }
    /// Borrow passive temporary disposition from the identical actual admitted capture session.
    /// An absent session is distinct from returned-unused, retired or transferred custody.
    pub(super) fn temporary_settled(
        &self,
    ) -> Option<sugarsubstitute_execution::DesktopCaptureTemporaryState> {
        self.capture.temporary_settled()
    }

    /// Transfer the genuine separately published process-lifetime capsule once.
    pub(super) fn try_take_process_lifetime(
        &self,
    ) -> Option<Result<ProcessLifetimeCapture, CaptureAttachFailure>> {
        self.capture.try_take_process_lifetime()
    }
    /// Return local map loans after the caller matches actual NativeRetired.
    /// Charged configuration and all original failure causes remain retained.
    /// # Errors
    /// Rejects while a projection worker can still access the original snapshot.
    pub(super) fn release_source_after_native_retired(
        &mut self,
    ) -> Result<(), ChildBootstrapIssue> {
        if self.projection_pending() || self.qualification_pending() {
            return Err(ChildBootstrapIssue::State);
        }
        if let Some(prepared) = &mut self.prepared {
            prepared.release_source();
        }
        if let Some(completion) = &mut self.failed_projection {
            match &mut completion.outcome {
                JobOutcome::Success(prepared) => prepared.release_source(),
                JobOutcome::Failure(failure) => failure.release_source(),
                _ => {}
            }
        }
        self.capture.release_startup_source();
        Ok(())
    }
    /// Borrow the first actual adapter or retained typed projection cause.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        #[cfg(feature = "k1-native-qualification")]
        if let Some(cause) = self.qualification.failure() {
            return Some(cause);
        }
        if let Some(cause) = &self.failure {
            return Some(cause.as_ref());
        }
        if let Some(completion) = &self.failed_projection
            && let JobOutcome::Failure(cause) = &completion.outcome
        {
            return Some(cause);
        }
        if let Some(cause) = self.capture.admission_failure() {
            return Some(cause);
        }
        if let Some(completion) = self.capture.terminal()
            && let JobOutcome::Failure(cause) = &completion.outcome
        {
            return Some(cause);
        }
        None
    }
}
