// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow accepted deliberate close intent without clock mutation or settlement inference.
use super::*;

/// Inert deliberate close intent accepted by one exact desktop exit transaction.
/// This preserves the original budget and grants no native or resource-settlement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopCloseRequest {
    run: RunKey,
    process: ProcessKey,
    intent: CompletionIntent,
    budget: ShutdownBudget,
    exited: bool,
}
impl DesktopCloseRequest {
    /// Return whether this transaction accepted its matching deliberate native exit.
    /// Family, pipe, profile and resource retirement still require separate owners.
    pub const fn exited(self) -> bool {
        self.exited
    }
    /// Return the unchanged current desktop run.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Return the unchanged process registry identity bound at transaction construction.
    pub const fn process(self) -> ProcessKey {
        self.process
    }
    /// Return the originally accepted normal or startup-cancellation intent.
    pub const fn intent(self) -> CompletionIntent {
        self.intent
    }
    /// Return the original 10/15-second transaction in its owner's monotonic epoch.
    pub const fn budget(self) -> ShutdownBudget {
        self.budget
    }
}
impl DesktopExitTransaction {
    /// Return only an actually accepted deliberate close request from this retained transaction.
    ///
    /// This passive query samples no clock, changes no phase, and grants no native control or
    /// resource settlement. An accepted failed/unexpected exit or retained cleanup failure denies
    /// normal quiescence; its original classification remains available through failed_incident.
    /// StartupFailed intent is excluded. Late messages cannot reclassify an already accepted exit.
    pub fn normal_close_request(&self) -> Option<DesktopCloseRequest> {
        if self.failure.is_some() {
            return None;
        }
        let transaction = self.transaction?;
        let (intent, _expected) = transaction.intent?;
        let allowed = matches!(
            (intent, self.exit.map(|exit| exit.attribution)),
            (CompletionIntent::Normal, None | Some(DesktopExitKind::Normal))
                | (
                    CompletionIntent::StartupCancelled,
                    None | Some(DesktopExitKind::StartupCancelled)
                )
        );
        allowed.then_some(DesktopCloseRequest {
            run: self.run,
            process: self.process,
            intent,
            budget: transaction.budget,
            exited: self.exit.is_some(),
        })
    }
}
#[cfg(test)]
#[path = "normal_close_tests.rs"]
mod tests;
