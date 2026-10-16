// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the original Finish exchange and checked helper retirement sequence.
mod native;
use super::*;
use sugarsubstitute_contracts::execution::{JobCompletion, JobOutcome, TerminalKind};
use sugarsubstitute_execution::{
    backend_custody::BackendCustodyPipeReport, windows_lifecycle_pipe::LifecyclePipeFailure,
};
use sugarsubstitute_supervision::BackendCustodyFinishStatus;
type Terminal = JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>;
/// Borrow real native adapters; returned facts do not grant fresh admission.
trait Boundary {
    /// Observe the existing transport without admitting Finish while profile owners remain.
    fn observe(&mut self) -> Result<()>;
    /// Consume the actual original Finished fact; pending prior responses keep this false.
    fn finish(&mut self, until: Moment) -> Result<bool>;
    /// Borrow the actual original nonce; unavailable state cannot imply never issued.
    fn finish_status(&self) -> BackendCustodyFinishStatus;
    /// Begin and advance the exact registered helper family with its immutable budget.
    fn retire_family(&mut self, budget: ShutdownBudget) -> Result<bool>;
    /// Close the original pipe only after its helper family has actually retired.
    fn close_pipe(&mut self) -> Result<()>;
    /// Transfer the original typed completion after callbacks, handles and loans retire.
    fn take_terminal(&mut self) -> Result<Option<Terminal>>;
    /// Release the captured attachment only after original native retirement evidence.
    fn parent_retired(&mut self);
}
/// Retain the original shutdown fields until actual family and pipe retirement.
#[derive(Default)]
pub(super) struct Retirement {
    pub(super) original: Option<ShutdownBudget>,
    finish_received: bool,
    terminal: Option<Terminal>,
    pipe_closed: bool,
    pub(super) settled: bool,
    grace_observed: bool,
    total_observed: bool,
}
impl Retirement {
    /// Adopt the existing pipe's actual terminal independently of fresh admission.
    /// Its original guard remains here until full family and transport retirement.
    pub(super) fn observe_terminal(&mut self, parent: &BackendCustodyParent) -> Result<()> {
        if self.terminal.is_none() {
            self.terminal = parent
                .take_terminal()
                .map_err(|cause| diagnostics::failed("pipe_terminal", cause))?;
        }
        let Some(terminal) = &self.terminal else { return Ok(()) };
        match &terminal.outcome {
            JobOutcome::Failure(cause) => Err(Box::new(cause.clone())),
            JobOutcome::Panicked(cause) => Err(Box::new(RetirementPanicked(cause.clone()))),
            outcome => Err(Box::new(RetirementRejected::Terminal(outcome.kind()))),
        }
    }
    /// Freeze one unchanged original transaction, including pre-parent attachment cleanup.
    fn freeze(&mut self, budget: ShutdownBudget) -> Result<ShutdownBudget> {
        let original = *self.original.get_or_insert(budget);
        if original != budget {
            return Err("Custody original cleanup changed".into());
        }
        Ok(original)
    }
    /// Advance the complete retirement policy through the actual external effect boundary.
    fn advance(
        &mut self,
        budget: ShutdownBudget,
        ready: bool,
        profiles_retired: bool,
        now: Moment,
        key: BackendCustodyKey,
        port: &mut impl Boundary,
    ) -> Result<()> {
        let original = self.freeze(budget)?;
        if self.settled {
            return self.terminal_result(ready, key);
        }
        self.grace_observed |= now >= original.graceful_until;
        self.total_observed |= now >= original.settle_until;
        let mut failure = None;
        if ready && !self.finish_received && now < original.settle_until {
            let observed = if profiles_retired {
                port.finish(original.settle_until)
            } else {
                port.observe().map(|()| false)
            };
            match observed {
                Ok(finished) => {
                    self.finish_received = finished;
                    if finished {
                        tracing::info!(operation="backend_custody_finished", key=?key, settle_until=?original.settle_until, "Authenticated backend custody Finished receipt consumed");
                    }
                }
                Err(cause) => failure = Some(cause),
            }
        }
        if ready && !self.finish_received && now >= original.settle_until {
            failure = Some(Box::new(RetirementRejected::FinishExpired {
                until: original.settle_until,
                profiles_retired,
                status: port.finish_status(),
            }));
        }
        if ready && !self.finish_received && failure.is_none() {
            return Ok(());
        }
        let cleanup = (|| -> Result<()> {
            if port.retire_family(original)? {
                if !self.pipe_closed {
                    port.close_pipe()?;
                    self.pipe_closed = true;
                }
                if self.terminal.is_none() {
                    self.terminal = port.take_terminal()?;
                }
                if let Some(terminal) = &self.terminal {
                    port.parent_retired();
                    self.settled = true;
                    let native_failure = match &terminal.outcome {
                        JobOutcome::Failure(cause) => Some(cause.native_cause()),
                        _ => None,
                    };
                    tracing::info!(operation="backend_custody_retired", key=?key, finish_received=self.finish_received,
                    terminal_kind=?terminal.outcome.kind(), native_stage=native_failure.map(sugarsubstitute_execution::windows::NativeObservationError::stage),
                    native_win32_code=native_failure.and_then(sugarsubstitute_execution::windows::NativeObservationError::win32_code), terminal_job=?terminal.spec.id, terminal_scope=?terminal.scope_id,
                    "Backend custody family and transport retired");
                }
            }
            Ok(())
        })();
        if let Err(cause) = cleanup {
            return Err(diagnostics::preserve_cleanup(failure, cause));
        }
        match failure {
            Some(cause) => Err(cause),
            None if self.settled => self.terminal_result(ready, key),
            None => Ok(()),
        }
    }
    /// Preserve the retained terminal outcome independently of physical closure.
    fn terminal_result(&self, ready: bool, key: BackendCustodyKey) -> Result<()> {
        let Some(terminal) = &self.terminal else { return Ok(()) };
        inspect_terminal(
            &terminal.outcome,
            key,
            ready,
            self.finish_received,
            self.pipe_closed,
            self.settled,
        )
    }
    /// Preserve the existing original graceful and total wake observations.
    pub(super) fn deadlines(&self) -> [Option<Moment>; 2] {
        match self.original.filter(|_| !self.settled && !self.total_observed) {
            Some(budget) => {
                [(!self.grace_observed).then_some(budget.graceful_until), Some(budget.settle_until)]
            }
            None => [None, None],
        }
    }
}
impl CoordinatorCustody {
    /// Finish the authenticated witness, observe its real exit, then close the actual pipe.
    pub(crate) fn close(
        &mut self,
        budget: ShutdownBudget,
        profiles_retired: bool,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        self.retirement.freeze(budget)?;
        let Some(parent) = self.parent.clone() else {
            self.retirement.settled = self.attachment.close(resources)?;
            return Ok(());
        };
        let now = resources.client.now()?;
        let key = parent.key();
        let mut port = native::Native {
            parent,
            ticket: &mut self.ticket,
            attachment: &mut self.attachment,
            resources,
        };
        self.retirement.advance(budget, self.ready, profiles_retired, now, key, &mut port)
    }
}

/// Classify the actual external terminal without extracting or discarding its original owner.
fn inspect_terminal<E>(
    outcome: &JobOutcome<sugarsubstitute_execution::backend_custody::BackendCustodyPipeReport, E>,
    key: BackendCustodyKey,
    finish_required: bool,
    finish_received: bool,
    close_requested: bool,
    family_retired: bool,
) -> Result<()>
where
    E: std::error::Error + Send + Sync + Clone + 'static,
{
    if let JobOutcome::Failure(cause) = outcome {
        return Err(Box::new(cause.clone()));
    }
    if !family_retired {
        return Err(Box::new(RetirementRejected::FamilyPending));
    }
    if let JobOutcome::Panicked(cause) = outcome {
        return Err(Box::new(RetirementPanicked(cause.clone())));
    }
    if matches!(outcome, JobOutcome::TimedOut | JobOutcome::Coalesced { .. }) {
        return Err(Box::new(RetirementRejected::Terminal(outcome.kind())));
    }
    if finish_required && !finish_received {
        return Err(Box::new(RetirementRejected::MissingFinished));
    }
    match outcome {
        JobOutcome::Success(report) if report.key == Some(key) => Ok(()),
        JobOutcome::Success(_) => Err(Box::new(RetirementRejected::ForeignKey)),
        JobOutcome::Cancelled if close_requested => Ok(()),
        _ => Err(Box::new(RetirementRejected::Terminal(outcome.kind()))),
    }
}
#[derive(Debug)]
enum RetirementRejected {
    MissingFinished,
    FinishExpired { until: Moment, profiles_retired: bool, status: BackendCustodyFinishStatus },
    ForeignKey,
    FamilyPending,
    Terminal(TerminalKind),
}
impl std::fmt::Display for RetirementRejected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFinished => {
                formatter.write_str("Original helper Finished acknowledgement is missing")
            }
            Self::FinishExpired { until, profiles_retired, status } => match status {
                BackendCustodyFinishStatus::Unissued if !profiles_retired => write!(
                    formatter,
                    "Original helper Finish not issued: profile retirement pending at cutoff {until:?}"
                ),
                BackendCustodyFinishStatus::Unissued => {
                    write!(formatter, "Original helper Finish not issued before cutoff {until:?}")
                }
                BackendCustodyFinishStatus::Issued(nonce) => write!(
                    formatter,
                    "Original helper Finish nonce {nonce} queued; Finished acknowledgement absent at cutoff {until:?}"
                ),
                BackendCustodyFinishStatus::Unavailable => write!(
                    formatter,
                    "Original helper Finish state unavailable at cutoff {until:?}"
                ),
            },
            Self::ForeignKey => formatter.write_str("Original custody terminal has a foreign key"),
            Self::FamilyPending => formatter.write_str("Original helper family remains owned"),
            Self::Terminal(kind) => {
                write!(formatter, "Original custody terminal rejected: {kind:?}")
            }
        }
    }
}
impl std::error::Error for RetirementRejected {}
#[derive(Debug)]
struct RetirementPanicked(sugarsubstitute_contracts::execution::PanicCause);
impl std::fmt::Display for RetirementPanicked {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _retained = &self.0;
        formatter.write_str("Original custody transport panicked")
    }
}
impl std::error::Error for RetirementPanicked {}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
