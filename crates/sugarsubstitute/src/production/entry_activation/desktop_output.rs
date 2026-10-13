// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep genuine desktop stdio, terminal guards and closed phase diagnostics owned together.
#[path = "desktop_output/diagnostic.rs"]
mod diagnostic;
use crate::job_sequence::JobSequence;
use std::error::Error;
use sugarsubstitute_contracts::execution::Scope;
use sugarsubstitute_contracts::{
    execution::{JobClass, JobCompletion, JobOutcome, JobSpec, Operation, TerminalKind},
    lifecycle::RunKey,
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_execution::{
    windows::NativeObservationError,
    windows_output::{
        AdmittedOutputBatch, MIN_WINDOWS_OUTPUT_BYTES, OutputIdentity, WindowsChildStdio,
        WindowsOutputError, WindowsOutputReport, WindowsOutputSession,
    },
};
use sugarsubstitute_supervision::{
    FamilyBinding, RegistryInputError, RejectedOutputLaunch, WindowsLaunchInput,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Stored inside the projection-funded DesktopStart before any launch; the
/// original native output job owns its unchanged 1,310,720-byte reservation.
pub(super) struct DesktopOutput {
    session: WindowsOutputSession,
    terminal: Option<JobCompletion<WindowsOutputReport, NativeObservationError>>,
    loan: Option<AdmittedOutputBatch>,
    rejected_input: Option<WindowsLaunchInput>,
    rejected_stdio: Option<WindowsChildStdio>,
    issue: Option<Issue>,
    notified: bool,
    closing: bool,
    run: RunKey,
    diagnostic: diagnostic::Diagnostic,
}
#[derive(Debug)]
enum Issue {
    Input(RegistryInputError),
    Output(WindowsOutputError),
    Identity,
    Stopped(TerminalKind),
}
impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped(kind) => write!(f, "Desktop output stopped: {kind:?}"),
            _ => f.write_str("Desktop output owner failed"),
        }
    }
}
impl Error for Issue {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(cause) => Some(cause),
            Self::Output(cause) => Some(cause),
            _ => None,
        }
    }
}
impl DesktopOutput {
    /// Reserve through the sole central native owner before pipe effects or launch.
    pub(super) fn new(
        run: RunKey,
        client: &ExecutionClient,
        native: &Scope,
        jobs: &JobSequence,
    ) -> Result<Self> {
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
            replacement: None,
        };
        let session = client.reserve_windows_output(native, spec, OutputIdentity::Desktop(run))?;
        Ok(Self {
            session,
            terminal: None,
            loan: None,
            rejected_input: None,
            rejected_stdio: None,
            issue: None,
            notified: false,
            closing: false,
            run,
            diagnostic: diagnostic::Diagnostic::new(),
        })
    }
    pub(super) fn prepared(&self) -> bool {
        let actual = self.session.control.snapshot();
        actual.identity == OutputIdentity::Desktop(self.run)
            && actual.prepared
            && self.terminal.is_none()
            && !self.closing
            && self.issue.is_none()
    }
    /// Preserve both actual rejected pins until explicit unused-launch retirement.
    pub(super) fn attach(&mut self, input: WindowsLaunchInput) -> Result<WindowsLaunchInput> {
        let stdio = match self.session.control.take_child_stdio() {
            Ok(stdio) => stdio,
            Err(cause) => {
                self.rejected_input = Some(input);
                self.issue.get_or_insert(Issue::Output(cause));
                return Err("Desktop stdio preparation failed with original input retained".into());
            }
        };
        match input.with_owned_output(
            FamilyBinding::Desktop(self.run),
            OutputIdentity::Desktop(self.run),
            &self.session.control,
            stdio,
        ) {
            Ok(input) => Ok(input),
            Err(RejectedOutputLaunch { input, stdio, error }) => {
                self.rejected_input = Some(*input);
                self.rejected_stdio = Some(stdio);
                self.issue.get_or_insert(Issue::Input(error));
                Err("Desktop output association rejected with original inputs retained".into())
            }
        }
    }
    /// Observe bounded passive bytes and the exact original native completion.
    /// Only closed typed diagnostic fields reach tracing; payload bytes never do.
    pub(super) fn observe(&mut self) -> Result<()> {
        if self.terminal.is_none() {
            self.terminal = self.session.completion.try_take_completion();
        }
        if let Some(terminal) = &self.terminal {
            if terminal.scope_id != self.session.completion.scope_id()
                || terminal.spec != self.session.completion.spec()
            {
                self.issue.get_or_insert(Issue::Identity);
            } else if let Some(issue) = terminal_issue(terminal.outcome.kind(), self.closing) {
                self.issue.get_or_insert(issue);
            }
        }
        let drained = self.drain();
        if let Err(cause) = drained {
            self.issue.get_or_insert(Issue::Output(cause));
        }
        if !self.notified && self.failure().is_some() {
            self.notified = true;
            return Err("Desktop output failed with its original retained source".into());
        }
        Ok(())
    }
    fn drain(&mut self) -> std::result::Result<(), WindowsOutputError> {
        for _ in 0..8 {
            if self.loan.is_none() {
                self.loan = match self.session.control.take_batch() {
                    Err(WindowsOutputError::NotReady) => return Ok(()),
                    other => other?,
                };
            }
            let Some(guard) = &self.loan else { break };
            let actual = guard.desktop_batch().ok_or(WindowsOutputError::ForeignIdentity)?;
            if actual.run() != self.run {
                return Err(WindowsOutputError::ForeignIdentity);
            }
            self.diagnostic.feed(actual.stream(),actual.sequence().get(),actual.gap().is_some(),actual.end(),actual.bytes(),|phase| {
                tracing::info!(operation="desktop_child_preparation",run=?self.run,
                    capture_admitted=phase.capture_admitted,capture=phase.capture,registered=phase.registered,
                    configuration=phase.configuration,original_failure=?phase.original_failure,
                    capture_failure=%phase.capture_failure,
                    failure_retained=phase.failure_retained,"Observed owned child's closed preparation diagnostic");
            });
            // Return the sole real delivery slot before taking another batch.
            self.loan = None;
        }
        Ok(())
    }
    /// Request cleanup of this actual owner; never infer EOF or native retirement.
    pub(super) fn request_close(&mut self) {
        self.closing = true;
        drop(self.rejected_input.take());
        drop(self.rejected_stdio.take());
        drop(self.loan.take());
        self.session.control.close();
    }
    /// Exact terminal correlation plus checked retirement from the same retained
    /// control and returned last delivery loan; family/profile settlement is separate.
    pub(super) fn settled(&self) -> bool {
        let actual = self.session.control.snapshot();
        self.loan.is_none()
            && self.rejected_stdio.is_none()
            && self.rejected_input.is_none()
            && actual.identity == OutputIdentity::Desktop(self.run)
            && actual.retired
            && actual.queued_chunks == 0
            && actual.pending_loss == [0, 0]
            && self.terminal.as_ref().is_some_and(|terminal| {
                terminal.scope_id == self.session.completion.scope_id()
                    && terminal.spec == self.session.completion.spec()
            })
    }
    /// Borrow original native/input causes while all terminal storage remains owned.
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.terminal
            .as_ref()
            .and_then(|terminal| match &terminal.outcome {
                JobOutcome::Failure(cause) => Some(cause as &(dyn Error + 'static)),
                _ => None,
            })
            .or_else(|| self.issue.as_ref().map(|issue| issue as &(dyn Error + 'static)))
    }
}

// Successful stream completion is passive EOF/retirement evidence. Only the
// Parent and actual registry classify desktop exit and application shutdown.
fn terminal_issue(kind: TerminalKind, closing: bool) -> Option<Issue> {
    match kind {
        TerminalKind::Success => None,
        TerminalKind::Cancelled if closing => None,
        other => Some(Issue::Stopped(other)),
    }
}
#[cfg(test)]
#[path = "desktop_output/ownership_tests.rs"]
mod ownership_tests;
