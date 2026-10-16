// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact report reads and metadata commits through independently checked cleanup.
#[path = "reporting/authentication.rs"]
mod authentication;
#[path = "reporting/prepared_view.rs"]
mod prepared_view;
#[path = "reporting/service.rs"]
mod service;
#[path = "reporting/source.rs"]
mod source;
pub(in crate::production) use prepared_view::PreparedReportView;
use prepared_view::PreparedViewContent;
use symbols::ReportingSymbols;
#[path = "reporting/failure.rs"]
mod failure;
#[path = "reporting/symbols.rs"]
mod symbols;
pub(in crate::production) use failure::OwnedReportingFailure;
pub(in crate::production) use service::{
    PassiveReportingRestoration, ReportingIssue, ReportingService, RetainedPassiveReporting,
};
/// Fixed source/control storage inside the original controller reservation.
pub(in crate::production) const REPORT_SOURCE_BYTES: u64 = 16 * 1024;
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::{
    diagnostic_client::{IssuedDiagnosticRequest, ReceivedDiagnosticRequest},
    incident::*,
    report_view::{ReportView, ReportViewError},
};
use sugarsubstitute_supervision::diagnostic_storage::*;

/// One finite presentation admission funds bounded text and its retained preparation metadata.
pub(in crate::production) const REPORT_VIEW_JOB_BYTES: u64 = 320 * 1024;
const VIEW_METADATA_BYTES: u64 = 32 * 1024;
/// One actual protected write loan and exact logical operation, never a caller-supplied path.
pub(in crate::production) struct ReportingInput {
    pub(in crate::production) request: ProtectedDiagnosticWriteRequest,
    pub(in crate::production) operation: LogicalStoreOperationKey,
    pub(in crate::production) work: ReportingWork,
    pub(in crate::production) authentication: Option<ReportingAuthentication>,
}
impl ReportingInput {
    /// Borrow only the exact inert binding from this owned original work, including restoration.
    pub(in crate::production) fn binding(&self) -> ReportBinding {
        self.work.binding()
    }
}
/// Retain the original same-session kernel-authenticated frame and nonclone issued request.
pub(in crate::production) struct ReportingAuthentication {
    pub(in crate::production) received: ReceivedDiagnosticRequest,
    pub(in crate::production) issued: IssuedDiagnosticRequest,
}
/// Report use cases; authenticated dismissal retains the original received and issued loans.
pub(in crate::production) enum ReportingWork {
    /// Load the same current committed summary for a separately admitted presentation worker.
    View { binding: ReportBinding, locale: &'static str, root_capture_degraded: bool },
    /// Persist actual presentation from an original authenticated Presented frame, or failure.
    /// Host-observed native terminal failures need no incoming frame; the host supplies that fact.
    Attempt { binding: ReportBinding, attempt: ReportAttempt },
    /// Own only a genuine retired Symbol observation and its original source/admission charge.
    Symbols { binding: ReportBinding, result: ReportingSymbols },
    /// Validate the actual same-session Action frame before any protected storage effect.
    Acknowledge { command: ReportCommand, at: IncidentUtc },
}
impl ReportingWork {
    /// Return inert exact correlation for namespace and current-summary checks.
    fn binding(&self) -> ReportBinding {
        match self {
            Self::View { binding, .. }
            | Self::Attempt { binding, .. }
            | Self::Symbols { binding, .. } => *binding,
            Self::Acknowledge { command, .. } => command.binding(),
        }
    }
}
/// Preserve original native causes and formatting/allocation failures without replacement.
#[derive(Debug)]
pub(in crate::production) enum ReportingCause {
    Store(IncidentStoreError),
    View(ReportViewError),
    Memory(MemoryClaimError),
    Wire(sugarsubstitute_crash::diagnostic_client::DiagnosticWireError),
    Symbols(SymbolRecordingError),
    Identity,
    Cancelled,
    Deadline,
}
impl std::fmt::Display for ReportingCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Incident report operation failed")
    }
}
impl std::error::Error for ReportingCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(cause) => Some(cause),
            Self::View(cause) => Some(cause),
            Self::Memory(cause) => Some(cause),
            Self::Wire(cause) => Some(cause),
            Self::Symbols(cause) => Some(cause),
            Self::Identity | Self::Cancelled | Self::Deadline => None,
        }
    }
}
impl From<IncidentStoreError> for ReportingCause {
    fn from(cause: IncidentStoreError) -> Self {
        Self::Store(cause)
    }
}
impl From<DiagnosticStorageError> for ReportingCause {
    fn from(cause: DiagnosticStorageError) -> Self {
        Self::Store(cause.into())
    }
}
/// Own every original input, repository, completion product and cause until checked retirement.
pub(in crate::production) struct ReportingOperation {
    input: Option<ReportingInput>,
    repository: Option<IncidentRepository>,
    opening: Option<DiagnosticStoreOpening>,
    loaded: Option<CommittedIncident>,
    view: Option<Box<PreparedViewContent>>,
    view_request: Option<(ReportBinding, &'static str, bool)>,
    original_deadline: Option<Moment>,
    view_last_observed: Option<Moment>,
    attempt: Option<IncidentCommit>,
    symbols: Option<IncidentCommit>,
    acknowledgement: Option<AcknowledgementCommit>,
    first: Option<ReportingCause>,
    failure_transferred: bool,
    retiring_authentication: bool,
    retirement: Option<IncidentStoreError>,
    retired: bool,
}
impl ReportingOperation {
    /// Construct an inert source destination before claiming storage or invoking a provider.
    fn new() -> Self {
        Self {
            input: None,
            repository: None,
            opening: None,
            loaded: None,
            view: None,
            view_request: None,
            original_deadline: None,
            view_last_observed: None,
            attempt: None,
            symbols: None,
            acknowledgement: None,
            first: None,
            failure_transferred: false,
            retiring_authentication: false,
            retirement: None,
            retired: false,
        }
    }
    /// Verify complete pre-effect state before returning a passive original input.
    fn pristine(&self) -> bool {
        self.input.is_none()
            && self.repository.is_none()
            && self.opening.is_none()
            && self.loaded.is_none()
            && self.view.is_none()
            && self.view_request.is_none()
            && self.original_deadline.is_none()
            && self.view_last_observed.is_none()
            && self.attempt.is_none()
            && self.symbols.is_none()
            && self.acknowledgement.is_none()
            && self.first.is_none()
            && self.retirement.is_none()
            && !self.failure_transferred
            && !self.retiring_authentication
            && !self.retired
    }
    /// Keep input installed before effects so unwinding restores the same original custody.
    fn execute(&mut self, input: ReportingInput, context: &JobContext) {
        if self.failure_transferred || self.retiring_authentication {
            return;
        }
        if self.input.is_some() {
            self.first.get_or_insert(ReportingCause::Identity);
            return;
        }
        self.input = Some(input);
        self.original_deadline = context.deadline();
        if let Err(cause) = self.open_and_run(context) {
            self.first.get_or_insert(cause);
        }
    }
    /// Resolve genuine existing protected storage under the actual original Core admission.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn open_and_run(&mut self, context: &JobContext) -> Result<(), ReportingCause> {
        let input = self.input.as_ref().ok_or(ReportingCause::Identity)?;
        if context.spec().reservation_bytes != DIAGNOSTIC_STORAGE_BYTES
            || context.deadline().is_none()
            || input.operation.owner().namespace
                != input.work.binding().incident().subject().coordinator().namespace
        {
            return Err(ReportingCause::Identity);
        }
        authentication::validate(input)?;
        symbols::validate(input, context)?;
        let memory =
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).map_err(ReportingCause::Memory)?;
        let mut storage = DiagnosticStorage::new(memory)?;
        let operation = storage.begin_operation(input.operation, context)?;
        self.repository = Some(IncidentRepository::new(storage, operation, context)?);
        let repository = self.repository.as_mut().ok_or(ReportingCause::Identity)?;
        self.opening = Some(repository.resolve_namespace(&input.request, context)?);
        if !matches!(self.opening, Some(DiagnosticStoreOpening::Existing)) {
            return Err(ReportingCause::Identity);
        }
        repository.acquire_writer(context)?;
        repository.validate_store(context)?;
        self.run(context)
    }
    /// Preserve each authoritative repository result in its distinct owned destination.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn run(&mut self, context: &JobContext) -> Result<(), ReportingCause> {
        let input = self.input.as_ref().ok_or(ReportingCause::Identity)?;
        match &input.work {
            ReportingWork::View { binding, locale, root_capture_degraded } => {
                self.load_current(*binding, locale, *root_capture_degraded, context)?;
            }
            ReportingWork::Attempt { binding, attempt } => {
                self.record_attempt(*binding, *attempt, context)?;
            }
            ReportingWork::Symbols { binding, result } => {
                symbols::record(
                    &mut self.repository,
                    &mut self.symbols,
                    *binding,
                    result.observation(),
                    context,
                )?;
            }
            ReportingWork::Acknowledge { command, at, .. } => {
                self.acknowledge(*command, *at, context)?;
            }
        }
        Ok(())
    }
    /// Retain actual checked metadata activation without changing immutable failure or payload.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn record_attempt(
        &mut self,
        binding: ReportBinding,
        attempt: ReportAttempt,
        context: &JobContext,
    ) -> Result<(), ReportingCause> {
        let repository = self.repository.as_mut().ok_or(ReportingCause::Identity)?;
        self.attempt = Some(repository.record_attempt(binding, attempt, context)?);
        Ok(())
    }
    /// Retain the real acknowledgement commit at the exact supplied command and injected UTC.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn acknowledge(
        &mut self,
        command: ReportCommand,
        at: IncidentUtc,
        context: &JobContext,
    ) -> Result<(), ReportingCause> {
        let repository = self.repository.as_mut().ok_or(ReportingCause::Identity)?;
        self.acknowledgement = Some(repository.acknowledge(command, at, context)?);
        Ok(())
    }
    /// Retain actual validated current summary even when expected binding rejects presentation.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn load_current(
        &mut self,
        binding: ReportBinding,
        locale: &'static str,
        root_capture_degraded: bool,
        context: &JobContext,
    ) -> Result<(), ReportingCause> {
        let repository = self.repository.as_mut().ok_or(ReportingCause::Identity)?;
        self.loaded = Some(repository.load(binding.incident(), context)?);
        let current = self.loaded.as_ref().ok_or(ReportingCause::Identity)?;
        if current.summary().revision() != binding.revision() {
            return Err(ReportingCause::Store(IncidentStoreError::StaleRevision));
        }
        self.view_request = Some((binding, locale, root_capture_degraded));
        self.original_deadline = context.deadline();
        Ok(())
    }
    /// Observe real cancellation/time before allocation and after bounded formatting.
    #[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
    fn check_view_controls(&mut self, context: &JobContext) -> Result<(), ReportingCause> {
        if context.cancellation_requested() {
            return Err(ReportingCause::Cancelled);
        }
        let now = context.now();
        if context.deadline().is_none_or(|until| now >= until)
            || self.view_last_observed.is_some_and(|before| now < before)
        {
            return Err(ReportingCause::Deadline);
        }
        self.view_last_observed = Some(now);
        Ok(())
    }
    /// Transfer the actual original charged view after independent checked file cleanup.
    fn take_view(&mut self) -> Option<Box<PreparedViewContent>> {
        if self.retired && !self.failed() && !self.retiring_authentication {
            self.view.take()
        } else {
            None
        }
    }
    /// Move original authenticated custody only after actual successful mutation and checked close.
    fn take_authentication(&mut self) -> Option<ReportingAuthentication> {
        if !self.retired || self.failed() || self.retiring_authentication {
            return None;
        }
        let input = self.input.as_mut()?;
        let committed = match input.work {
            ReportingWork::Attempt { .. } => self.attempt.is_some(),
            ReportingWork::Acknowledge { .. } => self.acknowledgement.is_some(),
            ReportingWork::View { .. } | ReportingWork::Symbols { .. } => false,
        };
        if committed { input.authentication.take() } else { None }
    }
    /// Borrow the repository's constructor-private checked attempt activation.
    pub(in crate::production) fn attempt(&self) -> Option<&IncidentCommit> {
        self.attempt.as_ref()
    }
    /// Borrow genuine acknowledgement activation; matching wire facts cannot construct it.
    pub(in crate::production) fn acknowledgement(&self) -> Option<&AcknowledgementCommit> {
        self.acknowledgement.as_ref()
    }
    /// Borrow the original cause even after a separate failed cleanup.
    pub(in crate::production) fn failure(&self) -> Option<&ReportingCause> {
        self.first.as_ref()
    }
    /// Preserve unsuccessful work/retirement after its original cause moves to the caller.
    pub(in crate::production) fn failed(&self) -> bool {
        self.first.is_some() || self.retirement.is_some() || self.failure_transferred
    }

    /// Checked-close under the independently admitted first cleanup cutoff; retain first cause.
    fn retire(&mut self, context: &JobContext) -> bool {
        if self.retired {
            return true;
        }
        if let Some(repository) = &mut self.repository
            && let Err(cause) = repository.retire(context)
        {
            self.retirement.get_or_insert(cause);
            return false;
        }
        self.retired = true;
        true
    }
    /// File retirement alone grants no worker-join or source-credit release claim.
    pub(in crate::production) const fn files_retired(&self) -> bool {
        self.retired
    }
}

#[cfg(test)]
pub(in crate::production) use service::tests::{
    continuation_fixture::{acknowledge_fixture, presented_fixture},
    support::original as fixture_original,
};
