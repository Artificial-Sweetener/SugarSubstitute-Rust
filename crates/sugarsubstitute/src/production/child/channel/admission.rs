// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close positive channel admission at the actual matching terminal notice.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::TerminalKind;

/// Reject inconsistent terminal metadata while retaining the first classification.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct TerminalConflict;
impl fmt::Display for TerminalConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Production lifecycle terminal classification changed")
    }
}
impl Error for TerminalConflict {}

/// Own the single outbound fence; metadata never substitutes for typed completion.
pub(super) struct ChannelAdmission {
    closed: bool,
    terminal: Option<TerminalKind>,
}
impl ChannelAdmission {
    /// Begin inert and open before the actual transport reservation.
    pub(super) const fn new() -> Self {
        Self { closed: false, terminal: None }
    }

    /// Check positive admission independently of typed result availability.
    pub(super) const fn open(&self) -> bool {
        !self.closed && self.terminal.is_none()
    }

    /// Observe only metadata already matched to the original real scope/job.
    /// # Errors
    /// A conflicting classification preserves the original terminal fence.
    pub(super) fn observe_terminal(&mut self, kind: TerminalKind) -> Result<(), TerminalConflict> {
        if self.terminal.is_some_and(|original| original != kind) {
            return Err(TerminalConflict);
        }
        self.terminal = Some(kind);
        Ok(())
    }

    /// Borrow matching metadata while retaining actual completion at its owner.
    pub(super) const fn terminal(&self) -> Option<TerminalKind> {
        self.terminal
    }

    /// Fence fresh commands without manufacturing a terminal or native settlement.
    pub(super) fn close(&mut self) {
        self.closed = true;
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
