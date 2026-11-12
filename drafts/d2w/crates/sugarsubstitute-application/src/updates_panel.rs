// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer the page's `sugarcubes.updates` requests from the host's update status.
//!
//! The page watches the host's update state, asks it to check now and asks it to restart to
//! apply a ready update. This owner answers `watch` with a state carrying a revision, sends a
//! newer state whenever what the page would show changes, and answers every check and restart
//! with accepted or rejected. The updater itself is reached through [`UpdateControls`], so the
//! decisions here do not depend on how the host prepares, checks or relaunches.

use sugarsubstitute_contracts::{
    host_updates::{
        UpdatesChannel, UpdatesClientMessage, UpdatesFailure, UpdatesHostMessage, UpdatesPhase,
        UpdatesRejection, UpdatesReport, UpdatesRestartTiming, UpdatesScheduledRestart,
        UpdatesState, UpdatesStatus, UpdatesVersion, MAX_CAUSE_CHARACTERS, MAX_REPORT_CAUSES,
    },
    update::{
        attempt::{AttemptPhase, UpdateReport},
        status::{UpdateActivity, UpdateStatus},
    },
};

#[cfg(test)]
mod tests;

/// What the host's updater reports at one moment, as the updates protocol needs it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateFacts {
    /// The running version.
    pub current_version: UpdatesVersion,
    /// The channel the host follows.
    pub channel: UpdatesChannel,
    /// The coordinator's update status.
    pub status: UpdateStatus,
    /// A workflow is queued or running in the backend.
    pub workflow_running: bool,
    /// A restart is waiting for the running workflow to finish.
    pub restart_after_workflow: bool,
}

/// The host's updater as the page may use it.
///
/// Calls are made on the owning thread between native callbacks and must return promptly; a
/// request that takes time is accepted now and its result reaches the page as a later state.
pub trait UpdateControls {
    /// The newest facts, or `None` while the host has none to report yet.
    fn facts(&self) -> Option<UpdateFacts>;

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

/// One panel session's updates answers. A new navigation starts a new session.
#[derive(Debug, Default)]
pub struct UpdatesPanelSession {
    watching: bool,
    revision: u64,
    sent: Option<UpdatesState>,
}

impl UpdatesPanelSession {
    /// Answer one page request, in the order the page must receive the answers.
    ///
    /// `watch` is answered with the current state once the host has facts; `check` and
    /// `restart` with accepted or rejected and, when accepting changed what the page shows,
    /// the newer state after it.
    pub fn receive(
        &mut self,
        message: UpdatesClientMessage,
        controls: &mut dyn UpdateControls,
    ) -> Vec<UpdatesHostMessage> {
        let mut answers = Vec::new();
        match message {
            UpdatesClientMessage::Watch(_) => {
                self.watching = true;
                // A repeated watch is answered again, so the page never waits on a lost state.
                self.sent = None;
            }
            UpdatesClientMessage::Check { request, .. } => {
                answers.push(match controls.check_now() {
                    Ok(()) => UpdatesHostMessage::Accepted(request),
                    Err(reason) => UpdatesHostMessage::Rejected { request, reason },
                });
            }
            UpdatesClientMessage::Restart { request, when, .. } => {
                answers.push(match controls.restart(when) {
                    Ok(()) => UpdatesHostMessage::Accepted(request),
                    Err(reason) => UpdatesHostMessage::Rejected { request, reason },
                });
            }
        }
        answers.extend(self.refresh(&*controls));
        answers
    }

    /// Send a newer state when what a watching page shows has changed.
    pub fn refresh(&mut self, controls: &dyn UpdateControls) -> Option<UpdatesHostMessage> {
        if !self.watching {
            return None;
        }
        let state = page_state(&controls.facts()?);
        if self.sent.as_ref() == Some(&state) {
            return None;
        }
        self.revision += 1;
        self.sent = Some(state.clone());
        Some(UpdatesHostMessage::State { revision: self.revision, update: Box::new(state) })
    }
}

/// Translate the updater's facts into the state the page shows.
///
/// A running check shows as checking; otherwise a ready update outranks a failure, since the
/// user can still apply it.
pub fn page_state(facts: &UpdateFacts) -> UpdatesState {
    let status = &facts.status;
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
        current_version: facts.current_version.clone(),
        channel: facts.channel,
        last_successful_check_ms: status
            .last_successful_check_utc()
            .and_then(|seconds| u64::try_from(seconds).ok())
            .and_then(|seconds| seconds.checked_mul(1000)),
        status: page_status,
        report: failure.and_then(page_report),
        previous_version_running: status.previous_version_running(),
        workflow_running: facts.workflow_running,
        scheduled_restart: if facts.restart_after_workflow {
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
