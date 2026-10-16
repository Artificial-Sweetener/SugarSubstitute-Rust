// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer original failure custody only after actual family and same-session pipe retirement.
use super::*;
use std::error::Error;
use sugarsubstitute_supervision::{FamilyBinding, RegistryAdmissionError, RegistryInputError};
/// Exact original Parent failure, including the whole failed native completion and its charge.
pub(in crate::production) enum ReporterParentFailure {
    Native(JobCompletion<DiagnosticPipeReport, LifecyclePipeFailure>),
    Wire(DiagnosticWireError),
    Launch(RegistryAdmissionError),
    Input(RegistryInputError),
    Send(DiagnosticSendError),
    Transport(LifecyclePipeError),
}
pub(super) enum OriginalCause {
    Parent(ReporterParentFailure),
    Admission(AdmissionError),
    Wire(DiagnosticWireError),
    Time(time::ReporterTimeCause),
    Source(ReporterSourceIssue),
    Worker(JobCompletion<worker::RelayTurn, ReporterSourceIssue>),
}
/// A terminal admission has no worker; a failed worker has no terminal admission.
pub(super) enum ReporterSecondaryFailure {
    Worker { _completion: JobCompletion<worker::RelayTurn, ReporterSourceIssue> },
    Admission { _cause: AdmissionError },
}
/// Nonclone original cause and source lease remain owned outside all metadata locks.
pub(in crate::production) struct OwnedReporterFailure {
    pub(super) cause: OriginalCause,
    pub(super) _secondary: Option<ReporterSecondaryFailure>,
    pub(super) _admission: Option<AdmissionError>,
    pub(super) _source: ReporterSource,
}
impl std::fmt::Debug for OwnedReporterFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::fmt::Display for OwnedReporterFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter retained its original checked failure")
    }
}
impl Error for OwnedReporterFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            OriginalCause::Parent(ReporterParentFailure::Native(done)) => match &done.outcome {
                JobOutcome::Failure(cause) => Some(cause),
                _ => None,
            },
            OriginalCause::Parent(ReporterParentFailure::Wire(cause))
            | OriginalCause::Wire(cause) => Some(cause),
            OriginalCause::Parent(ReporterParentFailure::Launch(cause)) => Some(cause),
            OriginalCause::Parent(ReporterParentFailure::Input(cause)) => Some(cause),
            OriginalCause::Parent(ReporterParentFailure::Send(cause)) => Some(cause),
            OriginalCause::Parent(ReporterParentFailure::Transport(cause)) => Some(cause),
            OriginalCause::Time(cause) => Some(cause),
            OriginalCause::Admission(cause) => Some(cause),
            OriginalCause::Source(cause) => Some(cause),
            OriginalCause::Worker(done) => match &done.outcome {
                JobOutcome::Failure(cause) => Some(cause),
                _ => None,
            },
        }
    }
}
impl ReporterSource {
    pub(super) fn take_retired_failure(
        &self,
        registry: &WindowsProcessRegistry,
    ) -> Result<Option<OwnedReporterFailure>, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if !state.closing {
            return Err(ReporterSourceIssue::Identity);
        }
        if state.failure_transferred {
            return Ok(None);
        }
        let parent = state.parent.as_mut().ok_or(ReporterSourceIssue::Busy)?;
        if !parent.pipe_settled() {
            return Err(ReporterSourceIssue::Busy);
        }
        if let Some(process) = parent.process() {
            let actual = registry.snapshot(process).map_err(|cause| {
                state.registry_cause.get_or_insert(cause);
                state.first.get_or_insert(ReporterSourceIssue::Parent);
                ReporterSourceIssue::Parent
            })?;
            if actual.binding != FamilyBinding::Diagnostic(parent.key()) || !actual.settled {
                return Err(ReporterSourceIssue::Identity);
            }
        }
        let cause = match state.first {
            Some(ReporterSourceIssue::Clock) => state.time_cause.take().map(OriginalCause::Time),
            Some(ReporterSourceIssue::Wire) => state.wire.take().map(OriginalCause::Wire),
            Some(ReporterSourceIssue::Parent) | None => {
                parent.take_retired_failure().map(OriginalCause::Parent)
            }
            Some(cause) => Some(OriginalCause::Source(cause)),
        };
        let Some(cause) = cause else { return Ok(None) };
        state.failure_transferred = true;
        Ok(Some(OwnedReporterFailure {
            cause,
            _secondary: None,
            _admission: None,
            _source: self.alias(),
        }))
    }
}
