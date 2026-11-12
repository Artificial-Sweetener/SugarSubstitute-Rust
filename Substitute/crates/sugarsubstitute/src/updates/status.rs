// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the update control shows. Activity and the last result come from the attempt channel;
//! whether an update is ready, or the previous version is running, comes only from the selection
//! record, read through the storage owner at start and after every check. Each change raises the
//! revision, so a consumer that receives snapshots out of order keeps the newest.
use std::{io, path::PathBuf};
use sugarsubstitute_contracts::{
    execution::{
        ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Moment, Operation, OwnedJob, Scope,
    },
    update::{
        attempt::{AttemptOutcome, AttemptProgress},
        status::{RecordedUpdates, UpdateStatus},
    },
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::setup_storage::{
    self, SelectionFailure, UpdateSelection, read_update_selection,
};

/// Bound of one selection read; the read takes only the shared record lock.
const READ: std::time::Duration = std::time::Duration::from_secs(30);

/// The coordinator's one update status, with the revision of its last change.
pub(crate) struct StatusSource {
    status: UpdateStatus,
    revision: u64,
    stale: bool,
    reading: Option<OwnedJob<RecordedUpdates, io::Error>>,
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
            JobOutcome::Success(recorded) => self.change(|status| status.reconcile(recorded)),
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
        self.reading = Some(client.submit_blocking(scope, spec, move |context| {
            recorded(read_update_selection(&installation, &context))
        })?);
        self.stale = false;
        Ok(())
    }

    /// Whether the record should be read now: it may have changed and no read is outstanding.
    pub(crate) fn wants_read(&self) -> bool {
        self.stale && self.reading.is_none()
    }

    fn change(&mut self, apply: impl FnOnce(&mut UpdateStatus)) {
        let before = self.status.clone();
        apply(&mut self.status);
        if self.status != before {
            self.revision = self.revision.saturating_add(1);
        }
    }
}

/// What the record says about updates. An installation without an enabled selection records
/// nothing to show.
fn recorded(selection: Result<UpdateSelection, SelectionFailure>) -> io::Result<RecordedUpdates> {
    match selection {
        Ok(selection) => Ok(RecordedUpdates {
            prepared: selection.prepared.map(|prepared| prepared.version),
            previous_version_running: selection.rollback.is_some(),
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
    use sugarsubstitute_contracts::update::attempt::{AttemptPhase, UpdateReport};

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
}
