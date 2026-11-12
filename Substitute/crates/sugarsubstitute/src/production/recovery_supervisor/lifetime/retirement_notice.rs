// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep advisory retirement behind genuine local settlement and the original native write.
use super::*;
use sugarsubstitute_contracts::execution::CounterDeadline;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Wait,
    Send,
    Close,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Transfer {
    Waiting,
    Queued,
    Written,
}
/// Policy only: callers supply actual child/capture settlement and checked channel observations.
/// This owner creates no process, write-completion or physical-retirement proof.
pub(super) struct RetirementNotice {
    stop: Option<CounterDeadline>,
    transfer: Transfer,
    abandoned: bool,
}
impl RetirementNotice {
    pub(super) const fn new() -> Self {
        Self { stop: None, transfer: Transfer::Waiting, abandoned: false }
    }
    /// Preserve the authenticated original counter; replay cannot renew or replace it.
    pub(super) fn accept_stop(&mut self, counter: CounterDeadline) -> Result<bool> {
        match self.stop {
            Some(original) if original == counter => Ok(false),
            Some(_) => Err("Recovery original Stop counter changed".into()),
            None => {
                self.stop = Some(counter);
                Ok(true)
            }
        }
    }
    pub(super) const fn stop_received(&self) -> bool {
        self.stop.is_some()
    }
    /// The natural DesktopExited path keeps receiving Stop, without granting Retired itself.
    pub(super) fn action(&self, now: Moment, until: Moment, local_retired: bool) -> Action {
        if self.abandoned || now >= until || self.transfer == Transfer::Written {
            Action::Close
        } else if self.stop.is_some() && local_retired && self.transfer == Transfer::Waiting {
            Action::Send
        } else {
            Action::Wait
        }
    }
    /// Retain the original failure and close without emitting an advisory completion.
    pub(super) fn abandon(&mut self) {
        self.abandoned = true;
    }
    /// Queue admission cannot authorize healthy close or substitute actual native completion.
    pub(super) fn queued(&mut self, now: Moment, until: Moment, local_retired: bool) -> Result<()> {
        if self.action(now, until, local_retired) != Action::Send {
            return Err("Recovery Retired queue order rejected".into());
        }
        self.transfer = Transfer::Queued;
        Ok(())
    }
    /// Called only for the channel's exact issued, complete original Retired write observation.
    pub(super) fn written(&mut self) -> Result<()> {
        if self.transfer != Transfer::Queued {
            return Err("Recovery Retired completion order rejected".into());
        }
        self.transfer = Transfer::Written;
        Ok(())
    }
}
#[cfg(test)]
#[path = "retirement_notice_tests.rs"]
mod tests;
