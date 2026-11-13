// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The host's updater as the embedded page's Updates entry reaches it.
//!
//! The page sees the coordinator's update status exactly as the window shows it, and a restart
//! the page asks for goes through the window's own close: at once, or once the backend's running
//! workflow finishes, just as the caption's restart does. The coordinator takes no check
//! request from the window, so a check from the page is declined as unavailable rather than
//! claimed.
use super::workflow_restart::WorkflowRestart;
use sugarsubstitute_application::updates_panel::{
    UpdateSnapshot, UpdateStatusPort, WorkflowActivitySource,
};
use sugarsubstitute_contracts::{
    host_updates::{UpdatesRejection, UpdatesRestartTiming},
    update::status::UpdateStatus,
};
use sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue;

/// One turn's view of the host's updater for the page, and the close the page asked for.
pub(super) struct HostUpdates<'a> {
    status: Option<UpdateStatus>,
    queue: WorkflowQueue,
    closing: bool,
    restart: &'a mut WorkflowRestart,
    close_now: bool,
}

impl<'a> HostUpdates<'a> {
    /// The window's `status`, the backend's workflow `queue`, whether the application is
    /// already `closing`, and the restart that may wait for the workflow.
    pub(super) fn new(
        status: Option<UpdateStatus>,
        queue: WorkflowQueue,
        closing: bool,
        restart: &'a mut WorkflowRestart,
    ) -> Self {
        Self { status, queue, closing, restart, close_now: false }
    }

    /// Whether the page asked for a restart that closes the application now.
    pub(super) fn close_now(&self) -> bool {
        self.close_now
    }
}

impl UpdateStatusPort for HostUpdates<'_> {
    fn snapshot(&self) -> Option<UpdateSnapshot> {
        let status = self.status.clone()?;
        // The window process does not see the installation's subscription; it follows
        // releases until the coordinator reports otherwise.
        Some(UpdateSnapshot {
            status,
            channel: sugarsubstitute_contracts::update::Channel::default(),
            restart_after_workflow: self.restart.waiting(),
        })
    }

    fn check_now(&mut self) -> Result<(), UpdatesRejection> {
        Err(if self.closing { UpdatesRejection::Busy } else { UpdatesRejection::Unavailable })
    }

    fn restart(&mut self, when: UpdatesRestartTiming) -> Result<(), UpdatesRejection> {
        if self.closing {
            return Err(UpdatesRejection::Busy);
        }
        if self.status.as_ref().and_then(UpdateStatus::ready).is_none() {
            return Err(UpdatesRejection::NotReady);
        }
        self.close_now = match when {
            UpdatesRestartTiming::Now => self.restart.request(WorkflowQueue::Idle),
            UpdatesRestartTiming::AfterWorkflow => self.restart.request(self.queue),
        };
        Ok(())
    }
}

/// The backend's workflow queue as the page reads it: running only while it reads busy.
pub(super) struct QueueActivity(pub(super) WorkflowQueue);

impl WorkflowActivitySource for QueueActivity {
    fn workflow_running(&self) -> bool {
        self.0 == WorkflowQueue::Busy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::update::status::RecordedUpdates;

    fn ready() -> UpdateStatus {
        let mut status = UpdateStatus::default();
        status.reconcile(RecordedUpdates { prepared: Some("2.0.0".into()), ..Default::default() });
        status
    }

    #[test]
    fn the_page_sees_the_windows_status_and_whether_a_restart_waits() {
        let mut restart = WorkflowRestart::default();
        let port = HostUpdates::new(None, WorkflowQueue::Idle, false, &mut restart);
        assert_eq!(port.snapshot(), None, "nothing to show before the coordinator reports");
        let port = HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart);
        let snapshot = port.snapshot();
        assert_eq!(snapshot.as_ref().and_then(|shown| shown.status.ready()), Some("2.0.0"));
        assert_eq!(snapshot.map(|shown| shown.restart_after_workflow), Some(false));
        assert!(!restart.request(WorkflowQueue::Busy));
        let port = HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart);
        assert_eq!(port.snapshot().map(|shown| shown.restart_after_workflow), Some(true));
        assert!(QueueActivity(WorkflowQueue::Busy).workflow_running());
        assert!(!QueueActivity(WorkflowQueue::Idle).workflow_running());
        assert!(!QueueActivity(WorkflowQueue::Unobserved).workflow_running());
    }

    #[test]
    fn a_restart_after_the_workflow_waits_for_a_busy_queue_and_closes_at_once_when_idle() {
        let mut restart = WorkflowRestart::default();
        let mut port = HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart);
        assert_eq!(port.restart(UpdatesRestartTiming::AfterWorkflow), Ok(()));
        assert!(!port.close_now(), "a running workflow keeps the application open");
        assert!(restart.waiting());
        assert!(restart.advance(WorkflowQueue::Idle), "the close starts once the queue is idle");
        let mut port = HostUpdates::new(Some(ready()), WorkflowQueue::Idle, false, &mut restart);
        assert_eq!(port.restart(UpdatesRestartTiming::AfterWorkflow), Ok(()));
        assert!(port.close_now());
    }

    #[test]
    fn a_restart_now_closes_at_once_even_while_a_workflow_runs() {
        let mut restart = WorkflowRestart::default();
        assert!(!restart.request(WorkflowQueue::Busy));
        let mut port = HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart);
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Ok(()));
        assert!(port.close_now());
        assert!(!restart.waiting(), "nothing waits once the close starts");
    }

    #[test]
    fn requests_the_host_cannot_honour_are_declined_with_their_reason() {
        let mut restart = WorkflowRestart::default();
        let mut port = HostUpdates::new(
            Some(UpdateStatus::default()),
            WorkflowQueue::Idle,
            false,
            &mut restart,
        );
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Err(UpdatesRejection::NotReady));
        assert_eq!(port.check_now(), Err(UpdatesRejection::Unavailable));
        assert!(!port.close_now());
        let mut port = HostUpdates::new(Some(ready()), WorkflowQueue::Idle, true, &mut restart);
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Err(UpdatesRejection::Busy));
        assert_eq!(port.check_now(), Err(UpdatesRejection::Busy));
        assert!(!port.close_now());
    }
}
