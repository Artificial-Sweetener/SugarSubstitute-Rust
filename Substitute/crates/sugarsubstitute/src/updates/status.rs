// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the update control shows. Activity and the last result come from the attempt channel;
//! whether an update is ready, or the previous version is running, comes only from the selection
//! record, and the channel only from the installation's subscription, both read through the
//! storage owner at start and after every check. Each change raises the
//! revision, so a consumer that receives snapshots out of order keeps the newest.
use std::{io, path::PathBuf};
use sugarsubstitute_contracts::{
    execution::{
        ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Moment, Operation, OwnedJob, Scope,
    },
    update::{
        Channel,
        attempt::{AttemptOutcome, AttemptPhase, AttemptProgress, UpdateReport},
        status::{LaunchVersion, RecordedUpdates, UpdateStatus},
    },
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::setup_storage::{
    self, SelectionFailure, TrustStore, UpdateSelection, read_update_selection,
};

/// Bound of one selection read; the read takes only the shared record lock.
const READ: std::time::Duration = std::time::Duration::from_secs(30);

/// The coordinator's one update status, with the revision of its last change.
pub(crate) struct StatusSource {
    status: UpdateStatus,
    revision: u64,
    stale: bool,
    reading: Option<OwnedJob<StoredUpdates, io::Error>>,
}

/// What one storage read found: the record's state and, when the installation has update
/// trust and its subscription could be read, the channel it follows.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct StoredUpdates {
    recorded: RecordedUpdates,
    channel: Option<Channel>,
}
impl StatusSource {
    /// No check has run and the record has not been read yet.
    pub(crate) fn new() -> Self {
        Self { status: UpdateStatus::default(), revision: 0, stale: true, reading: None }
    }

    /// The current status and the revision of its last change.
    pub(crate) fn snapshot(&self) -> (u64, &UpdateStatus) {
        (self.revision, &self.status)
    }

    /// A check started.
    pub(crate) fn begin(&mut self) {
        self.change(UpdateStatus::begin);
    }

    /// The running check reported progress.
    pub(crate) fn observe(&mut self, progress: AttemptProgress) {
        self.change(|status| status.observe(progress));
    }

    /// The running check set untrusted update storage aside and set updates up again.
    pub(crate) fn observe_storage_set_aside(&mut self) {
        self.change(UpdateStatus::observe_storage_set_aside);
    }

    /// Adopt the coordinator's SugarCubes update state.
    pub(crate) fn observe_sugarcubes(
        &mut self,
        status: sugarsubstitute_contracts::update::pack::PackUpdateStatus,
    ) {
        if self.status.sugarcubes() != &status {
            self.change(|current| current.observe_sugarcubes(status));
        }
    }

    /// Take back a restart this launch no longer makes.
    pub(crate) fn withdraw_restart(&mut self) {
        self.change(UpdateStatus::withdraw_restart);
    }

    /// Adopt whether this run's restart notice covers the screen.
    pub(crate) fn observe_restart_cover(&mut self, covered: bool) {
        self.change(|status| status.observe_restart_cover(covered));
    }

    /// Adopt what this launch observed about the version it starts for the first time.
    pub(crate) fn observe_launch(&mut self, launch: LaunchVersion) {
        self.change(|status| status.observe_launch(launch));
    }

    /// A check could not be admitted, so it ended before it started: the user sees that it
    /// couldn't check, and the cause stays in the report.
    pub(crate) fn not_started(&mut self, now_utc: i64) {
        self.change(|status| {
            status.begin();
            status.finish(
                Some(AttemptOutcome::Failed(UpdateReport {
                    version: None,
                    phase: AttemptPhase::Opening,
                    causes: vec!["The update check could not start".into()],
                })),
                now_utc,
            );
        });
    }

    /// Adopt the version this launch runs.
    pub(crate) fn observe_running(&mut self, running: Option<String>) {
        self.change(|status| status.observe_running(running));
    }

    /// Adopt whether a check runs when asked now.
    pub(crate) fn observe_checks(&mut self, checks: bool) {
        self.change(|status| status.observe_checks(checks));
    }

    /// A check ended with `outcome` at `now_utc` seconds. The record is read again, because a
    /// check may have prepared an update.
    pub(crate) fn finish(&mut self, outcome: AttemptOutcome, now_utc: i64) {
        self.change(|status| status.finish(Some(outcome), now_utc));
        self.stale = true;
    }

    /// Adopt a finished read. A failed read keeps the last known record and is logged; the next
    /// check reads again.
    pub(crate) fn poll(&mut self) {
        let Some(done) = self.reading.as_mut().and_then(OwnedJob::try_take_completion) else {
            return;
        };
        self.reading = None;
        match done.outcome {
            JobOutcome::Success(stored) => self.adopt(stored),
            JobOutcome::Failure(cause) => {
                tracing::warn!(operation = "update_record_read_failed", cause = %cause,
                    "The update record could not be read; the last known state stays shown");
            }
            other => tracing::warn!(operation = "update_record_read_stopped",
                outcome = ?other.kind(), "The update record read stopped"),
        }
    }

    /// Admit one read of `installation`'s record as job `id`, when `wants_read`.
    /// # Errors
    /// Returns execution admission and clock failures; the read stays wanted.
    pub(crate) fn submit_read(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        id: JobId,
        installation: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline: Moment = client.now()?.checked_add(READ)?;
        let spec = JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(deadline),
            reservation_bytes: setup_storage::RESERVATION_BYTES,
            replacement: None,
        };
        self.reading = Some(
            client.submit_blocking(scope, spec, move |context| stored(&installation, &context))?,
        );
        self.stale = false;
        Ok(())
    }

    /// Whether the record should be read now: it may have changed and no read is outstanding.
    pub(crate) fn wants_read(&self) -> bool {
        self.stale && self.reading.is_none()
    }

    /// Adopt a finished storage read. A channel that could not be read keeps the last known one.
    fn adopt(&mut self, stored: StoredUpdates) {
        self.change(|status| {
            status.reconcile(stored.recorded);
            if let Some(channel) = stored.channel {
                status.observe_channel(channel);
            }
        });
    }

    fn change(&mut self, apply: impl FnOnce(&mut UpdateStatus)) {
        let before = self.status.clone();
        apply(&mut self.status);
        if self.status != before {
            self.revision = self.revision.saturating_add(1);
        }
    }
}

/// Read the record and the subscription of `installation`. The subscription read is advisory:
/// its failure is logged and leaves the channel unknown, never the record's state.
fn stored(
    installation: &std::path::Path,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> io::Result<StoredUpdates> {
    let recorded = recorded(read_update_selection(installation, context))?;
    let channel = match TrustStore::read_subscription(installation, context) {
        Ok(subscription) => subscription.map(|subscription| subscription.channel()),
        Err(cause) => {
            tracing::warn!(operation = "update_subscription_read_failed", cause = %cause,
                "The update channel could not be read; the last known channel stays shown");
            None
        }
    };
    Ok(StoredUpdates { recorded, channel })
}

/// What the record says about updates. An installation without an enabled selection records
/// nothing to show.
fn recorded(selection: Result<UpdateSelection, SelectionFailure>) -> io::Result<RecordedUpdates> {
    match selection {
        Ok(selection) => Ok(RecordedUpdates {
            prepared: selection.prepared.map(|prepared| prepared.version),
            previous_version_running: selection.rollback.is_some(),
            repairing: selection.repairing.is_some(),
        }),
        Err(SelectionFailure::Missing) => Ok(RecordedUpdates::default()),
        Err(cause) => Err(io::Error::other(cause)),
    }
}

/// Seconds since the Unix epoch for "last successful check"; a clock before 1970 reads as 0.
pub(crate) fn now_utc() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_and_rollback_come_only_from_the_record() -> io::Result<()> {
        assert_eq!(recorded(Err(SelectionFailure::Missing))?, RecordedUpdates::default());
        assert!(recorded(Err(SelectionFailure::LockTimeout)).is_err());
        Ok(())
    }

    #[test]
    fn every_visible_change_raises_the_revision_once_and_nothing_else_does() {
        let mut source = StatusSource::new();
        assert_eq!(source.snapshot().0, 0);
        source.begin();
        assert_eq!(source.snapshot().0, 1);
        let progress = AttemptProgress { phase: AttemptPhase::Checking, bytes: None };
        source.observe(progress);
        source.observe(progress);
        assert_eq!(source.snapshot().0, 2);
        source.finish(
            AttemptOutcome::Failed(UpdateReport {
                version: None,
                phase: AttemptPhase::Checking,
                causes: vec!["Update repository transport failed".into()],
            }),
            10,
        );
        assert_eq!(source.snapshot().0, 3);
        assert!(source.snapshot().1.failure().is_some());
        assert!(source.wants_read());
    }

    #[test]
    fn the_channel_follows_the_subscription_and_an_unread_one_keeps_the_last_known() {
        let mut source = StatusSource::new();
        assert_eq!(source.snapshot().1.channel(), Channel::Release);
        source.adopt(StoredUpdates { channel: Some(Channel::Canary), ..StoredUpdates::default() });
        assert_eq!((source.snapshot().0, source.snapshot().1.channel()), (1, Channel::Canary));
        source.adopt(StoredUpdates::default());
        assert_eq!((source.snapshot().0, source.snapshot().1.channel()), (1, Channel::Canary));
    }

    #[test]
    fn the_running_version_raises_the_revision_only_when_it_changes() {
        let mut source = StatusSource::new();
        source.observe_running(None);
        assert_eq!(source.snapshot().0, 0);
        source.observe_running(Some("0.2.0".into()));
        source.observe_running(Some("0.2.0".into()));
        assert_eq!(source.snapshot().0, 1);
        assert_eq!(source.snapshot().1.running(), Some("0.2.0"));
    }

    #[test]
    fn whether_checks_run_raises_the_revision_only_when_it_changes() {
        let mut source = StatusSource::new();
        source.observe_checks(false);
        assert_eq!(source.snapshot().0, 0);
        source.observe_checks(true);
        source.observe_checks(true);
        assert_eq!(source.snapshot().0, 1);
        assert!(source.snapshot().1.checks());
    }

    #[test]
    fn a_check_that_could_not_start_shows_that_it_could_not_check() {
        let mut source = StatusSource::new();
        source.not_started(10);
        let (revision, status) = source.snapshot();
        assert_eq!(revision, 1);
        assert_eq!(
            status.activity(),
            sugarsubstitute_contracts::update::status::UpdateActivity::Idle
        );
        assert_eq!(status.failure().map(|report| report.phase), Some(AttemptPhase::Opening));
        assert_eq!(status.last_successful_check_utc(), None);
    }

    #[test]
    fn the_sugarcubes_update_raises_the_revision_only_when_it_changes() {
        use sugarsubstitute_contracts::update::pack::PackUpdateStatus;
        let mut source = StatusSource::new();
        source.observe_sugarcubes(PackUpdateStatus::Idle);
        assert_eq!(source.snapshot().0, 0);
        source.observe_sugarcubes(PackUpdateStatus::Updating);
        source.observe_sugarcubes(PackUpdateStatus::Updating);
        assert_eq!(source.snapshot().0, 1);
        assert_eq!(source.snapshot().1.sugarcubes(), &PackUpdateStatus::Updating);
    }
}
