// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual child capture admission, registration and independent native custody.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CompletionBatch, JobClass, JobCompletion, JobId, JobOutcome, JobSpec,
    Operation, Scope,
};
use sugarsubstitute_execution::{
    DesktopCaptureAdmissionRejection, DesktopCaptureAttached, DesktopCaptureSession,
    ExecutionClient, NativeCaptureDriver, ProcessLifetimeCapture,
    windows_capture_attach::{CaptureAttachFailure, CaptureCleanupSnapshot},
    windows_lifecycle_pipe::{AuthenticatedStartupSnapshot, CapturedStartup},
};
#[path = "capture/correlation.rs"]
mod correlation;
use correlation::CaptureCorrelation;

type Terminal = JobCompletion<DesktopCaptureAttached, CaptureAttachFailure>;

/// Reject inconsistent application observations without replacing retained native causes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChildCaptureError {
    /// The one-shot owner already started or has closed admission.
    State,
    /// Original completion scope, specification, kind or output association differs.
    Identity,
}
impl fmt::Display for ChildCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Child capture observation rejected: {self:?}")
    }
}
impl Error for ChildCaptureError {}

/// Borrow retained original rejection custody or return an untransferred new pair.
pub(super) enum ChildCaptureStartRejection<'a> {
    /// Preserve the new untransferred pair while the prior owner remains unchanged.
    State(CapturedStartup),
    /// Preserve actual admission cause, input and inert driver together with its snapshot.
    Admission {
        rejected: &'a DesktopCaptureAdmissionRejection,
        startup: &'a AuthenticatedStartupSnapshot,
    },
}
impl ChildCaptureStartRejection<'_> {
    /// Borrow the identical authenticated snapshot; no new configuration is constructed.
    pub(super) fn startup(&self) -> &AuthenticatedStartupSnapshot {
        match self {
            Self::State(input) => &input.startup,
            Self::Admission { startup, .. } => startup,
        }
    }
}
impl fmt::Debug for ChildCaptureStartRejection<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChildCaptureStartRejection")
            .field("startup_available", &self.startup().view().is_ok())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ChildCaptureStartRejection<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(_) => ChildCaptureError::State.fmt(formatter),
            Self::Admission { rejected, .. } => rejected.fmt(formatter),
        }
    }
}
impl Error for ChildCaptureStartRejection<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(_) => Some(&ChildCaptureError::State),
            Self::Admission { rejected, .. } => Some(&rejected.error),
        }
    }
}

struct RejectedStartup {
    rejected: DesktopCaptureAdmissionRejection,
    startup: AuthenticatedStartupSnapshot,
}

/// Own the actual admitted child job; its composing parent retains the original Scope.
/// Registration, cleanup and permanent process handoff remain distinct observations.
pub(super) struct ChildCapture {
    driver: Option<Box<dyn NativeCaptureDriver>>,
    startup: Option<AuthenticatedStartupSnapshot>,
    rejected: Option<RejectedStartup>,
    released_rejection: Option<AdmissionError>,
    session: Option<DesktopCaptureSession>,
    correlation: Option<CaptureCorrelation>,
    terminal: Option<Terminal>,
    closed: bool,
    invalid: bool,
}
impl ChildCapture {
    /// Construct inertly: no clock, input, bundle selection or driver callback runs.
    pub(super) fn new(driver: Box<dyn NativeCaptureDriver>) -> Self {
        Self {
            driver: Some(driver),
            startup: None,
            rejected: None,
            released_rejection: None,
            session: None,
            correlation: None,
            terminal: None,
            closed: false,
            invalid: false,
        }
    }

    /// Transfer the genuine authenticated pair through exact original four-MiB admission.
    /// Admission rejection keeps every original input in this owner; its borrowed
    /// error exposes the genuine cause without allocating another shared control.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        job: JobId,
        input: CapturedStartup,
    ) -> Result<(), ChildCaptureStartRejection<'_>> {
        if self.closed || self.session.is_some() || self.driver.is_none() {
            return Err(ChildCaptureStartRejection::State(input));
        }
        let spec = JobSpec {
            id: job,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(input.capture.original_deadline()),
            reservation_bytes: 4_194_304,
            replacement: None,
        };
        let run = input.capture.run();
        let Some(driver) = self.driver.take() else {
            return Err(ChildCaptureStartRejection::State(input));
        };
        let CapturedStartup { capture, startup } = input;
        match client.reserve_desktop_capture_attachment(scope, spec, capture, driver) {
            Ok(session) => {
                self.startup = Some(startup);
                self.correlation =
                    Some(CaptureCorrelation::new(run, scope.owner_id(), scope.id(), spec));
                self.session = Some(session);
                Ok(())
            }
            Err(rejected) => {
                let retained = self.rejected.insert(RejectedStartup { rejected, startup });
                Err(ChildCaptureStartRejection::Admission {
                    rejected: &retained.rejected,
                    startup: &retained.startup,
                })
            }
        }
    }

    /// Match immutable original metadata before transferring its real typed completion.
    /// Full completion and original guards remain retained even when identity checks fail.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), ChildCaptureError> {
        let Some(correlation) = &mut self.correlation else { return Ok(()) };
        for notice in &batch.notifications {
            if let Err(cause) = correlation.notice(*notice) {
                self.invalid = true;
                return Err(cause);
            }
        }
        if self.terminal.is_none() && correlation.noticed() {
            let Some(session) = &mut self.session else {
                self.invalid = true;
                return Err(ChildCaptureError::State);
            };
            if let Some(completion) = session.job().try_take_completion() {
                let matches = correlation.completion(&completion)
                    && match &completion.outcome {
                        JobOutcome::Success(attached) => correlation.attached(
                            attached.run(),
                            attached.execution(),
                            attached.job(),
                        ),
                        _ => true,
                    };
                self.terminal = Some(completion);
                if !matches {
                    self.invalid = true;
                    return Err(ChildCaptureError::Identity);
                }
            }
        }
        Ok(())
    }

    /// Observe actual registration while the same native source has not entered cleanup.
    /// This grants no native effect or temporary/permanent retirement permission.
    pub(super) fn ready(&self) -> bool {
        !self.closed
            && !self.invalid
            && self
                .terminal
                .as_ref()
                .is_some_and(|terminal| matches!(terminal.outcome, JobOutcome::Success(_)))
            && self.session.as_ref().is_some_and(|session| {
                matches!(session.control().cleanup_snapshot(), CaptureCleanupSnapshot::Dormant)
            })
    }

    /// Borrow original authenticated Launch bytes for the actual configuration projection.
    #[cfg(test)]
    pub(super) fn startup(&self) -> Option<&AuthenticatedStartupSnapshot> {
        self.startup.as_ref().or_else(|| self.rejected.as_ref().map(|retained| &retained.startup))
    }

    /// Transfer the unchanged authenticated loan once after actual capture readiness.
    /// Root retains it through configuration projection; transfer changes no native source state.
    pub(super) fn take_startup(&mut self) -> Option<AuthenticatedStartupSnapshot> {
        if self.ready() { self.startup.take() } else { None }
    }

    /// Borrow complete original failure/success and its actual retained accounting guards.
    pub(super) const fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Observe an actual admitted session, including closed or failed sessions.
    pub(super) fn admitted(&self) -> bool {
        self.session.is_some()
    }

    /// Observe only the actual admitted capture control's passive temporary disposition.
    /// None means no session was admitted; it grants no physical native closure.
    pub(super) fn temporary_settled(
        &self,
    ) -> Option<sugarsubstitute_execution::DesktopCaptureTemporaryState> {
        self.session.as_ref().map(|session| session.control().temporary_settled())
    }

    /// Borrow actual admission cause and complete unissued input/driver custody.
    pub(super) fn admission_rejection(&self) -> Option<&DesktopCaptureAdmissionRejection> {
        self.rejected.as_ref().map(|retained| &retained.rejected)
    }

    /// Borrow the original admission cause even after its unissued source returns.
    pub(super) fn admission_failure(&self) -> Option<&AdmissionError> {
        self.admission_rejection()
            .map(|rejected| &rejected.error)
            .or(self.released_rejection.as_ref())
    }

    /// Return unused startup bytes after native UI retirement and settled projection.
    /// Rejected admission created no job or native effect. Returning its original
    /// input releases its map retention without declaring native retirement.
    /// Its actual admission cause and inert driver remain owned here.
    pub(super) fn release_startup_source(&mut self) {
        self.startup = None;
        if let Some(rejected) = self.rejected.take() {
            let (cause, input, driver) = rejected.rejected.into_parts();
            drop(rejected.startup);
            drop(input);
            self.driver = Some(driver);
            self.released_rejection = Some(cause);
        }
    }

    /// Invalidate local readiness and request the same original ten/fifteen-second transaction.
    /// Repeated requests reuse the execution owner's first actual budget and cause.
    pub(super) fn request_cleanup(&mut self) -> Option<CaptureCleanupSnapshot> {
        self.closed = true;
        self.session.as_ref().map(|session| session.control().request_cleanup())
    }

    /// Observe existing cleanup facts without sampling a provider or creating a budget.
    pub(super) fn cleanup_snapshot(&self) -> Option<CaptureCleanupSnapshot> {
        self.session.as_ref().map(|session| session.control().cleanup_snapshot())
    }

    /// Transfer only the actual separately published permanent capsule, once.
    /// Neither terminal success nor a cleanup request manufactures this handoff.
    pub(super) fn try_take_process_lifetime(
        &self,
    ) -> Option<Result<ProcessLifetimeCapture, CaptureAttachFailure>> {
        self.session.as_ref().and_then(|session| session.control().try_take_process_lifetime())
    }
}

#[cfg(test)]
#[path = "capture/tests.rs"]
mod tests;
