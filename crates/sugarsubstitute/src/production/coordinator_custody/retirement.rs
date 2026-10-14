// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain helper Finish and original native pipe terminal through checked retirement.
use super::*;
use sugarsubstitute_contracts::execution::{JobOutcome, TerminalKind};
impl CoordinatorCustody {
    /// Finish the authenticated witness, observe its real exit, then close the actual pipe.
    pub(crate) fn close(
        &mut self,
        budget: ShutdownBudget,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        let original = *self.shutdown.get_or_insert(budget);
        if original != budget {
            return Err("Custody original cleanup changed".into());
        }
        if self.settled {
            return self.terminal_result();
        }
        let Some(parent) = self.parent.clone() else {
            self.settled = self.attachment.close(resources)?;
            return Ok(());
        };
        let now = resources.client.now()?;
        self.grace_observed |= now >= original.graceful_until;
        self.total_observed |= now >= original.settle_until;
        let mut failure: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        if self.ready && !self.finish_received && resources.client.now()? < original.settle_until {
            match parent.advance_finish(resources.client, original.settle_until) {
                Ok(finished) => {
                    self.finish_received = finished;
                    if finished {
                        tracing::info!(
                            operation = "backend_custody_finished",
                            key = ?parent.key(),
                            settle_until = ?original.settle_until,
                            "Authenticated backend custody Finished receipt consumed"
                        );
                    }
                }
                Err(cause) => failure = Some(diagnostics::failed("finish_exchange", cause)),
            }
        }
        if let Some(ticket) = self.ticket {
            let registry = resources.authority.registry_mut().ok_or("Custody registry absent")?;
            registry.begin_stop(ticket.process(), ShutdownId::new(1)?, original)?;
            let actual = registry.snapshot(ticket.process())?;
            if (actual.cleanup_step_due(resources.client.now()?)
                || (actual.settled && registry.custody_retirement_pending(ticket.process())?))
                && !actual.cleanup_step_pending
                && resources.client.now()? < original.settle_until
            {
                let watch = if actual.member_watch_required {
                    Some(spec(resources, JobClass::NativeObservation, 4096, None)?)
                } else {
                    None
                };
                let cleanup = spec(
                    resources,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(original.settle_until),
                )?;
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody registry absent")?
                    .submit_cleanup_step(
                        resources.client,
                        ticket.process(),
                        resources.cleanup,
                        cleanup,
                        watch,
                    )?;
            }
            let registry = resources.authority.registry_mut().ok_or("Custody registry absent")?;
            if registry.snapshot(ticket.process())?.settled
                && !registry.custody_retirement_pending(ticket.process())?
            {
                while registry.take_observation(ticket.process())?.is_some() {}
                registry.retire_settled(ticket.process())?;
                self.ticket = None;
            }
        }
        if self.ticket.is_none() {
            if !self.pipe_closed {
                parent.close().map_err(|cause| diagnostics::failed("pipe_close", cause))?;
                self.pipe_closed = true;
            }
            if self.terminal.is_none() {
                self.terminal = parent
                    .take_terminal()
                    .map_err(|cause| diagnostics::failed("pipe_terminal", cause))?;
            }
            if let Some(terminal) = &self.terminal {
                self.attachment.parent_retired();
                self.settled = true;
                tracing::info!(
                    operation = "backend_custody_retired",
                    key = ?parent.key(),
                    finish_received = self.finish_received,
                    terminal_kind = ?terminal.outcome.kind(),
                    terminal_job = ?terminal.spec.id,
                    terminal_scope = ?terminal.scope_id,
                    "Backend custody family and transport retired"
                );
            }
        }
        match failure {
            Some(cause) => Err(cause),
            None => {
                if self.settled {
                    self.terminal_result()
                } else {
                    Ok(())
                }
            }
        }
    }
    /// Preserve the retained terminal outcome independently of physical closure.
    fn terminal_result(&self) -> Result<()> {
        let Some(terminal) = &self.terminal else { return Ok(()) };
        inspect_terminal(
            &terminal.outcome,
            self.parent()?.key(),
            self.ready,
            self.finish_received,
            self.pipe_closed,
            self.ticket.is_none(),
        )
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
