// SPDX-License-Identifier: GPL-3.0-or-later
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
//! version, so the version that failed never loads twice. The live [`PackUpdateStatus`] and
//! its revision are what the coordinator publishes to its window.

use super::startup::StartupOutcome;
use super::trial::{Restored, TrialEvent};
use std::collections::VecDeque;
use sugarsubstitute_contracts::update::pack::PackUpdateStatus;

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
    running: Option<Option<K>>,
    queued: VecDeque<(PackWork, Option<K>)>,
    status: PackUpdateStatus,
    revision: u64,
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
            revision: 0,
        }
    }
}

impl<K: Copy + Eq> PackSchedule<K> {
    /// A backend preparation `key` begins. Returns whether SugarCubes work now holds its
    /// inspection and launch.
    pub fn prepare(&mut self, key: K, managed: bool) -> bool {
        if !self.checked {
            self.checked = true;
            self.judging = managed;
            self.queued.push_back((PackWork::Update { managed }, managed.then_some(key)));
            if managed {
                self.publish(PackUpdateStatus::Updating);
            }
            return managed;
        }
        if !(managed && std::mem::take(&mut self.page_failed)) {
            return false;
        }
        self.queued.push_back((PackWork::RestoreFailedPage, Some(key)));
        self.publish(PackUpdateStatus::Updating);
        true
    }

    /// Record an observation of the judged version; ignored when nothing is judged here.
    pub fn observe(&mut self, event: TrialEvent) {
        if self.judging {
            self.queued.push_back((PackWork::Observe(event), None));
        }
    }

    /// The managed backend ended before it was ready.
    pub fn backend_stopped(&mut self) {
        if self.judging {
            self.queued.push_back((PackWork::BackendStopped, None));
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

    /// Whether the running job is the one holding `key`, so stopping `key` may cancel it.
    pub fn running_holds(&self, key: K) -> bool {
        self.running.is_some_and(|gates| gates == Some(key))
    }

    /// Whether `key`'s inspection and launch still wait for SugarCubes work.
    pub fn gates(&self, key: K) -> bool {
        self.running.is_some_and(|gates| gates == Some(key))
            || self.queued.iter().any(|(_, gates)| *gates == Some(key))
    }

    /// The next work to start when no job runs, with the preparation it holds.
    pub fn start_next(&mut self) -> Option<(PackWork, Option<K>)> {
        if self.running.is_some() {
            return None;
        }
        let (work, gates) = self.queued.pop_front()?;
        self.running = Some(gates);
        Some((work, gates))
    }

    /// The running job ended; returns the preparation it released, once nothing else holds it.
    pub fn ended(&mut self, ended: PackWorkEnded) -> Option<K> {
        let released = self.running.take().flatten();
        match ended {
            PackWorkEnded::Updated(outcome) if released.is_some() => {
                self.publish(PackUpdateStatus::Finished(outcome.report()));
            }
            PackWorkEnded::Restored(Some(restored)) => {
                self.publish(PackUpdateStatus::Finished(
                    StartupOutcome::Restored(restored).report(),
                ));
            }
            PackWorkEnded::Restored(None) if released.is_some() => {
                self.publish(PackUpdateStatus::Idle);
            }
            PackWorkEnded::Unfinished { stopped } if released.is_some() => {
                let outcome = if stopped {
                    StartupOutcome::Stopped
                } else {
                    StartupOutcome::Failed { step: super::StartupStep::Checking, code: "job" }
                };
                self.publish(PackUpdateStatus::Finished(outcome.report()));
            }
            _ => {}
        }
        released.filter(|key| !self.gates(*key))
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
