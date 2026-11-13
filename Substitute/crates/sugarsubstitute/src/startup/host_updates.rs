// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The host's updater as the embedded page's Updates entry reaches it.
//!
//! The page sees the coordinator's update status exactly as the window shows it, and a restart
//! the page asks for goes through the window's own close: at once, or once the backend's running
//! workflow finishes, just as the caption's restart does. A check the page asks for goes to the
//! coordinator, which runs its usual update check or joins the one running; the page follows it
//! through the statuses that come back, with the channel the coordinator reports.
use super::workflow_restart::WorkflowRestart;
use std::error::Error;
use sugarsubstitute_application::updates_panel::{
    UpdateSnapshot, UpdateStatusPort, WorkflowActivitySource,
};
use sugarsubstitute_contracts::{
    host_updates::{UpdatesRejection, UpdatesRestartTiming},
    update::status::{UpdateActivity, UpdateStatus},
};
use sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue;

/// Asks the coordinator to check for updates now.
pub(super) type CheckRequest<'a> = &'a mut dyn FnMut() -> Result<(), Box<dyn Error>>;

/// One turn's view of the host's updater for the page, and the close the page asked for.
pub(super) struct HostUpdates<'a> {
    status: Option<UpdateStatus>,
    queue: WorkflowQueue,
    closing: bool,
    restart: &'a mut WorkflowRestart,
    check: CheckRequest<'a>,
    close_now: bool,
}

impl<'a> HostUpdates<'a> {
    /// The window's `status`, the backend's workflow `queue`, whether the application is
    /// already `closing`, the restart that may wait for the workflow, and how to ask the
    /// coordinator to `check`.
    pub(super) fn new(
        status: Option<UpdateStatus>,
        queue: WorkflowQueue,
        closing: bool,
        restart: &'a mut WorkflowRestart,
        check: CheckRequest<'a>,
    ) -> Self {
        Self { status, queue, closing, restart, check, close_now: false }
    }

    /// Whether the page asked for a restart that closes the application now.
    pub(super) fn close_now(&self) -> bool {
        self.close_now
    }
}

impl UpdateStatusPort for HostUpdates<'_> {
    fn snapshot(&self) -> Option<UpdateSnapshot> {
        let status = self.status.clone()?;
        Some(UpdateSnapshot { status, restart_after_workflow: self.restart.waiting() })
    }

    fn check_now(&mut self) -> Result<(), UpdatesRejection> {
        if self.closing {
            return Err(UpdatesRejection::Busy);
        }
        let status = self.status.as_ref().ok_or(UpdatesRejection::Unavailable)?;
        if !status.checks() {
            return Err(UpdatesRejection::Unavailable);
        }
        if let UpdateActivity::Checking(_) = status.activity() {
            // The running check answers this request too.
            return Ok(());
        }
        (self.check)().map_err(|cause| {
            tracing::warn!(operation = "update_check_request_failed", %cause,
                "The page's update check could not be sent to the coordinator");
            UpdatesRejection::Unavailable
        })
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
    use sugarsubstitute_contracts::update::{
        Channel,
        attempt::{AttemptOutcome, AttemptPhase, UpdateReport},
        status::RecordedUpdates,
    };

    /// A status from a coordinator that checks when asked.
    fn checking_host() -> UpdateStatus {
        let mut status = UpdateStatus::default();
        status.observe_checks(true);
        status
    }

    fn ready() -> UpdateStatus {
        let mut status = checking_host();
        status.reconcile(RecordedUpdates { prepared: Some("2.0.0".into()), ..Default::default() });
        status
    }

    /// Counts the check requests sent to the coordinator.
    fn counting(sent: &mut usize) -> impl FnMut() -> Result<(), Box<dyn Error>> + '_ {
        move || {
            *sent += 1;
            Ok(())
        }
    }

    #[test]
    fn the_page_sees_the_windows_status_and_whether_a_restart_waits() {
        let mut restart = WorkflowRestart::default();
        let mut sent = 0;
        let mut check = counting(&mut sent);
        let port = HostUpdates::new(None, WorkflowQueue::Idle, false, &mut restart, &mut check);
        assert_eq!(port.snapshot(), None, "nothing to show before the coordinator reports");
        let port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart, &mut check);
        let snapshot = port.snapshot();
        assert_eq!(snapshot.as_ref().and_then(|shown| shown.status.ready()), Some("2.0.0"));
        assert_eq!(snapshot.map(|shown| shown.restart_after_workflow), Some(false));
        assert!(!restart.request(WorkflowQueue::Busy));
        let port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart, &mut check);
        assert_eq!(port.snapshot().map(|shown| shown.restart_after_workflow), Some(true));
        assert!(QueueActivity(WorkflowQueue::Busy).workflow_running());
        assert!(!QueueActivity(WorkflowQueue::Idle).workflow_running());
        assert!(!QueueActivity(WorkflowQueue::Unobserved).workflow_running());
    }

    #[test]
    fn the_page_shows_the_channel_the_coordinator_reports() {
        let mut restart = WorkflowRestart::default();
        let mut check = || Ok::<(), Box<dyn Error>>(());
        let mut status = checking_host();
        status.observe_channel(Channel::Canary);
        let port =
            HostUpdates::new(Some(status), WorkflowQueue::Idle, false, &mut restart, &mut check);
        assert_eq!(port.snapshot().map(|shown| shown.status.channel()), Some(Channel::Canary));
    }

    #[test]
    fn a_check_now_asks_the_coordinator_once_and_joins_a_running_check() {
        let mut restart = WorkflowRestart::default();
        let mut sent = 0;
        let mut check = counting(&mut sent);
        let mut port = HostUpdates::new(
            Some(checking_host()),
            WorkflowQueue::Idle,
            false,
            &mut restart,
            &mut check,
        );
        assert_eq!(port.check_now(), Ok(()));
        let mut running = checking_host();
        running.begin();
        let mut port =
            HostUpdates::new(Some(running), WorkflowQueue::Idle, false, &mut restart, &mut check);
        assert_eq!(port.check_now(), Ok(()), "a running check answers the request");
        drop(port);
        drop(check);
        assert_eq!(sent, 1, "only the first request went to the coordinator");
    }

    #[test]
    fn a_requested_check_shows_checking_then_its_outcome_through_the_coordinators_status()
    -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_application::updates_panel::page_state;
        use sugarsubstitute_contracts::host_updates::{
            UpdatesFailure, UpdatesStatus, UpdatesVersion,
        };
        let version = UpdatesVersion::new("1.0.0")?;
        let shown = |status: UpdateStatus| {
            let mut restart = WorkflowRestart::default();
            let mut check = || Ok::<(), Box<dyn Error>>(());
            let port = HostUpdates::new(
                Some(status),
                WorkflowQueue::Idle,
                false,
                &mut restart,
                &mut check,
            );
            port.snapshot().map(|snapshot| page_state(&version, &snapshot, false).status)
        };
        let mut status = checking_host();
        status.begin();
        assert_eq!(shown(status.clone()), Some(UpdatesStatus::Checking));
        let mut current = status.clone();
        current.finish(Some(AttemptOutcome::Current), 10);
        assert_eq!(shown(current), Some(UpdatesStatus::UpToDate));
        let mut failed = status;
        failed.finish(
            Some(AttemptOutcome::Failed(UpdateReport {
                version: None,
                phase: AttemptPhase::Checking,
                causes: vec!["Update repository transport failed".into()],
            })),
            10,
        );
        assert_eq!(shown(failed), Some(UpdatesStatus::Failed(UpdatesFailure::Check)));
        assert!(matches!(shown(ready()), Some(UpdatesStatus::Ready(_))));
        Ok(())
    }

    #[test]
    fn a_restart_after_the_workflow_waits_for_a_busy_queue_and_closes_at_once_when_idle() {
        let mut restart = WorkflowRestart::default();
        let mut check = || Ok::<(), Box<dyn Error>>(());
        let mut port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart, &mut check);
        assert_eq!(port.restart(UpdatesRestartTiming::AfterWorkflow), Ok(()));
        assert!(!port.close_now(), "a running workflow keeps the application open");
        drop(port);
        assert!(restart.waiting());
        assert!(restart.advance(WorkflowQueue::Idle), "the close starts once the queue is idle");
        let mut port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Idle, false, &mut restart, &mut check);
        assert_eq!(port.restart(UpdatesRestartTiming::AfterWorkflow), Ok(()));
        assert!(port.close_now());
    }

    #[test]
    fn a_restart_now_closes_at_once_even_while_a_workflow_runs() {
        let mut restart = WorkflowRestart::default();
        let mut check = || Ok::<(), Box<dyn Error>>(());
        assert!(!restart.request(WorkflowQueue::Busy));
        let mut port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Busy, false, &mut restart, &mut check);
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Ok(()));
        assert!(port.close_now());
        drop(port);
        assert!(!restart.waiting(), "nothing waits once the close starts");
    }

    #[test]
    fn requests_the_host_cannot_honour_are_declined_with_their_reason() {
        let mut restart = WorkflowRestart::default();
        let mut sent = 0;
        let mut check = counting(&mut sent);
        let mut port = HostUpdates::new(
            Some(checking_host()),
            WorkflowQueue::Idle,
            false,
            &mut restart,
            &mut check,
        );
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Err(UpdatesRejection::NotReady));
        assert!(!port.close_now());
        let mut port = HostUpdates::new(
            Some(UpdateStatus::default()),
            WorkflowQueue::Idle,
            false,
            &mut restart,
            &mut check,
        );
        assert_eq!(
            port.check_now(),
            Err(UpdatesRejection::Unavailable),
            "a launch that cannot check says so"
        );
        let mut port = HostUpdates::new(None, WorkflowQueue::Idle, false, &mut restart, &mut check);
        assert_eq!(port.check_now(), Err(UpdatesRejection::Unavailable));
        let mut port =
            HostUpdates::new(Some(ready()), WorkflowQueue::Idle, true, &mut restart, &mut check);
        assert_eq!(port.restart(UpdatesRestartTiming::Now), Err(UpdatesRejection::Busy));
        assert_eq!(port.check_now(), Err(UpdatesRejection::Busy));
        assert!(!port.close_now());
        drop(port);
        drop(check);
        assert_eq!(sent, 0, "a declined check never reaches the coordinator");
        let mut restart = WorkflowRestart::default();
        let mut unreachable = || Err::<(), Box<dyn Error>>("channel closed".into());
        let mut port = HostUpdates::new(
            Some(checking_host()),
            WorkflowQueue::Idle,
            false,
            &mut restart,
            &mut unreachable,
        );
        assert_eq!(port.check_now(), Err(UpdatesRejection::Unavailable));
    }
}
