// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the desktop child the coordinator's update status, and keep its restart-to-apply intent.
//!
//! Only the newest snapshot matters, so one encoded snapshot waits for the shared write slot and
//! a newer one replaces it. Snapshots go out after replies and the UI watchdog's challenge; a
//! revoked write is retried unless a newer snapshot already replaced it. The child's
//! RestartToApply only records intent: the entry runner relaunches when the child then exits
//! normally, and never otherwise.
use super::{ParentLifecycle, ParentResources, Result, types::WritePurpose};
use std::{num::NonZeroU64, time::Duration};
use sugarsubstitute_contracts::update::status::{MAX_STATUS_BYTES, UpdateStatus};
use sugarsubstitute_supervision::lifecycle_protocol::{WireHeader, WirePayload};

/// How long one snapshot write may wait for the child to read.
const WRITE: Duration = Duration::from_secs(30);

/// One encoded snapshot.
struct Snapshot {
    revision: NonZeroU64,
    workflow_running: bool,
    status: Vec<u8>,
}

/// The snapshot waiting to be written, the one being written, and the child's intent.
#[derive(Default)]
pub(super) struct OutgoingUpdates {
    pending: Option<Snapshot>,
    writing: Option<Snapshot>,
    restart_to_apply: bool,
}
impl OutgoingUpdates {
    /// Heap the two retained encodings may occupy inside the parent partition.
    pub(super) const fn heap_bytes() -> u64 {
        2 * MAX_STATUS_BYTES as u64
    }

    /// Whether `revision` is newer than every snapshot waiting or being written.
    fn newer(&self, revision: NonZeroU64) -> bool {
        [&self.pending, &self.writing]
            .into_iter()
            .flatten()
            .all(|snapshot| snapshot.revision < revision)
    }

    /// The child's snapshot write finished.
    pub(super) fn written(&mut self) {
        self.writing = None;
    }

    /// The child's snapshot write was revoked before issue; retry it unless a newer one waits.
    pub(super) fn revoked(&mut self) {
        let revoked = self.writing.take();
        if self.pending.is_none() {
            self.pending = revoked;
        }
    }

    /// The authenticated child finished its close save and is exiting so an update applies.
    pub(super) fn restart_to_apply(&mut self) {
        self.restart_to_apply = true;
    }
}

impl ParentLifecycle {
    /// Queue `status` at `revision` for the child, replacing any snapshot not yet written.
    /// Revisions must increase; an older or repeated revision is ignored.
    /// # Errors
    /// Returns the status's encoding failure; nothing is queued then.
    pub(crate) fn publish_update_status(
        &mut self,
        revision: u64,
        status: &UpdateStatus,
        workflow_running: bool,
    ) -> Result<()> {
        let Some(revision) = NonZeroU64::new(revision) else { return Ok(()) };
        if self.updates.newer(revision) {
            self.updates.pending =
                Some(Snapshot { revision, workflow_running, status: status.encode()? });
        }
        Ok(())
    }

    /// Whether the authenticated child said it is exiting so a prepared update applies.
    /// This is intent only; the caller still requires the child's normal exit.
    pub(crate) fn restart_to_apply_requested(&self) -> bool {
        self.updates.restart_to_apply
    }

    /// Write the waiting snapshot when the shared write slot is free and the child is registered.
    pub(super) fn send_update_status(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if self.write.is_some() || !self.registered || self.cleanup.is_some() {
            return Ok(());
        }
        let Some(snapshot) = self.updates.pending.take() else { return Ok(()) };
        let until = resources.execution.client.now()?.checked_add(WRITE)?;
        let header = WireHeader {
            run: self.run,
            connection: Some(self.connection),
            request: None,
            deadline_qpc: resources.execution.client.encode_counter_deadline(until)?.ticks(),
        };
        let payload = WirePayload::UpdateStatus {
            revision: snapshot.revision,
            workflow_running: snapshot.workflow_running,
            status: &snapshot.status,
        };
        if self.write_message(resources, header, payload, until, WritePurpose::UpdateStatus)? {
            self.updates.writing = Some(snapshot);
        } else {
            self.updates.pending = Some(snapshot);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(revision: u64) -> Result<Snapshot> {
        Ok(Snapshot {
            revision: NonZeroU64::new(revision).ok_or("zero revision")?,
            workflow_running: false,
            status: UpdateStatus::default().encode()?,
        })
    }

    #[test]
    fn only_newer_snapshots_wait_and_a_revoked_write_retries_unless_replaced() -> Result<()> {
        let mut updates =
            OutgoingUpdates { pending: Some(snapshot(2)?), ..OutgoingUpdates::default() };
        assert!(!updates.newer(NonZeroU64::new(2).ok_or("zero")?));
        assert!(updates.newer(NonZeroU64::new(3).ok_or("zero")?));
        updates.writing = updates.pending.take();
        assert!(!updates.newer(NonZeroU64::new(1).ok_or("zero")?), "in flight still counts");
        updates.revoked();
        assert_eq!(updates.pending.as_ref().map(|s| s.revision.get()), Some(2), "retried");
        updates.writing = updates.pending.take();
        updates.pending = Some(snapshot(4)?);
        updates.revoked();
        assert_eq!(updates.pending.as_ref().map(|s| s.revision.get()), Some(4), "newer kept");
        updates.writing = updates.pending.take();
        updates.written();
        assert!(updates.writing.is_none() && updates.pending.is_none());
        assert!(!updates.restart_to_apply);
        updates.restart_to_apply();
        assert!(updates.restart_to_apply);
        Ok(())
    }
}
