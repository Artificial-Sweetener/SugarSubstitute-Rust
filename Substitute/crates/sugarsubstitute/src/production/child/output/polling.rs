// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound inactive IPC requests while the execution owner continuously drains native pipes.
use super::*;

/// Maximum extra delay before observing new output or a changed backend notice after inactivity.
const IDLE_INTERVAL: Duration = Duration::from_millis(100);

/// Preserve immediate backlog draining; only an empty or outage reply starts an inactive interval.
pub(super) fn after_reply(now: Moment, idle: bool) -> Result<Option<Moment>> {
    Ok(if idle { Some(now.checked_add(IDLE_INTERVAL)?) } else { None })
}
impl ChildOutput {
    /// Admit at most one pull per inactive interval, without rebasing early native wakeups.
    pub(super) fn pull_due(&mut self, now: Moment) -> bool {
        if self.pull_after.is_some_and(|at| now < at) {
            return false;
        }
        self.pull_after = None;
        true
    }
}
