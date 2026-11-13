// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold a restart to apply an update until the backend's running workflow finishes.
//!
//! The coordinator reports the backend's workflow queue with every update status. A restart
//! requested while the queue is busy waits; it starts the normal close once the queue reads
//! idle. When the coordinator loses track of the queue while a restart waits, the restart is
//! dropped rather than risk ending a workflow, and the window says so. Nothing restarts blindly.
use sugarsubstitute_contracts::update::{
    attempt::{AttemptOutcome, AttemptPhase, UpdateReport},
    status::UpdateStatus,
};
use sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue;

use super::{StartupBackend, StartupHost};

impl<B: StartupBackend> StartupHost<B> {
    /// Start the close that restarts to apply the prepared update.
    pub(super) fn restart_to_apply(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.request_close(
            sugarsubstitute_application::workspace_session::close_save::CloseReason::RestartToApply,
        )
    }

    /// Start a waiting restart once the backend's workflow finished. Called every turn.
    pub(super) fn advance_workflow_restart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.closing && self.workflow_restart.advance(self.child.workflow_queue()) {
            self.restart_to_apply()?;
        }
        Ok(())
    }
}

/// Whether a restart waits for the workflow, or was dropped while waiting.
#[derive(Debug, Default)]
pub(super) struct WorkflowRestart {
    waiting: bool,
    dropped: bool,
}
impl WorkflowRestart {
    /// The user asked to restart with the queue reading `queue`. True when the close starts
    /// now; false when it waits for the workflow.
    pub(super) fn request(&mut self, queue: WorkflowQueue) -> bool {
        self.dropped = false;
        self.waiting = queue == WorkflowQueue::Busy;
        !self.waiting
    }

    /// Adopt the newest queue. True once when a waiting restart may start the close.
    pub(super) fn advance(&mut self, queue: WorkflowQueue) -> bool {
        if !self.waiting {
            return false;
        }
        match queue {
            WorkflowQueue::Busy => false,
            WorkflowQueue::Idle => {
                self.waiting = false;
                true
            }
            WorkflowQueue::Unobserved => {
                tracing::warn!(
                    operation = "restart_after_workflow_dropped",
                    "The workflow queue was lost while a restart waited; nothing was restarted"
                );
                self.waiting = false;
                self.dropped = true;
                false
            }
        }
    }

    /// `status` as the window shows it: a dropped restart reads as a failed attempt to start
    /// the prepared version, with `reason` as its cause, until the user asks again.
    pub(super) fn presented(&self, mut status: UpdateStatus, reason: &str) -> UpdateStatus {
        if self.dropped {
            let version = status.ready().map(str::to_owned);
            status.finish(
                Some(AttemptOutcome::Failed(UpdateReport {
                    version,
                    phase: AttemptPhase::Applying,
                    causes: vec![reason.to_owned()],
                })),
                0,
            );
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restart_waits_for_a_busy_queue_and_closes_once_it_is_idle() {
        let mut restart = WorkflowRestart::default();
        assert!(restart.request(WorkflowQueue::Idle), "an idle queue closes at once");
        assert!(!restart.advance(WorkflowQueue::Idle), "nothing waits after an immediate close");
        assert!(!restart.request(WorkflowQueue::Busy));
        assert!(!restart.advance(WorkflowQueue::Busy));
        assert!(restart.advance(WorkflowQueue::Idle));
        assert!(!restart.advance(WorkflowQueue::Idle), "the close starts once");
    }

    #[test]
    fn losing_the_queue_while_waiting_drops_the_restart_visibly_until_asked_again() {
        let mut restart = WorkflowRestart::default();
        assert!(!restart.request(WorkflowQueue::Busy));
        assert!(!restart.advance(WorkflowQueue::Unobserved));
        assert!(!restart.advance(WorkflowQueue::Idle), "a dropped restart never closes later");
        let shown = restart.presented(UpdateStatus::default(), "Nothing was restarted.");
        let failure = shown.failure().map(|report| (report.phase, report.causes.clone()));
        assert_eq!(failure, Some((AttemptPhase::Applying, vec!["Nothing was restarted.".into()])));
        assert!(restart.request(WorkflowQueue::Unobserved), "asking again closes at once");
        assert!(restart.presented(UpdateStatus::default(), "x").failure().is_none());
    }
}
