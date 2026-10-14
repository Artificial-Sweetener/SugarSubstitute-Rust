// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one registered development preview request, central persistence and checked terminal disposal.

#[cfg(test)]
#[path = "preview/tests.rs"]
mod tests;
#[path = "preview/worker.rs"]
mod worker;
use crate::{job_sequence::JobSequence, production::qualification::PreviewIntent};
use std::{error::Error, fmt};
use sugarsubstitute_application::startup::{StartupPhase, StartupSession};
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey, startup::MainIdentity};
use sugarsubstitute_desktop::{
    CapturedMainPreview, DesktopError, DesktopStartupCommand, DesktopStartupObservation,
};
use sugarsubstitute_execution::{ExecutionClient, PreviewSourceLoan};
use worker::{PersistFailure, PersistInput, Persisted};

/// Preserve qualification rejection independently of ordinary product startup health.
#[derive(Debug)]
pub(super) enum PreviewError {
    State,
    Identity,
    Deadline,
    Capacity,
    Execution(ExecutionError),
    Admission(sugarsubstitute_execution::BlockingInputError),
    JobIdentity(IdentityError),
}
impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Native preview qualification rejected")
    }
}
impl Error for PreviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::JobIdentity(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Retain the first typed original cause without an additional erased heap allocation.
enum PreviewFailure {
    Native(DesktopError),
    Host(PreviewError),
}
impl fmt::Display for PreviewFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(cause) => fmt::Display::fmt(cause, formatter),
            Self::Host(cause) => fmt::Display::fmt(cause, formatter),
        }
    }
}

/// Retain original intent/cutoff/input/job/result across cancellation; publication grants no policy.
pub(super) struct Preview {
    intent: Option<PreviewIntent>,
    source: Option<PreviewSourceLoan>,
    run: RunKey,
    until: Moment,
    main: Option<MainIdentity>,
    input: Option<PersistInput>,
    job: Option<OwnedJob<Persisted, PersistFailure>>,
    terminal: Option<JobCompletion<Persisted, PersistFailure>>,
    rejected: Option<DesktopStartupObservation>,
    notice: Option<CompletionNotification>,
    requested: bool,
    received: bool,
    closing: bool,
    failure: Option<PreviewFailure>,
    reported: bool,
    terminal_rejected: bool,
}

impl Preview {
    /// Preflight the original intent and source before consuming either caller-owned input.
    /// Every rejection leaves both options unchanged; successful construction creates no effect.
    pub(super) fn new(
        intent: &mut Option<PreviewIntent>,
        run: RunKey,
        source: &mut Option<PreviewSourceLoan>,
    ) -> Result<Self, PreviewError> {
        let original_intent = intent.as_ref().ok_or(PreviewError::State)?;
        let original_source = source.as_ref().ok_or(PreviewError::State)?;
        if original_intent.deadline() != original_source.counter_deadline() {
            return Err(PreviewError::Deadline);
        }
        let controller_bytes = Self::storage_bytes()
            .checked_add(original_intent.allocated_bytes())
            .ok_or(PreviewError::Capacity)?;
        if controller_bytes > original_source.control_storage_bytes() {
            return Err(PreviewError::Capacity);
        }
        let until = original_source.original_deadline();
        Self::validate(original_intent, until)?;
        match (intent.take(), source.take()) {
            (Some(intent), Some(source)) => Ok(Self::compose(intent, run, until, Some(source))),
            (original_intent, original_source) => {
                *intent = original_intent;
                *source = original_source;
                Err(PreviewError::State)
            }
        }
    }

    /// Include this inline owner in the composing host's existing metadata preflight.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Self>()
    }

    fn validate(intent: &PreviewIntent, until: Moment) -> Result<(), PreviewError> {
        if until.as_nanos() == 0 {
            return Err(PreviewError::Deadline);
        }
        if !worker::layout_fits(intent.allocated_bytes()) {
            return Err(PreviewError::Capacity);
        }
        Ok(())
    }

    fn compose(
        intent: PreviewIntent,
        run: RunKey,
        until: Moment,
        source: Option<PreviewSourceLoan>,
    ) -> Self {
        Self {
            intent: Some(intent),
            source,
            run,
            until,
            main: None,
            input: None,
            job: None,
            terminal: None,
            rejected: None,
            notice: None,
            requested: false,
            received: false,
            closing: false,
            failure: None,
            reported: false,
            terminal_rejected: false,
        }
    }

    /// Fake only source production for controller tests that perform no filesystem effect.
    #[cfg(test)]
    fn fixture_new(
        intent: PreviewIntent,
        run: RunKey,
        until: Moment,
    ) -> Result<Self, PreviewError> {
        Self::validate(&intent, until)?;
        Ok(Self::compose(intent, run, until, None))
    }

    /// Freeze one actual-current Running Main request within the original campaign cutoff.
    /// The completed frontend setup budget grants no new lifetime and does not expire Running.
    pub(super) fn after_main_revealed(
        &mut self,
        identity: MainIdentity,
        session: &StartupSession,
        scope: &Scope,
        native_scope: &Scope,
        now: Moment,
    ) -> Result<Option<DesktopStartupCommand>, PreviewError> {
        if self.closing || self.requested || self.failure.is_some() {
            return Err(PreviewError::State);
        }
        if identity.run != self.run {
            return Err(PreviewError::Identity);
        }
        if session.launch() != identity.launch || session.navigation() != identity.navigation {
            return Err(PreviewError::Identity);
        }
        let scope_state = scope.snapshot();
        let native_state = native_scope.snapshot();
        if session.phase() != StartupPhase::Running
            || session.failure().is_some()
            || scope_state.closed
            || scope_state.cancellation_requested
            || native_state.closed
            || native_state.cancellation_requested
        {
            return Err(PreviewError::State);
        }
        if now >= self.until {
            return Err(PreviewError::Deadline);
        }
        let source_deadline = self.intent.as_ref().ok_or(PreviewError::State)?.deadline();
        self.requested = true;
        self.main = Some(identity);
        Ok(Some(DesktopStartupCommand::CaptureMainPreview { identity, source_deadline }))
    }

    /// Consume only exact requested preview evidence; other native observations retain their owner.
    pub(super) fn accept(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<bool, PreviewError> {
        match observation.as_ref() {
            Some(DesktopStartupObservation::MainPreviewCaptured(image)) => {
                self.validate_image(image)?;
            }
            Some(DesktopStartupObservation::MainPreviewFailed { identity, .. }) => {
                if !self.requested || self.received || self.main != Some(*identity) {
                    return Err(PreviewError::Identity);
                }
            }
            _ => return Ok(false),
        }
        match observation.take() {
            Some(DesktopStartupObservation::MainPreviewCaptured(image)) => {
                self.accept_image(image, client, scope, jobs)?;
            }
            Some(DesktopStartupObservation::MainPreviewFailed { cause, .. }) => {
                self.received = true;
                self.record_failure(PreviewFailure::Native(cause));
            }
            other => {
                *observation = other;
                return Err(PreviewError::State);
            }
        }
        Ok(true)
    }

    fn validate_image(&self, image: &CapturedMainPreview) -> Result<(), PreviewError> {
        if !self.requested
            || self.received
            || self.input.is_some()
            || self.job.is_some()
            || self.main != Some(image.snapshot().identity())
        {
            return Err(PreviewError::Identity);
        }
        let intent = self.intent.as_ref().ok_or(PreviewError::State)?;
        if image.snapshot().source_deadline() != intent.deadline()
            || image.image_capacity_bytes() != worker::IMAGE_BYTES
            || image.metadata_capacity_bytes() > 8192
        {
            return Err(PreviewError::Identity);
        }
        Ok(())
    }

    fn accept_image(
        &mut self,
        image: CapturedMainPreview,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), PreviewError> {
        let intent = self.intent.take().ok_or(PreviewError::State)?;
        self.received = true;
        self.input = Some(PersistInput {
            destination: Some(intent.into_destination()),
            source: self.source.take(),
            image,
            until: self.until,
            client: client.clone(),
        });
        if self.closing {
            return Ok(());
        }
        if client.now().map_err(PreviewError::Execution)? >= self.until {
            self.record_failure(PreviewFailure::Host(PreviewError::Deadline));
            return Ok(());
        }
        let id = jobs.allocate().map_err(PreviewError::JobIdentity)?;
        let spec = JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(self.until),
            reservation_bytes: worker::JOB_BYTES,
            replacement: None,
        };
        match client.submit_blocking_owned_input(scope, spec, &mut self.input, worker::persist) {
            Ok(job) => self.job = Some(job),
            Err(cause) => self.record_failure(PreviewFailure::Host(PreviewError::Admission(cause))),
        }
        Ok(())
    }

    /// Retain one rejected preview observation without consuming ordinary observations or allocating.
    /// A full slot leaves the caller's observation untouched and reports its still-owned custody.
    pub(super) fn reject_retained(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> Result<bool, PreviewError> {
        if !matches!(
            observation,
            Some(
                DesktopStartupObservation::MainPreviewCaptured(_)
                    | DesktopStartupObservation::MainPreviewFailed { .. }
            )
        ) {
            return Ok(false);
        }
        if self.rejected.is_some()
            || self.input.is_some()
            || self.job.is_some()
            || self.terminal.is_some()
        {
            return Err(PreviewError::State);
        }
        self.rejected = observation.take();
        Ok(true)
    }

    /// Route immutable notifications without consuming a foreign job or assuming terminal delivery.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(job) = &self.job {
            for notice in &batch.notifications {
                if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                    self.notice = Some(*notice);
                }
            }
        }
    }

    /// Consume only an actual transferred terminal; optional failures stay in this qualifier owner.
    pub(super) fn poll(&mut self, now: Moment) -> Result<(), PreviewError> {
        if self.notice.is_some()
            && let Some(job) = &mut self.job
            && let Some(terminal) = job.try_take_completion()
        {
            self.terminal_rejected = terminal.scope_id != job.scope_id()
                || terminal.spec != job.spec()
                || self.notice.is_none_or(|notice| {
                    notice.kind != terminal.outcome.kind() || notice.replacement.is_some()
                })
                || terminal.memory.bytes() != worker::JOB_BYTES
                || terminal.native_handles.is_some()
                || terminal.spec.class != JobClass::InfrastructureOrdinary
                || terminal.spec.operation != Operation::Infrastructure
                || terminal.spec.deadline != Some(self.until)
                || terminal.spec.reservation_bytes != worker::JOB_BYTES
                || terminal.spec.replacement.is_some();
            self.terminal = Some(terminal);
            self.job = None;
            self.notice = None;
            if self.terminal_rejected {
                self.record_failure(PreviewFailure::Host(PreviewError::Identity));
            }
        }
        if now >= self.until
            && let Some(job) = &self.job
        {
            job.cancel();
        }
        if self.job.is_none() && (self.closing || now >= self.until || self.failure.is_some()) {
            self.input = None;
            self.rejected = None;
            self.source = None;
        }
        if self.reported || self.terminal_rejected {
            return Ok(());
        }
        if let Some(terminal) = &self.terminal {
            match &terminal.outcome {
                JobOutcome::Success(value)
                    if value.artifact.retired() && value.artifact.activated() =>
                {
                    tracing::info!(operation="main_preview_persisted", identity=?value.image.snapshot().identity(),
                        png_bytes=value.image.as_bytes().len(), png_sha256=%worker::Digest(&value.digest),
                        native_navigation=value.image.snapshot().native_navigation(),
                        viewport_width=value.image.snapshot().viewport().width(),
                        viewport_height=value.image.snapshot().viewport().height(),
                        dpi=value.image.snapshot().viewport().dpi(),
                        "Actual current renderer PNG persisted with checked native retirement");
                }
                JobOutcome::Failure(cause) => {
                    tracing::warn!(operation="main_preview_persist_failed", identity=?self.main,
                        cause=%cause, native_retired=cause.retired(),
                        "Original preview filesystem failure retained");
                }
                other => {
                    tracing::warn!(operation="main_preview_terminal", identity=?self.main,
                        terminal=?other.kind(), "Preview qualification has no successful artifact");
                }
            }
            self.reported = true;
        }
        if self.terminal.as_ref().is_some_and(|terminal| match &terminal.outcome {
            JobOutcome::Success(value) => value.artifact.retired(),
            JobOutcome::Failure(cause) => cause.retired(),
            _ => true,
        }) {
            self.terminal = None;
        }
        Ok(())
    }

    /// Cancel existing work without equating the request with termination or native closure.
    pub(super) fn close(&mut self) {
        self.closing = true;
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    /// Cancel an already requested exact Main only after its real retirement command admission.
    /// A pre-reveal retry keeps the same registered run loan and immutable campaign.
    pub(super) fn retire_main(&mut self, identity: MainIdentity) {
        if self.requested && self.main == Some(identity) {
            self.close();
        }
    }

    /// Require actual job/input/result disposal; an uncertain native close remains unsettled.
    pub(super) fn settled(&self) -> bool {
        !self.pending()
    }

    /// Observe retained input, rejected evidence, actual jobs and uncertain native terminal custody.
    /// Callback/controller obligations remain with the Desktop owner rather than this worker state.
    pub(super) fn pending(&self) -> bool {
        self.source.is_some()
            || self.job.is_some()
            || self.input.is_some()
            || self.rejected.is_some()
            || self.terminal.is_some()
    }

    fn record_failure(&mut self, cause: PreviewFailure) {
        if self.failure.is_none() {
            tracing::warn!(operation="main_preview_qualification_failed", identity=?self.main,
                cause=%cause, "Original optional preview failure retained");
            self.failure = Some(cause);
        }
    }
}
