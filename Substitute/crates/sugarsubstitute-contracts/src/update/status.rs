// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the host tells the user about application updates: checking, up to date, ready,
//! couldn't check, or that the previous version is running. The same status carries the
//! start's SugarCubes update, so the splash, the caption and the report agree on it.
//!
//! Attempts report what happened; the selection record decides what is prepared. `reconcile`
//! adopts the record's state, so a status survives helper crashes, repeated attempts and
//! restarts without trusting an attempt's claim alone.
use super::Channel;
use super::attempt::{
    AttemptOutcome, AttemptPhase, AttemptProgress, MAX_VERSION_BYTES, UpdateReport, codec,
};
use super::pack::{PackUpdateReport, PackUpdateStatus};
use serde::{Deserialize, Serialize};

/// Largest encoded status: twelve bounded causes even when every byte needs escaping.
pub const MAX_STATUS_BYTES: usize = 32 * 1024;

/// Whether an attempt is running now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateActivity {
    /// No attempt is running.
    Idle,
    /// An attempt is running; the latest progress it reported.
    Checking(AttemptProgress),
}

/// Prepared and running versions as the selection record states them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedUpdates {
    /// Version of a prepared candidate that activates at the next launch.
    pub prepared: Option<String>,
    /// The accepted version was rolled back; the previous version is running.
    pub previous_version_running: bool,
    /// The newer version's files were found damaged, so the previous version is running while
    /// an update check downloads it again. This is not a rollback.
    #[serde(default)]
    pub repairing: bool,
}

/// What this launch did about a version it starts for the first time, as the coordinator
/// observed it. Only the coordinator that started that version knows it; the record does not.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LaunchVersion {
    /// This launch is starting this version for the first time, and it is not yet confirmed.
    Applying(String),
    /// This launch started this version for the first time and confirmed it.
    Updated(String),
    /// This launch prepared this version before its window opened and restarts into it; the
    /// window stays hidden and the splash says which version it updates to.
    Restarting(String),
}

/// A one-time message about updates for the start of the application, in the order a window
/// shows them. A window shows each at most once per launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupNotice {
    /// This is the first start on this version.
    Updated(String),
    /// The previous version is running while the newer one's damaged files are downloaded again.
    Repairing,
    /// The newer version did not start, so the previous one is running.
    PreviousVersionRunning,
    /// The start's SugarCubes update finished with a report that has a line to show.
    Sugarcubes(PackUpdateReport),
}

/// One host's update status for its update surfaces. The coordinator owns it; the desktop child
/// receives copies through [`UpdateStatus::encode`] and [`UpdateStatus::decode`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateStatus {
    activity: Option<AttemptProgress>,
    last_successful_check_utc: Option<i64>,
    recorded: RecordedUpdates,
    needs_installer: bool,
    failure: Option<UpdateReport>,
    #[serde(default)]
    launch: Option<LaunchVersion>,
    #[serde(default)]
    sugarcubes: PackUpdateStatus,
    #[serde(default)]
    channel: Channel,
    #[serde(default)]
    checks: bool,
    #[serde(default)]
    restart_covered: bool,
    #[serde(default)]
    running: Option<String>,
}
impl UpdateStatus {
    /// The channel the installation's subscription follows; a new installation follows
    /// releases.
    pub fn channel(&self) -> Channel {
        self.channel
    }
    /// The version this launch runs, as the installation records it; `None` for a run without
    /// an installation.
    pub fn running(&self) -> Option<&str> {
        self.running.as_deref()
    }
    /// Whether the coordinator checks for updates when asked now. A build without update
    /// configuration, or a launch that does not own the installation, never checks.
    pub fn checks(&self) -> bool {
        self.checks
    }
    /// Current activity.
    pub fn activity(&self) -> UpdateActivity {
        self.activity.map_or(UpdateActivity::Idle, UpdateActivity::Checking)
    }
    /// Unix UTC seconds of the last check that finished, whatever it found.
    pub fn last_successful_check_utc(&self) -> Option<i64> {
        self.last_successful_check_utc
    }
    /// Version ready to apply on restart, per the selection record.
    pub fn ready(&self) -> Option<&str> {
        self.recorded.prepared.as_deref()
    }
    /// The previous version is running after a rollback, per the selection record.
    pub fn previous_version_running(&self) -> bool {
        self.recorded.previous_version_running
    }
    /// The previous version is running while the newer one is repaired, per the selection record.
    pub fn repairing(&self) -> bool {
        self.recorded.repairing
    }
    /// The version this launch is starting for the first time, until it is confirmed, or the
    /// version it restarts into.
    pub fn applying(&self) -> Option<&str> {
        match &self.launch {
            Some(LaunchVersion::Applying(version) | LaunchVersion::Restarting(version)) => {
                Some(version)
            }
            Some(LaunchVersion::Updated(_)) | None => None,
        }
    }
    /// Whether this launch restarts into an update prepared before its window opened, so the
    /// window must stay hidden.
    pub fn restarting(&self) -> bool {
        matches!(self.launch, Some(LaunchVersion::Restarting(_)))
    }
    /// Whether a restarting window may close now: the coordinator's own "Updating to
    /// version X…" notice is on screen to stay until the next copy shows, or it cannot show one.
    pub fn restart_covered(&self) -> bool {
        self.restarting() && self.restart_covered
    }
    /// The version this launch started for the first time and confirmed.
    pub fn updated_to(&self) -> Option<&str> {
        match &self.launch {
            Some(LaunchVersion::Updated(version)) => Some(version),
            Some(LaunchVersion::Applying(_) | LaunchVersion::Restarting(_)) | None => None,
        }
    }
    /// The one-time start-up messages this status states, in the order a window shows them. A
    /// repair already says the previous version runs, so it replaces the rollback line.
    pub fn startup_notices(&self) -> Vec<StartupNotice> {
        let mut notices = Vec::new();
        if let Some(version) = self.updated_to() {
            notices.push(StartupNotice::Updated(version.to_owned()));
        }
        if self.recorded.repairing {
            notices.push(StartupNotice::Repairing);
        } else if self.recorded.previous_version_running {
            notices.push(StartupNotice::PreviousVersionRunning);
        }
        if let Some(report) = self.sugarcubes.report()
            && report.notice_template(&crate::localization::pack_updates::EN).is_some()
        {
            notices.push(StartupNotice::Sugarcubes(report.clone()));
        }
        notices
    }
    /// The start's SugarCubes update: running, finished with its report, or not run.
    pub fn sugarcubes(&self) -> &PackUpdateStatus {
        &self.sugarcubes
    }
    /// The latest release needs the installer.
    pub fn needs_installer(&self) -> bool {
        self.needs_installer
    }
    /// Report of the most recent failed attempt, until a later attempt finishes.
    pub fn failure(&self) -> Option<&UpdateReport> {
        self.failure.as_ref()
    }

    /// An attempt started.
    pub fn begin(&mut self) {
        self.activity = Some(AttemptProgress { phase: AttemptPhase::Opening, bytes: None });
    }
    /// Adopt live progress of the running attempt; progress without an attempt is ignored.
    pub fn observe(&mut self, progress: AttemptProgress) {
        if self.activity.is_some() {
            self.activity = Some(progress);
        }
    }
    /// The attempt ended at `now_utc`. A missing result is a failure at the last phase seen.
    /// Whether an update is ready still comes from `reconcile`.
    pub fn finish(&mut self, outcome: Option<AttemptOutcome>, now_utc: i64) {
        let phase = self.activity.map_or(AttemptPhase::Opening, |progress| progress.phase);
        self.activity = None;
        match outcome {
            Some(AttemptOutcome::Failed(report)) => self.failure = Some(report),
            None => {
                self.failure = Some(UpdateReport {
                    version: None,
                    phase,
                    causes: vec!["The update helper ended without reporting a result".into()],
                });
            }
            Some(AttemptOutcome::AlreadyPreparing) => {}
            Some(outcome) => {
                self.failure = None;
                self.last_successful_check_utc = Some(now_utc);
                self.needs_installer = outcome == AttemptOutcome::NeedsInstaller;
            }
        }
    }
    /// Adopt the selection record's prepared and rollback state.
    pub fn reconcile(&mut self, recorded: RecordedUpdates) {
        self.recorded = recorded;
    }
    /// Adopt the coordinator's SugarCubes update state.
    pub fn observe_sugarcubes(&mut self, status: PackUpdateStatus) {
        self.sugarcubes = status;
    }
    /// Adopt the channel the installation's subscription follows.
    pub fn observe_channel(&mut self, channel: Channel) {
        self.channel = channel;
    }
    /// Adopt the version this launch runs.
    pub fn observe_running(&mut self, running: Option<String>) {
        self.running = running;
    }
    /// Adopt whether the coordinator checks for updates when asked now.
    pub fn observe_checks(&mut self, checks: bool) {
        self.checks = checks;
    }
    /// Adopt what this launch observed about a version it starts for the first time. A
    /// confirmed version is never taken back by a later `Applying`.
    pub fn observe_launch(&mut self, launch: LaunchVersion) {
        if !matches!(self.launch, Some(LaunchVersion::Updated(_))) {
            self.launch = Some(launch);
        }
    }
    /// Take back a restart this launch no longer makes: its window opened first, so the update
    /// starts on the next launch.
    pub fn withdraw_restart(&mut self) {
        if self.restarting() {
            self.launch = None;
            self.restart_covered = false;
        }
    }
    /// Adopt whether the coordinator's restart notice covers the screen, so a restarting window
    /// may close without leaving the screen empty.
    pub fn observe_restart_cover(&mut self, covered: bool) {
        self.restart_covered = covered;
    }

    /// Encode for another process after checking the bounds `decode` enforces.
    /// # Errors
    /// Rejects out-of-bounds versions, reports and progress, and encodings over
    /// [`MAX_STATUS_BYTES`].
    pub fn encode(&self) -> Result<Vec<u8>, StatusCodecError> {
        if !self.valid() {
            return Err(StatusCodecError::Bounds);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| StatusCodecError::Malformed)?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusCodecError::Oversized);
        }
        Ok(bytes)
    }

    /// Decode a status from another process; nothing it claims is trusted beyond display.
    /// # Errors
    /// Rejects oversized or malformed bytes, unknown fields and out-of-bounds content.
    pub fn decode(bytes: &[u8]) -> Result<Self, StatusCodecError> {
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusCodecError::Oversized);
        }
        let status: Self =
            serde_json::from_slice(bytes).map_err(|_| StatusCodecError::Malformed)?;
        if status.valid() { Ok(status) } else { Err(StatusCodecError::Bounds) }
    }

    fn valid(&self) -> bool {
        let version = |version: &String| !version.is_empty() && version.len() <= MAX_VERSION_BYTES;
        self.activity
            .is_none_or(|progress| progress.bytes.is_none_or(|(received, total)| received <= total))
            && self.recorded.prepared.as_ref().is_none_or(version)
            && self.running.as_ref().is_none_or(version)
            && self.launch.as_ref().is_none_or(|launch| match launch {
                LaunchVersion::Applying(value)
                | LaunchVersion::Updated(value)
                | LaunchVersion::Restarting(value) => version(value),
            })
            && self.failure.as_ref().is_none_or(codec::valid_report)
            && self.sugarcubes.valid()
    }
}

/// An encoded status was oversized, malformed or out of bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusCodecError {
    /// The encoding exceeds [`MAX_STATUS_BYTES`].
    Oversized,
    /// The bytes are not an encoded status.
    Malformed,
    /// A field exceeds its bound or contradicts another field.
    Bounds,
}
impl std::fmt::Display for StatusCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Oversized => "Update status is oversized",
            Self::Malformed => "Update status is malformed",
            Self::Bounds => "Update status exceeds its bounds",
        })
    }
}
impl std::error::Error for StatusCodecError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_check_clears_failure_and_records_its_time() {
        let mut status = UpdateStatus::default();
        status.begin();
        assert_eq!(
            status.activity(),
            UpdateActivity::Checking(AttemptProgress { phase: AttemptPhase::Opening, bytes: None })
        );
        status.finish(None, 10);
        assert_eq!(status.failure().map(|report| report.phase), Some(AttemptPhase::Opening));
        assert_eq!(status.last_successful_check_utc(), None);
        status.begin();
        status.finish(Some(AttemptOutcome::Current), 20);
        assert_eq!(status.activity(), UpdateActivity::Idle);
        assert_eq!(status.failure(), None);
        assert_eq!(status.last_successful_check_utc(), Some(20));
    }

    #[test]
    fn readiness_comes_from_the_record_not_from_an_attempt_claim() {
        let mut status = UpdateStatus::default();
        status.begin();
        status.finish(Some(AttemptOutcome::Prepared { version: "2.0.0".into() }), 5);
        assert_eq!(status.ready(), None);
        status.reconcile(RecordedUpdates {
            prepared: Some("2.0.0".into()),
            ..RecordedUpdates::default()
        });
        assert_eq!(status.ready(), Some("2.0.0"));
        status.reconcile(RecordedUpdates {
            previous_version_running: true,
            ..RecordedUpdates::default()
        });
        assert_eq!(status.ready(), None);
        assert!(status.previous_version_running());
    }

    #[test]
    fn a_failure_keeps_the_last_good_check_and_progress_follows_the_attempt() {
        let mut status = UpdateStatus::default();
        status.observe(AttemptProgress { phase: AttemptPhase::Checking, bytes: None });
        assert_eq!(status.activity(), UpdateActivity::Idle);
        status.begin();
        status.finish(Some(AttemptOutcome::NeedsInstaller), 7);
        assert!(status.needs_installer());
        status.begin();
        let downloading = AttemptProgress { phase: AttemptPhase::Downloading, bytes: Some((1, 2)) };
        status.observe(downloading);
        assert_eq!(status.activity(), UpdateActivity::Checking(downloading));
        let report = UpdateReport {
            version: Some("2.0.0".into()),
            phase: AttemptPhase::Downloading,
            causes: vec!["Update repository transport failed".into()],
        };
        status.finish(Some(AttemptOutcome::Failed(report.clone())), 9);
        assert_eq!(status.failure(), Some(&report));
        assert_eq!(status.last_successful_check_utc(), Some(7));
        assert!(status.needs_installer());
    }

    #[test]
    fn a_status_crosses_processes_only_within_its_bounds() -> Result<(), StatusCodecError> {
        let mut status = UpdateStatus::default();
        status.begin();
        status.finish(
            Some(AttemptOutcome::Failed(UpdateReport {
                version: Some("2.0.0".into()),
                phase: AttemptPhase::Downloading,
                causes: vec!["The download stopped".into()],
            })),
            9,
        );
        status.reconcile(RecordedUpdates {
            prepared: Some("1.9.0".into()),
            previous_version_running: true,
            repairing: true,
        });
        status.observe_launch(LaunchVersion::Applying("1.8.0".into()));
        assert_eq!(UpdateStatus::decode(&status.encode()?)?, status);
        status.observe_launch(LaunchVersion::Updated("v".repeat(MAX_VERSION_BYTES + 1)));
        assert_eq!(status.encode(), Err(StatusCodecError::Bounds));
        status.launch = None;
        status.recorded.prepared = Some("v".repeat(MAX_VERSION_BYTES + 1));
        assert_eq!(status.encode(), Err(StatusCodecError::Bounds));
        assert_eq!(UpdateStatus::decode(b"{\"unknown\":1}"), Err(StatusCodecError::Malformed));
        assert_eq!(
            UpdateStatus::decode(&vec![b' '; MAX_STATUS_BYTES + 1]),
            Err(StatusCodecError::Oversized)
        );
        let forged = br#"{"activity":{"phase":"downloading","bytes":[5,1]},"last_successful_check_utc":null,"recorded":{"prepared":null,"previous_version_running":false},"needs_installer":false,"failure":null}"#;
        assert_eq!(UpdateStatus::decode(forged), Err(StatusCodecError::Bounds));
        Ok(())
    }

    #[test]
    fn the_sugarcubes_update_crosses_processes_within_bounds_and_states_its_line_once()
    -> Result<(), StatusCodecError> {
        use crate::update::pack::{PackOutcome, PackUpdateReport};
        let mut status = UpdateStatus::default();
        status.observe_sugarcubes(PackUpdateStatus::Updating);
        assert_eq!(UpdateStatus::decode(&status.encode()?)?, status);
        assert!(status.startup_notices().is_empty(), "nothing to say while it runs");
        let could_not_check = PackUpdateReport {
            unchecked: vec!["registry".into(), "github-release".into()],
            ..PackUpdateReport::new(PackOutcome::CouldNotCheck)
        };
        status.observe_sugarcubes(PackUpdateStatus::Finished(could_not_check.clone()));
        assert_eq!(UpdateStatus::decode(&status.encode()?)?, status);
        assert_eq!(status.startup_notices(), [StartupNotice::Sugarcubes(could_not_check)]);
        status.observe_sugarcubes(PackUpdateStatus::Finished(PackUpdateReport::new(
            PackOutcome::UpToDate,
        )));
        assert!(status.startup_notices().is_empty(), "up to date has no line");
        status.observe_sugarcubes(PackUpdateStatus::Finished(PackUpdateReport {
            to: Some("v".repeat(65)),
            ..PackUpdateReport::new(PackOutcome::Updated)
        }));
        assert_eq!(status.encode(), Err(StatusCodecError::Bounds));
        Ok(())
    }

    #[test]
    fn the_running_version_crosses_processes_within_its_bound() -> Result<(), StatusCodecError> {
        let mut status = UpdateStatus::default();
        assert_eq!(status.running(), None);
        status.observe_running(Some("0.2.0".into()));
        assert_eq!(UpdateStatus::decode(&status.encode()?)?.running(), Some("0.2.0"));
        status.observe_running(Some("v".repeat(MAX_VERSION_BYTES + 1)));
        assert_eq!(status.encode(), Err(StatusCodecError::Bounds));
        Ok(())
    }

    #[test]
    fn the_channel_and_whether_checks_run_cross_processes_with_the_status()
    -> Result<(), StatusCodecError> {
        let mut status = UpdateStatus::default();
        assert_eq!((status.channel(), status.checks()), (Channel::Release, false));
        status.observe_channel(Channel::Canary);
        status.observe_checks(true);
        let crossed = UpdateStatus::decode(&status.encode()?)?;
        assert_eq!((crossed.channel(), crossed.checks()), (Channel::Canary, true));
        assert_eq!(crossed, status);
        Ok(())
    }

    #[test]
    fn a_launch_applies_then_confirms_a_version_and_states_its_notices_once_in_order() {
        let mut status = UpdateStatus::default();
        assert!(status.startup_notices().is_empty());
        status.observe_launch(LaunchVersion::Applying("2.0.0".into()));
        assert_eq!((status.applying(), status.updated_to()), (Some("2.0.0"), None));
        assert!(status.startup_notices().is_empty(), "an unconfirmed version is no update yet");
        status.observe_launch(LaunchVersion::Updated("2.0.0".into()));
        status.observe_launch(LaunchVersion::Applying("2.0.0".into()));
        assert_eq!((status.applying(), status.updated_to()), (None, Some("2.0.0")));
        assert_eq!(status.startup_notices(), [StartupNotice::Updated("2.0.0".into())]);
        status.reconcile(RecordedUpdates {
            previous_version_running: true,
            ..RecordedUpdates::default()
        });
        assert_eq!(status.startup_notices()[1], StartupNotice::PreviousVersionRunning);
        status.reconcile(RecordedUpdates {
            previous_version_running: true,
            repairing: true,
            ..RecordedUpdates::default()
        });
        assert_eq!(
            status.startup_notices(),
            [StartupNotice::Updated("2.0.0".into()), StartupNotice::Repairing],
            "the repair line already says the previous version runs"
        );
    }

    #[test]
    fn a_restart_says_its_version_holds_the_window_and_can_be_withdrawn()
    -> Result<(), StatusCodecError> {
        let mut status = UpdateStatus::default();
        status.observe_launch(LaunchVersion::Restarting("2.0.0".into()));
        assert!(status.restarting());
        assert_eq!((status.applying(), status.updated_to()), (Some("2.0.0"), None));
        assert!(status.startup_notices().is_empty());
        assert!(!status.restart_covered(), "the window closes only once the restart is covered");
        status.observe_restart_cover(true);
        assert!(status.restart_covered());
        assert_eq!(UpdateStatus::decode(&status.encode()?)?, status);
        status.withdraw_restart();
        assert!(!status.restart_covered());
        assert!(!status.restarting());
        assert_eq!(status.applying(), None);
        status.observe_launch(LaunchVersion::Applying("2.0.0".into()));
        status.withdraw_restart();
        assert_eq!(status.applying(), Some("2.0.0"), "only a restart is withdrawn");
        Ok(())
    }
}
