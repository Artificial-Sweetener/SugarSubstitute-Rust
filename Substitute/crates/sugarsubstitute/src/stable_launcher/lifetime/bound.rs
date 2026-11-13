// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the launch inside its seconds budget: stop late update work, and always exit.
use super::*;

impl Launcher {
    /// Freeze the launch's moments at start and arm the native wakes that observe them.
    pub(super) fn begin_budget(&mut self, budget: LauncherBudget) {
        let now = budget.started();
        self.until = Some(budget.until());
        self.wake_at = native_wake(now, budget.until());
        self.update_wake = native_wake(now, budget.update_until());
        self.notice = Some(crate::stable_launcher::notice::Notice::new(budget));
        self.budget = Some(budget);
    }

    /// Show the notice while this launcher still owns startup, and drain its jobs. Closing
    /// withdraws it and only drains.
    pub(super) fn advance_notice(&mut self) {
        let owns = !self.completed && !self.closing;
        let now = self.clock.now();
        let Some(notice) = &mut self.notice else { return };
        notice.observe(now, owns);
        let (Some(client), Some(scope), Some(until)) = (&self.client, &self.work_scope, self.until)
        else {
            return;
        };
        notice.advance(
            &crate::stable_launcher::notice::Resources { client, scope, jobs: &self.jobs, until },
            self.closing,
        );
    }

    /// Whether the notice is on screen now.
    pub(super) fn notice_shown(&self) -> bool {
        self.notice.as_ref().is_some_and(crate::stable_launcher::notice::Notice::shown)
    }

    /// Whether a notice job still runs.
    pub(super) fn notice_busy(&self) -> bool {
        self.notice.as_ref().is_some_and(crate::stable_launcher::notice::Notice::busy)
    }

    /// Whether `task` would begin optional update work that may no longer begin. A refused step
    /// takes the same fallback as a failed one, so the accepted version starts in time. Cached
    /// preparation that already exists is not a new step: it retires first, through
    /// [`Self::stop_late_update`], before the accepted release is selected.
    pub(super) fn late_update(&self, task: Task) -> bool {
        let begins = match task {
            Task::Installation | Task::Trial | Task::Delegate | Task::Create { optional: true } => {
                true
            }
            Task::Cache => self.cached.is_none(),
            _ => false,
        };
        begins && self.budget.is_some_and(|budget| !budget.admits_update(self.clock.now()))
    }

    /// At the update cutoff, stop cached preparation so its retirement leads to the accepted
    /// version. Steps already running keep their own bounded waits; later ones are refused by
    /// admission. Observing the cutoff once retires its native wake.
    pub(super) fn stop_late_update(&mut self) -> Result<()> {
        if self.closing
            || self.completed
            || self.update_wake.is_none()
            || self.budget.is_none_or(|budget| budget.admits_update(self.clock.now()))
        {
            return Ok(());
        }
        self.update_wake = None;
        if let Some(cached) = &mut self.cached {
            tracing::warn!(
                operation = "startup_update_late",
                "Update preparation did not finish in time; starting the accepted release"
            );
            cached.abandon(self.clock.now())?;
        }
        Ok(())
    }

    /// Start the exit grace once the cleanup cutoff is reported; the moment never renews.
    pub(super) fn arm_exit(&mut self) -> Result<()> {
        if self.exit_at.is_none() {
            let now = self.clock.now();
            self.exit_at = Some((
                LauncherBudget::exit_at(now)?,
                Instant::now().checked_add(LauncherBudget::EXIT_GRACE),
            ));
        }
        Ok(())
    }

    /// Whether the exit grace has passed with work still unsettled.
    pub(super) fn exit_due(&self) -> bool {
        self.exit_at.is_some_and(|(at, _)| self.clock.now() >= at)
    }

    /// Exit without the workers that never returned. Their threads and native custody stay
    /// with this process, which is about to end; the kernel then releases every handle and
    /// lock they hold. Nothing here waits, so the process always exits.
    pub(super) fn abandon(&mut self) -> Box<dyn Error> {
        tracing::error!(
            operation = "launcher_exit_unsettled",
            "Launcher work did not stop after its cleanup cutoff; exiting without it"
        );
        std::mem::forget(self.job.take());
        std::mem::forget(self.launch.take());
        std::mem::forget(self.cached.take());
        std::mem::forget(self.engine.take());
        std::mem::forget(self.delegate.take());
        std::mem::forget(self.notice.take());
        std::mem::forget(self.work_scope.take());
        std::mem::forget(self.cleanup_scope.take());
        std::mem::forget(self.client.take());
        // Dropping the owner would join its workers; that join is what may never return.
        std::mem::forget(self.owner.take());
        self.finished = true;
        self.failure.take().unwrap_or_else(|| {
            "Launcher work did not stop after its cleanup cutoff; exiting without it".into()
        })
    }
}

/// The native wake for `at`, measured from `now` on the launcher's clock.
fn native_wake(now: Moment, at: Moment) -> Option<Instant> {
    Instant::now().checked_add(at.checked_duration_since(now).unwrap_or_default())
}
