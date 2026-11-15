// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide, for one coordinator, which SugarCubes work runs next and which backend start it holds.
//!
//! The coordinator runs one job at a time from this schedule on its execution owner and
//! reports each job's end here; the schedule itself performs no effect. The first backend
//! preparation of a coordinator's life runs the launch-time update: for a managed backend it
//! holds that preparation's SugarCubes inspection and launch until it settles, for an attached
//! ComfyUI it only checks, read-only, and holds nothing. Trial observations of the judged
//! version run in the order the coordinator saw them. When the judged version's page reports
//! SugarCubes failing, the next managed preparation is held behind a restore of the previous
//! version, so the version that failed never loads twice; the user cancelling a startup whose
//! page never became ready is the same observed failure. The live [`PackUpdateStatus`] and
//! its revision are what the coordinator publishes to its window.

use super::startup::StartupOutcome;
use super::trial::{Restored, TrialEvent};
use std::collections::VecDeque;
use sugarsubstitute_contracts::update::pack::{PackOutcome, PackUpdateStatus};

/// One unit of SugarCubes work for the coordinator to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackWork {
    /// The launch-time update (managed) or read-only check (attached).
    Update {
        /// Whether the host starts this backend, so the pack may be replaced.
        managed: bool,
    },
    /// Record one observation of the judged version.
    Observe(TrialEvent),
    /// The managed backend ended before it was ready.
    BackendStopped,
    /// The judged version's page failed and its backend stopped: restore the previous version.
    RestoreFailedPage,
    /// The page loaded while the status said the previous version runs again: the user has
    /// been shown it, so later starts stop repeating it.
    RestoreTold,
}

/// What a job of the schedule does, which decides how much time and memory it is given.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackWorkKind {
    /// Checks the release sources and may download and install a version.
    Download,
    /// May put the previous version's files back, a replacement as large as an install.
    Replace,
    /// Writes one small journal record.
    Record,
}

impl PackWork {
    /// What this job does. A restore is given an install's time, never an observation's, so
    /// a slow disk cannot cut it short and lose the report of what it restored.
    pub const fn kind(self) -> PackWorkKind {
        match self {
            Self::Update { .. } => PackWorkKind::Download,
            Self::BackendStopped | Self::RestoreFailedPage => PackWorkKind::Replace,
            Self::Observe(_) | Self::RestoreTold => PackWorkKind::Record,
        }
    }
}

/// How one job of the schedule ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackWorkEnded {
    /// The update or check finished with this outcome.
    Updated(StartupOutcome),
    /// An observation was recorded.
    Observed,
    /// A restore ran; what it restored, if anything.
    Restored(Option<Restored>),
    /// The job did not finish: cancelled or out of time (`stopped`), or it failed.
    Unfinished {
        /// Whether it was cancelled or ran out of time rather than failing.
        stopped: bool,
    },
}

/// The SugarCubes work of one coordinator, generic over its backend resource key.
#[derive(Debug)]
pub struct PackSchedule<K> {
    checked: bool,
    judging: bool,
    page_failed: bool,
    running: Option<(PackWork, Option<K>)>,
    queued: VecDeque<(PackWork, Option<K>)>,
    status: PackUpdateStatus,
    /// The status a restore that finds nothing to put back returns to.
    before_restore: Option<PackUpdateStatus>,
    revision: u64,
    /// The launch-time update waits: this launch may still restart into an application
    /// update, and then the restarted copy runs it.
    update_held: bool,
    /// The record that the shown restore was seen is queued or written.
    restore_told: bool,
}

impl<K> Default for PackSchedule<K> {
    fn default() -> Self {
        Self {
            checked: false,
            judging: false,
            page_failed: false,
            running: None,
            queued: VecDeque::new(),
            status: PackUpdateStatus::Idle,
            before_restore: None,
            revision: 0,
            update_held: false,
            restore_told: false,
        }
    }
}

impl<K: Copy + Eq> PackSchedule<K> {
    /// A backend preparation `key` begins. Returns whether SugarCubes work now holds its
    /// inspection and launch. A managed preparation always waits for a restore that is
    /// running or queued, so a retry never reads or starts the pack while its files change.
    pub fn prepare(&mut self, key: K, managed: bool) -> bool {
        if !self.checked {
            self.checked = true;
            self.judging = managed;
            self.queued.push_back((PackWork::Update { managed }, managed.then_some(key)));
            return managed;
        }
        if !managed {
            return false;
        }
        if std::mem::take(&mut self.page_failed) {
            self.queued.push_back((PackWork::RestoreFailedPage, Some(key)));
            self.restoring();
            return true;
        }
        self.hold_behind_restore(key)
    }

    /// Let a restore that holds no preparation yet hold `key`; returns whether one does.
    fn hold_behind_restore(&mut self, key: K) -> bool {
        let pending = self.running.iter_mut().chain(self.queued.iter_mut());
        for (work, gates) in pending {
            if work.kind() == PackWorkKind::Replace && gates.is_none_or(|held| held == key) {
                *gates = Some(key);
                return true;
            }
        }
        false
    }

    /// Publish that the previous version is being put back, remembering what to show again if
    /// nothing needs restoring.
    fn restoring(&mut self) {
        if self.status != PackUpdateStatus::Restoring {
            self.before_restore = Some(std::mem::take(&mut self.status));
            self.publish(PackUpdateStatus::Restoring);
        }
    }

    /// Record an observation of the judged version; ignored when nothing is judged here. A
    /// page that loads while the status says the previous version runs again has shown the
    /// user that line, which is recorded once so later starts stop repeating it.
    pub fn observe(&mut self, event: TrialEvent) {
        if self.judging {
            self.queued.push_back((PackWork::Observe(event), None));
        }
        let restored = matches!(
            &self.status,
            PackUpdateStatus::Finished(report) if report.outcome == PackOutcome::Restored
        );
        if restored && !self.restore_told && matches!(event, TrialEvent::PageLoaded { .. }) {
            self.restore_told = true;
            self.queued.push_back((PackWork::RestoreTold, None));
        }
    }

    /// The managed backend ended before it was ready.
    pub fn backend_stopped(&mut self) {
        if self.judging {
            self.queued.push_back((PackWork::BackendStopped, None));
            self.restoring();
        }
    }

    /// The attempt's page reported SugarCubes itself failing: record it, and restore before
    /// the next managed preparation.
    pub fn page_failed(&mut self) {
        if self.judging {
            self.page_failed = true;
            self.observe(TrialEvent::PageFailed);
        }
    }

    /// The user cancelled startup before its page became ready. For the judged version this is
    /// an observed page failure: the user stopped waiting for a page that never loaded, so the
    /// previous version is restored before another start loads it. The judgement counts it only
    /// after the version's backend became ready and before its page loaded; earlier, the
    /// backend ending decides, and a version whose page loaded is already kept.
    pub fn startup_cancelled(&mut self) {
        self.page_failed();
    }

    /// Whether stopping `key` may cancel the running job: the launch-time update that holds
    /// it. A restore is never cancelled; the preparation waiting for it simply stops waiting.
    pub fn running_holds(&self, key: K) -> bool {
        self.running.is_some_and(|(work, gates)| {
            gates == Some(key) && work.kind() == PackWorkKind::Download
        })
    }

    /// Whether `key`'s inspection and launch still wait for SugarCubes work.
    pub fn gates(&self, key: K) -> bool {
        self.running.is_some_and(|(_, gates)| gates == Some(key))
            || self.queued.iter().any(|(_, gates)| *gates == Some(key))
    }

    /// Hold the launch-time update while this launch may still restart into an application
    /// update, or let it start. A running update is never held back; it runs to its end.
    pub fn hold_update(&mut self, held: bool) {
        self.update_held = held;
    }

    /// Whether SugarCubes files are being installed or put back now. A restart waits for this
    /// to end instead of cutting it short.
    pub fn installing(&self) -> bool {
        self.running.is_some_and(|(work, _)| work.kind() != PackWorkKind::Record)
    }

    /// `key` is being stopped before its launch-time update began: that update is left to the
    /// copy that starts next, and `key` no longer waits for it.
    pub fn withdraw_update(&mut self, key: K) {
        self.queued.retain(|(work, gates)| {
            !(matches!(work, PackWork::Update { .. }) && *gates == Some(key))
        });
    }

    /// The next work to start when no job runs, with the preparation it holds. A held
    /// launch-time update and everything after it wait.
    pub fn start_next(&mut self) -> Option<(PackWork, Option<K>)> {
        if self.running.is_some() {
            return None;
        }
        if self.update_held
            && self.queued.front().is_some_and(|(work, _)| matches!(work, PackWork::Update { .. }))
        {
            return None;
        }
        let (work, gates) = self.queued.pop_front()?;
        self.running = Some((work, gates));
        // The splash says SugarCubes is updating only while it really is, never while the
        // update waits, so a close never waits for an update that has not begun.
        if work == (PackWork::Update { managed: true }) {
            self.show_after_restores(PackUpdateStatus::Updating);
        }
        Some((work, gates))
    }

    /// The running job ended; returns the preparation it released, once nothing else holds it.
    /// The launch-time update or check publishes its report whether or not it held a start, so
    /// an attached ComfyUI's check is shown too.
    pub fn ended(&mut self, ended: PackWorkEnded) -> Option<K> {
        let (work, released) = self.running.take()?;
        let update = matches!(work, PackWork::Update { .. });
        let restore = work.kind() == PackWorkKind::Replace;
        match ended {
            PackWorkEnded::Updated(outcome) => {
                self.show_after_restores(PackUpdateStatus::Finished(outcome.report()));
            }
            PackWorkEnded::Restored(Some(restored)) => {
                self.before_restore = None;
                self.publish(PackUpdateStatus::Finished(
                    StartupOutcome::Restored(restored).report(),
                ));
            }
            PackWorkEnded::Restored(None) if !self.restore_pending() => {
                if let Some(previous) = self.before_restore.take() {
                    self.publish(previous);
                }
            }
            PackWorkEnded::Unfinished { stopped } if update || restore || released.is_some() => {
                let outcome = if stopped {
                    StartupOutcome::Stopped
                } else {
                    StartupOutcome::Failed { step: super::StartupStep::Checking, code: "job" }
                };
                let status = PackUpdateStatus::Finished(outcome.report());
                if restore {
                    self.before_restore = None;
                    self.publish(status);
                } else {
                    self.show_after_restores(status);
                }
            }
            _ => {}
        }
        released.filter(|key| !self.gates(*key))
    }

    /// Publish `status`, or keep it for after the restore when one is putting the previous
    /// version back, so the splash keeps saying so until it finished.
    fn show_after_restores(&mut self, status: PackUpdateStatus) {
        if self.status == PackUpdateStatus::Restoring {
            self.before_restore = Some(status);
        } else {
            self.publish(status);
        }
    }

    fn restore_pending(&self) -> bool {
        self.queued.iter().any(|(work, _)| work.kind() == PackWorkKind::Replace)
    }

    /// The live update state and its revision, which rises with every change.
    pub fn status(&self) -> (&PackUpdateStatus, u64) {
        (&self.status, self.revision)
    }

    fn publish(&mut self, status: PackUpdateStatus) {
        self.status = status;
        self.revision += 1;
    }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
