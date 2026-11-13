// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the authenticated helper capture input, selected registration and checked retirement.
//! The original job completion retains its four-MiB charge through physical settlement.
//! Rejected input loans stay here until shutdown; the identical admission cause transfers
//! to the caller error owner. Permanent capture remains retained until actual process exit.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey, crash::CaptureTarget, execution::*,
};
use sugarsubstitute_execution::{
    DesktopCaptureAttached, DesktopCaptureSession, DesktopCaptureTemporaryState, ExecutionClient,
    InheritedCaptureAttachInput, ProcessLifetimeCapture,
    windows_capture_attach::{CaptureAttachFailure, CaptureCleanupSnapshot},
};
type Terminal = JobCompletion<DesktopCaptureAttached, CaptureAttachFailure>;
/// Actual inherited input stays borrowed until one exact successful native-owner admission.
pub(super) struct HelperCapture {
    target: CaptureTarget,
    session: Option<DesktopCaptureSession>,
    original: Option<(ExecutionId, ScopeId, JobSpec)>,
    noticed: Option<TerminalKind>,
    terminal: Option<Terminal>,
    rejected: Option<(
        InheritedCaptureAttachInput,
        Box<dyn sugarsubstitute_execution::NativeCaptureDriver>,
    )>,
    permanent: Option<ProcessLifetimeCapture>,
    released: Option<Box<dyn sugarsubstitute_execution::NativeCaptureDriver>>,
    closed: bool,
}
#[derive(Debug)]
enum CaptureIssue {
    Identity,
    Cancelled,
    TimedOut,
    Coalesced(JobId),
    Panicked(PanicCause),
}
impl fmt::Display for CaptureIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => f.write_str("Actual helper capture identity or state rejected"),
            Self::Cancelled => f.write_str("Actual helper capture cancelled"),
            Self::TimedOut => f.write_str("Original helper capture cutoff expired"),
            Self::Coalesced(job) => write!(f, "Actual helper capture replaced by {job:?}"),
            Self::Panicked(cause) => {
                write!(f, "Actual helper capture panicked: {}", cause.as_str())
            }
        }
    }
}
impl Error for CaptureIssue {}
impl HelperCapture {
    /// Retain the genuine helper key without selecting a driver or acquiring native resources.
    pub(super) const fn new(key: BackendCustodyKey) -> Self {
        Self {
            target: CaptureTarget::BackendCustody(key),
            session: None,
            original: None,
            noticed: None,
            terminal: None,
            rejected: None,
            permanent: None,
            released: None,
            closed: false,
        }
    }
    /// No body, handler or native input is reconstructed; only the authenticated producer can supply it.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        input: &mut Option<InheritedCaptureAttachInput>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed || self.session.is_some() || self.rejected.is_some() {
            return Err(Box::new(CaptureIssue::Identity));
        }
        let actual = input.as_ref().ok_or("Authenticated helper capture input missing")?;
        if actual.target() != self.target {
            return Err(Box::new(CaptureIssue::Identity));
        }
        let spec = JobSpec {
            id: JobId::new(3)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(actual.original_deadline()),
            reservation_bytes: 4_194_304,
            replacement: None,
        };
        let driver = sugarsubstitute_crash::desktop_capture_driver()?;
        let actual = input.take().ok_or("Authenticated helper capture input missing")?;
        match client.reserve_desktop_capture_attachment(scope, spec, actual, driver) {
            Ok(session) => {
                self.original = Some((scope.owner_id(), scope.id(), spec));
                self.session = Some(session);
                Ok(())
            }
            Err(rejected) => {
                let (error, input, driver) = rejected.into_parts();
                self.rejected = Some((input, driver));
                Err(Box::new(error))
            }
        }
    }
    /// Report whether the original native owner accepted this source, independently of Ready.
    pub(super) fn admitted(&self) -> bool {
        self.session.is_some()
    }
    /// Read only the original capture owner's cached first cleanup transaction.
    pub(super) fn cleanup_snapshot(&self) -> Option<CaptureCleanupSnapshot> {
        self.session.as_ref().map(|session| session.control().cleanup_snapshot())
    }
    /// Match the original actual notification and typed completion before observing registration.
    pub(super) fn observe(&mut self, batch: &CompletionBatch) -> Result<(), Box<dyn Error>> {
        let Some((execution, scope, spec)) = self.original else {
            return Ok(());
        };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (scope, spec.id) {
                continue;
            }
            if notice.replacement.is_some()
                || self.noticed.is_some_and(|first| first != notice.kind)
            {
                return Err(Box::new(CaptureIssue::Identity));
            }
            self.noticed = Some(notice.kind);
        }
        if self.terminal.is_none() && self.noticed.is_some() {
            let Some(session) = &mut self.session else {
                return Err(Box::new(CaptureIssue::Identity));
            };
            if let Some(done) = session.job().try_take_completion() {
                let valid = done.scope_id == scope
                    && done.spec == spec
                    && Some(done.outcome.kind()) == self.noticed
                    && done.memory.bytes() == 4_194_304
                    && done.native_handles.is_none()
                    && match &done.outcome {
                        JobOutcome::Success(attached) => {
                            attached.target() == self.target
                                && attached.execution() == execution
                                && attached.job() == spec.id
                        }
                        _ => true,
                    };
                self.terminal = Some(done);
                if !valid {
                    return Err(Box::new(CaptureIssue::Identity));
                }
            }
        }
        if !self.closed
            && let Some(done) = &self.terminal
        {
            match &done.outcome {
                JobOutcome::Success(_) => {}
                JobOutcome::Failure(cause) => return Err(Box::new(cause.clone())),
                JobOutcome::Cancelled => return Err(Box::new(CaptureIssue::Cancelled)),
                JobOutcome::TimedOut => return Err(Box::new(CaptureIssue::TimedOut)),
                JobOutcome::Coalesced { replacement } => {
                    return Err(Box::new(CaptureIssue::Coalesced(*replacement)));
                }
                JobOutcome::Panicked(cause) => {
                    return Err(Box::new(CaptureIssue::Panicked(cause.clone())));
                }
            }
        }
        Ok(())
    }
    /// Source construction is permitted only by the original successful registration before cleanup.
    pub(super) fn ready(&self) -> bool {
        !self.closed
            && self
                .terminal
                .as_ref()
                .is_some_and(|done| matches!(done.outcome, JobOutcome::Success(_)))
            && self.session.as_ref().is_some_and(|session| {
                matches!(session.control().cleanup_snapshot(), CaptureCleanupSnapshot::Dormant)
            })
    }
    /// Request the same original cleanup; unissued map loans return passively before pipe closure.
    pub(super) fn request_cleanup(&mut self) {
        self.closed = true;
        if let Some(session) = &self.session {
            session.control().request_cleanup();
        }
        if let Some((input, driver)) = self.rejected.take() {
            drop(input);
            self.released = Some(driver);
        }
    }
    /// Retain permanent capture explicitly; only actual process exit ends its registered native lifetime.
    pub(super) fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if self.permanent.is_none()
            && let Some(value) = self
                .session
                .as_ref()
                .and_then(|session| session.control().try_take_process_lifetime())
        {
            self.permanent = Some(value?);
        }
        Ok(match self.session.as_ref().map(|session| session.control().temporary_settled()) {
            None => true,
            Some(
                DesktopCaptureTemporaryState::UnissuedReturned
                | DesktopCaptureTemporaryState::Retired,
            ) => true,
            Some(DesktopCaptureTemporaryState::Transferred) => self.permanent.is_some(),
            Some(DesktopCaptureTemporaryState::Pending) => false,
        })
    }
}
