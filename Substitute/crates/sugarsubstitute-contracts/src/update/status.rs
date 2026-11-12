// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the host tells the user about application updates: checking, up to date, ready,
//! couldn't check, or that the previous version is running.
//!
//! Attempts report what happened; the selection record decides what is prepared. `reconcile`
//! adopts the record's state, so a status survives helper crashes, repeated attempts and
//! restarts without trusting an attempt's claim alone.
use super::attempt::{
    AttemptOutcome, AttemptPhase, AttemptProgress, MAX_VERSION_BYTES, UpdateReport, codec,
};
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
}
impl UpdateStatus {
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
            && self.failure.as_ref().is_none_or(codec::valid_report)
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
            previous_version_running: false,
        });
        assert_eq!(status.ready(), Some("2.0.0"));
        status.reconcile(RecordedUpdates { prepared: None, previous_version_running: true });
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
        });
        assert_eq!(UpdateStatus::decode(&status.encode()?)?, status);
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
}
