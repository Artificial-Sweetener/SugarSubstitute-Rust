// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Surface an existing typed channel failure while genuine pre-UI preparation remains active.
use super::{ChildBootstrap, ChildBootstrapIssue};
use sugarsubstitute_contracts::execution::JobOutcome;

impl ChildBootstrap {
    /// Surface the channel's original failed terminal only during active pre-UI preparation.
    pub(super) fn capture_transport_failure(&mut self) -> Option<ChildBootstrapIssue> {
        let cause = preparation_failure(
            self.channel.terminal().map(|terminal| &terminal.outcome),
            !self.closing && !self.delivered,
        )?;
        if self.failure.is_none() {
            crate::production::child_transport_diagnostic::report(cause.native_cause());
        }
        Some(self.record(ChildBootstrapIssue::Transport, Box::new(cause)))
    }
}

/// Clone only the actual failed outcome; cleanup, cancellation and success retain their policy.
/// The complete original completion remains owned by the lifecycle channel.
pub(super) fn preparation_failure<T, E: Clone>(
    outcome: Option<&JobOutcome<T, E>>,
    preparing: bool,
) -> Option<E> {
    if preparing && let Some(JobOutcome::Failure(cause)) = outcome {
        Some(cause.clone())
    } else {
        None
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
