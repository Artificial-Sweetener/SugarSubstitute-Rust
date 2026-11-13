// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer the page's `sugarcubes.updates` requests from the host's update status.
//!
//! The page watches the host's update state, asks it to check now and asks it to restart to
//! apply a ready update. This owner answers `watch` with a state carrying a revision, sends a
//! newer state whenever what the page would show changes, and answers every check and restart
//! with accepted or rejected. It reaches the updater only through [`UpdateStatusPort`] and the
//! running workflow only through [`WorkflowActivitySource`], so these decisions do not depend on
//! how the host prepares, checks or relaunches, or how it observes its backend.

use sugarsubstitute_contracts::{
    host_updates::{
        MAX_CAUSE_CHARACTERS, MAX_REPORT_CAUSES, UpdatesChannel, UpdatesClientMessage,
        UpdatesFailure, UpdatesHostMessage, UpdatesPhase, UpdatesRejection, UpdatesReport,
        UpdatesRestartTiming, UpdatesScheduledRestart, UpdatesState, UpdatesStatus, UpdatesVersion,
    },
    update::{
        Channel,
        attempt::{AttemptPhase, UpdateReport},
        status::{UpdateActivity, UpdateStatus},
    },
};

#[cfg(test)]
mod tests;

/// What the updater reports at one moment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateSnapshot {
    /// The coordinator's update status.
    pub status: UpdateStatus,
    /// The channel the installation's subscription follows; a new installation follows
    /// releases.
    pub channel: Channel,
    /// A restart is waiting for the running workflow to finish.
    pub restart_after_workflow: bool,
}

/// The host's updater as the page may use it.
///
/// Calls are made on the owning thread between native callbacks and must return promptly; a
/// request that takes time is accepted now and its result reaches the page as a later state.
pub trait UpdateStatusPort {
    /// The newest snapshot, or `None` while the host has none to report yet.
    fn snapshot(&self) -> Option<UpdateSnapshot>;

    /// Ask the updater to check now.
    ///
    /// # Errors
    /// Returns why the host declines: `Busy` while it is already checking or closing,
    /// `Unavailable` when it cannot check.
    fn check_now(&mut self) -> Result<(), UpdatesRejection>;

    /// Ask the host to apply the ready update by its normal close and relaunch, now or once the
    /// running workflow finishes.
    ///
    /// # Errors
    /// Returns why the host declines: `NotReady` without a ready update, `Busy` while it is
    /// already closing, `Unavailable` when it cannot restart.
    fn restart(&mut self, when: UpdatesRestartTiming) -> Result<(), UpdatesRejection>;
}

/// Whether the backend is running a workflow, which a restart can wait for.
pub trait WorkflowActivitySource {
    /// A workflow is queued or running.
    fn workflow_running(&self) -> bool;
}

/// The host's update status before an updater reports one: up to date and idle.
///
/// It never claims a check or a ready update it cannot back: a check is refused as
/// unavailable, and a restart as not ready, because there is nothing to apply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IdleUpdateStatus;

impl UpdateStatusPort for IdleUpdateStatus {
    fn snapshot(&self) -> Option<UpdateSnapshot> {
        Some(UpdateSnapshot::default())
    }

    fn check_now(&mut self) -> Result<(), UpdatesRejection> {
        Err(UpdatesRejection::Unavailable)
    }

    fn restart(&mut self, _when: UpdatesRestartTiming) -> Result<(), UpdatesRejection> {
        Err(UpdatesRejection::NotReady)
    }
}

/// Workflow activity before the host observes its backend's queue: no workflow is running.
///
/// A restart therefore never waits for a workflow; the page is told none is running.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IdleWorkflowActivity;

impl WorkflowActivitySource for IdleWorkflowActivity {
    fn workflow_running(&self) -> bool {
        false
    }
}

/// One panel session's updates answers. A new navigation starts a new session.
#[derive(Debug)]
pub struct UpdatesPanelSession {
    current_version: UpdatesVersion,
    watching: bool,
    revision: u64,
    sent: Option<UpdatesState>,
}

impl UpdatesPanelSession {
    /// Start a session that reports `current_version` as the running version.
    pub const fn new(current_version: UpdatesVersion) -> Self {
        Self { current_version, watching: false, revision: 0, sent: None }
    }

    /// Answer one page request, in the order the page must receive the answers.
    ///
    /// `watch` is answered with the current state once the updater has a snapshot; `check` and
    /// `restart` with accepted or rejected and, when that changed what the page shows, the
    /// newer state after it.
    pub fn receive(
        &mut self,
        message: UpdatesClientMessage,
        updater: &mut dyn UpdateStatusPort,
        workflow: &dyn WorkflowActivitySource,
    ) -> Vec<UpdatesHostMessage> {
        let mut answers = Vec::new();
        match message {
            UpdatesClientMessage::Watch(_) => {
                self.watching = true;
                // A repeated watch is answered again, so the page never waits on a lost state.
                self.sent = None;
            }
            UpdatesClientMessage::Check { request, .. } => {
                answers.push(match updater.check_now() {
                    Ok(()) => UpdatesHostMessage::Accepted(request),
                    Err(reason) => UpdatesHostMessage::Rejected { request, reason },
                });
            }
            UpdatesClientMessage::Restart { request, when, .. } => {
                answers.push(match updater.restart(when) {
                    Ok(()) => UpdatesHostMessage::Accepted(request),
                    Err(reason) => UpdatesHostMessage::Rejected { request, reason },
                });
            }
        }
        answers.extend(self.refresh(&*updater, workflow));
        answers
    }

    /// Send a newer state when what a watching page shows has changed.
    pub fn refresh(
        &mut self,
        updater: &dyn UpdateStatusPort,
        workflow: &dyn WorkflowActivitySource,
    ) -> Option<UpdatesHostMessage> {
        if !self.watching {
            return None;
        }
        let state =
            page_state(&self.current_version, &updater.snapshot()?, workflow.workflow_running());
        if self.sent.as_ref() == Some(&state) {
            return None;
        }
        self.revision += 1;
        self.sent = Some(state.clone());
        Some(UpdatesHostMessage::State { revision: self.revision, update: Box::new(state) })
    }
}

/// Translate the updater's snapshot into the state the page shows.
///
/// A running check shows as checking; otherwise a ready update outranks a failure, since the
/// user can still apply it.
pub fn page_state(
    current_version: &UpdatesVersion,
    snapshot: &UpdateSnapshot,
    workflow_running: bool,
) -> UpdatesState {
    let status = &snapshot.status;
    let failure = status.failure();
    let page_status = if let UpdateActivity::Checking(_) = status.activity() {
        UpdatesStatus::Checking
    } else if let Some(version) = status.ready().and_then(page_version) {
        UpdatesStatus::Ready(version)
    } else if let Some(report) = failure {
        UpdatesStatus::Failed(if checking(report.phase) {
            UpdatesFailure::Check
        } else {
            UpdatesFailure::Update
        })
    } else {
        UpdatesStatus::UpToDate
    };
    UpdatesState {
        current_version: current_version.clone(),
        channel: match snapshot.channel {
            Channel::Release => UpdatesChannel::Stable,
            Channel::Canary => UpdatesChannel::Beta,
        },
        last_successful_check_ms: status
            .last_successful_check_utc()
            .and_then(|seconds| u64::try_from(seconds).ok())
            .and_then(|seconds| seconds.checked_mul(1000)),
        status: page_status,
        report: failure.and_then(page_report),
        previous_version_running: status.previous_version_running(),
        workflow_running,
        scheduled_restart: if snapshot.restart_after_workflow {
            UpdatesScheduledRestart::AfterWorkflow
        } else {
            UpdatesScheduledRestart::None
        },
    }
}

/// Whether a failure at `phase` means the host couldn't check, rather than couldn't update.
const fn checking(phase: AttemptPhase) -> bool {
    matches!(phase, AttemptPhase::Opening | AttemptPhase::Checking | AttemptPhase::CheckingUpdater)
}

/// A release label the page can show; labels outside the schema's token are not shown.
fn page_version(label: &str) -> Option<UpdatesVersion> {
    UpdatesVersion::new(&label.to_ascii_lowercase()).ok()
}

/// The failure report as the page shows it, when it names a version.
fn page_report(report: &UpdateReport) -> Option<UpdatesReport> {
    let version = report.version.as_deref().and_then(page_version)?;
    let phase = match report.phase {
        AttemptPhase::Opening | AttemptPhase::Checking | AttemptPhase::CheckingUpdater => {
            UpdatesPhase::Check
        }
        AttemptPhase::Downloading => UpdatesPhase::Download,
        AttemptPhase::Preparing | AttemptPhase::PreparingUpdater => UpdatesPhase::Install,
        AttemptPhase::Applying => UpdatesPhase::Start,
    };
    let causes = report
        .causes
        .iter()
        .take(MAX_REPORT_CAUSES)
        .map(|cause| cause.chars().take(MAX_CAUSE_CHARACTERS).collect())
        .collect();
    UpdatesReport::new(version, phase, causes).ok()
}
