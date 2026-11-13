// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound the installed launcher in seconds and decide when it shows that it is starting.
//!
//! The launcher runs before any application window exists. A normal launch hands startup to
//! the application well within [`LauncherBudget::NOTICE_DELAY`] and shows nothing of its own.
//! A launch that is still the launcher's responsibility after that delay shows a small
//! "Starting…" notice until it hands off. Optional update work stops being admitted at
//! [`LauncherBudget::update_until`] so the accepted version still starts inside
//! [`LauncherBudget::until`], and the process exits no later than
//! [`LauncherBudget::exit_at`] after its cleanup cutoff even if a worker never returns.
//! Every moment here is on the launcher's injected execution clock; nothing reads wall time.

use crate::execution::{Moment, TimeError};
use std::time::Duration;

/// Frozen moments of one launcher process, measured from its own start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LauncherBudget {
    started: Moment,
    notice_at: Moment,
    update_until: Moment,
    until: Moment,
}

impl LauncherBudget {
    /// How long a launch may stay invisible before the "Starting…" notice appears.
    pub const NOTICE_DELAY: Duration = Duration::from_secs(2);
    /// Last moment an optional step (an update or its trial) may begin. Later, the launcher
    /// starts the accepted version instead.
    pub const UPDATE: Duration = Duration::from_secs(60);
    /// The whole launch, including the accepted version after an abandoned update. The
    /// longest wait on another process admitted before [`Self::UPDATE`] is the trial
    /// adoption (32 s), then trial recovery (15 s) and accepted selection (5 s), so the
    /// accepted version keeps room to start.
    pub const DURATION: Duration = Duration::from_secs(120);
    /// How long actual worker joins may still take after the cleanup cutoff was reported.
    /// The process then exits regardless; the kernel releases every handle it still holds.
    pub const EXIT_GRACE: Duration = Duration::from_secs(5);

    /// Freeze every launcher moment from its start.
    /// # Errors
    /// Returns the clock overflow if a moment is unrepresentable.
    pub fn new(started: Moment) -> Result<Self, TimeError> {
        Ok(Self {
            started,
            notice_at: started.checked_add(Self::NOTICE_DELAY)?,
            update_until: started.checked_add(Self::UPDATE)?,
            until: started.checked_add(Self::DURATION)?,
        })
    }
    /// The launcher's own start.
    pub const fn started(self) -> Moment {
        self.started
    }
    /// When the "Starting…" notice may first appear.
    pub const fn notice_at(self) -> Moment {
        self.notice_at
    }
    /// The last moment an optional update step may be admitted.
    pub const fn update_until(self) -> Moment {
        self.update_until
    }
    /// The unchanged deadline of the whole launch.
    pub const fn until(self) -> Moment {
        self.until
    }
    /// Whether an optional update step may still begin at `now`.
    pub fn admits_update(self, now: Moment) -> bool {
        now < self.update_until
    }
    /// When the process exits regardless, given the moment its cleanup cutoff was reported.
    /// # Errors
    /// Returns the clock overflow if the moment is unrepresentable.
    pub fn exit_at(reported: Moment) -> Result<Moment, TimeError> {
        reported.checked_add(Self::EXIT_GRACE)
    }
}

/// A change the notice's presenter applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeChange {
    /// Put the notice on screen.
    Show,
    /// Take the notice down; it never returns in this process.
    Withdraw,
}

/// Where the notice is in its one-way life.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticePhase {
    /// Not shown yet; it appears at its moment if the launcher still owns startup.
    Waiting,
    /// On screen.
    Shown,
    /// Gone for good: startup was handed off, or the launcher is closing.
    Withdrawn,
}

/// Decide, from observed facts only, when the launcher's "Starting…" notice is on screen.
///
/// The notice appears once the launcher has owned startup for [`LauncherBudget::NOTICE_DELAY`]
/// and disappears once it no longer does. A launch handed off before the delay never shows
/// it. The notice is one-way: once withdrawn it never reappears in the same process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartingNotice {
    at: Moment,
    phase: NoticePhase,
}

impl StartingNotice {
    /// A notice that may appear at the budget's notice moment.
    pub const fn new(budget: LauncherBudget) -> Self {
        Self { at: budget.notice_at, phase: NoticePhase::Waiting }
    }
    /// The current phase.
    pub const fn phase(self) -> NoticePhase {
        self.phase
    }
    /// The moment the owner must wake to show the notice, while it is still waiting.
    pub const fn next_wake(self) -> Option<Moment> {
        match self.phase {
            NoticePhase::Waiting => Some(self.at),
            NoticePhase::Shown | NoticePhase::Withdrawn => None,
        }
    }
    /// Observe `now` and whether the launcher still owns startup, and return the change the
    /// presenter applies, if any.
    pub fn observe(&mut self, now: Moment, owns_startup: bool) -> Option<NoticeChange> {
        match (self.phase, owns_startup) {
            (NoticePhase::Waiting, true) if now >= self.at => {
                self.phase = NoticePhase::Shown;
                Some(NoticeChange::Show)
            }
            (NoticePhase::Waiting, false) => {
                self.phase = NoticePhase::Withdrawn;
                None
            }
            (NoticePhase::Shown, false) => {
                self.phase = NoticePhase::Withdrawn;
                Some(NoticeChange::Withdraw)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "launcher_tests.rs"]
mod tests;
