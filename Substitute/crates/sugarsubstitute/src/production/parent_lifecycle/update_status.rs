// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the desktop child the coordinator's update status and the backend's workflow queue, and
//! keep its restart-to-apply intent and its requests to check now.
//!
//! Only the newest state matters. A status change or a queue change marks the snapshot due; the
//! next free write slot carries the newest status and queue under a fresh revision, so a revoked
//! write is simply sent again with whatever is newest by then. Snapshots go out after replies
//! and the UI watchdog's challenge. The child's RestartToApply only records intent: the entry
//! runner relaunches when the child then exits normally, and never otherwise. A CheckForUpdates
//! request is kept until the entry runner takes it; repeated requests before then are one.
use super::{ParentLifecycle, ParentResources, Result, types::WritePurpose, workflow_watch};
use std::{num::NonZeroU64, time::Duration};
use sugarsubstitute_contracts::{
    execution::Moment,
    update::status::{MAX_STATUS_BYTES, UpdateStatus},
};
use sugarsubstitute_supervision::lifecycle_protocol::{WireHeader, WirePayload, WorkflowQueue};

/// How long one snapshot write may wait for the child to read.
const WRITE: Duration = Duration::from_secs(30);

/// The newest status and queue, whether the child still needs them, and the child's intent.
#[derive(Default)]
pub(super) struct OutgoingUpdates {
    status_revision: Option<u64>,
    status: Option<Vec<u8>>,
    sent_queue: Option<WorkflowQueue>,
    due: bool,
    revision: u64,
    writing: bool,
    restart_to_apply: bool,
    check_requested: bool,
    /// Coordinator revision of the status being written, and of the newest the child received.
    writing_status: Option<u64>,
    delivered_status: Option<u64>,
    watch: workflow_watch::WorkflowWatch,
}
impl OutgoingUpdates {
    /// Heap the retained status encoding and the copy being written may occupy inside the
    /// parent partition.
    pub(super) const fn heap_bytes() -> u64 {
        2 * MAX_STATUS_BYTES as u64
    }

    /// The child's snapshot write finished.
    pub(super) fn written(&mut self) {
        self.writing = false;
        if let Some(written) = self.writing_status.take() {
            self.delivered_status = self.delivered_status.max(Some(written));
        }
    }

    /// The child's snapshot write was revoked before issue; the newest state goes out again.
    pub(super) fn revoked(&mut self) {
        self.writing = false;
        self.writing_status = None;
        self.due = true;
    }

    /// The authenticated child finished its close save and is exiting so an update applies.
    pub(super) fn restart_to_apply(&mut self) {
        self.restart_to_apply = true;
    }

    /// The authenticated child asked for an update check now.
    pub(super) fn check_requested(&mut self) {
        self.check_requested = true;
    }

    /// When the workflow watch's reconnect is due.
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.watch.deadline()
    }

    /// Stop following the backend's queue; cleanup has begun.
    pub(super) fn stop_watch(&mut self) {
        self.watch.stop();
    }

    /// Adopt the watch's queue, marking the snapshot due when it changed.
    fn observe_queue(&mut self) {
        let queue = self.watch.queue();
        if self.sent_queue != Some(queue) {
            self.sent_queue = Some(queue);
            self.due = true;
        }
    }

    /// The next snapshot to write, when one is due and a status exists.
    fn take_due(&mut self) -> Option<(NonZeroU64, WorkflowQueue, Vec<u8>)> {
        if !self.due || self.writing {
            return None;
        }
        let status = self.status.clone()?;
        let revision = NonZeroU64::new(self.revision.checked_add(1)?)?;
        self.revision = revision.get();
        self.due = false;
        self.writing_status = self.status_revision;
        Some((revision, self.sent_queue.unwrap_or(WorkflowQueue::Unobserved), status))
    }
}

impl ParentLifecycle {
    /// Keep `status`, the coordinator's status at `revision`, for the child when it is newer
    /// than the one kept. The caller may offer the same revision every turn.
    /// # Errors
    /// Returns the status's encoding failure; the kept status is unchanged.
    pub(crate) fn publish_update_status(
        &mut self,
        revision: u64,
        status: &UpdateStatus,
    ) -> Result<()> {
        if self.updates.status_revision.is_none_or(|kept| revision > kept) {
            self.updates.status = Some(status.encode()?);
            self.updates.status_revision = Some(revision);
            self.updates.due = true;
        }
        Ok(())
    }

    /// Follow the granted backend's queue; nothing here fails the parent.
    pub(super) fn advance_workflow_watch(&mut self, resources: &ParentResources<'_, '_>) {
        let origin = self
            .lease
            .and_then(|lease| resources.backend.ready_origin(lease.resource).ok().flatten());
        self.updates.watch.advance(origin, resources.execution);
        self.updates.observe_queue();
    }

    /// Whether the authenticated child said it is exiting so a prepared update applies.
    /// This is intent only; the caller still requires the child's normal exit.
    pub(crate) fn restart_to_apply_requested(&self) -> bool {
        self.updates.restart_to_apply
    }

    /// Whether the child received the coordinator's status at `revision` or a newer one.
    pub(crate) fn update_status_delivered(&self, revision: u64) -> bool {
        self.updates.delivered_status.is_some_and(|delivered| delivered >= revision)
    }

    /// Take the child's request to check for updates now, once.
    pub(crate) fn take_check_request(&mut self) -> bool {
        std::mem::take(&mut self.updates.check_requested)
    }

    /// Write the waiting snapshot when the shared write slot is free and the child is registered.
    pub(super) fn send_update_status(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if self.write.is_some() || !self.registered || self.cleanup.is_some() {
            return Ok(());
        }
        let until = resources.execution.client.now()?.checked_add(WRITE)?;
        let header = WireHeader {
            run: self.run,
            connection: Some(self.connection),
            request: None,
            deadline_qpc: resources.execution.client.encode_counter_deadline(until)?.ticks(),
        };
        let Some((revision, workflow, status)) = self.updates.take_due() else { return Ok(()) };
        let payload = WirePayload::UpdateStatus { revision, workflow, status: &status };
        if self.write_message(resources, header, payload, until, WritePurpose::UpdateStatus)? {
            self.updates.writing = true;
        } else {
            self.updates.due = true;
            self.updates.writing_status = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_status_and_queue_go_out_once_under_a_fresh_revision() -> Result<()> {
        let mut updates = OutgoingUpdates::default();
        updates.observe_queue();
        assert!(updates.take_due().is_none(), "nothing goes out before a status exists");
        updates.status = Some(UpdateStatus::default().encode()?);
        let first = updates.take_due().map(|(revision, queue, _)| (revision.get(), queue));
        assert_eq!(first, Some((1, WorkflowQueue::Unobserved)));
        assert!(updates.take_due().is_none(), "an unchanged state is not resent");
        updates.due = true;
        updates.writing = true;
        assert!(updates.take_due().is_none(), "one write at a time");
        updates.revoked();
        assert_eq!(updates.take_due().map(|(revision, ..)| revision.get()), Some(2));
        updates.observe_queue();
        assert!(updates.take_due().is_none(), "the same queue is not a change");
        assert!(!updates.restart_to_apply);
        updates.restart_to_apply();
        assert!(updates.restart_to_apply);
        Ok(())
    }

    fn parent() -> std::result::Result<ParentLifecycle, Box<dyn std::error::Error + Send + Sync>> {
        let identity = super::super::tests::identity().map_err(|cause| cause.to_string())?;
        let mut storage = Some(super::super::causes::BackendCauses::new(
            crate::production::coordinator_backend::tests::storage_tests::claim(
                ParentLifecycle::storage_bytes(),
            )?,
        )?);
        Ok(ParentLifecycle::new(
            identity.run,
            identity.launch,
            sugarsubstitute_contracts::configuration::ProfileId::new("Updates_1".into())?,
            sugarsubstitute_contracts::lifecycle::ConnectionId::new(1)?,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
                100,
                1_000_000_000,
            )?,
            &mut storage,
        )?)
    }

    /// The child's check requests reach the runner through the parent's own take, once, however
    /// many arrived before it was taken.
    #[test]
    fn repeated_check_requests_before_the_runner_takes_them_are_one()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut parent = parent()?;
        assert!(!parent.take_check_request());
        parent.updates.check_requested();
        parent.updates.check_requested();
        assert!(parent.take_check_request());
        assert!(!parent.take_check_request(), "taken once");
        Ok(())
    }

    /// A status counts as delivered only once its own write finished: a revoked or still
    /// running write delivers nothing, and a newer status is not delivered by an older write.
    #[test]
    fn a_status_is_delivered_only_when_its_write_finishes()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut parent = parent()?;
        let status = UpdateStatus::default();
        parent.publish_update_status(4, &status)?;
        assert!(!parent.update_status_delivered(4), "nothing written yet");
        parent.updates.take_due().ok_or("status not due")?;
        parent.updates.writing = true;
        parent.publish_update_status(5, &status)?;
        assert!(!parent.update_status_delivered(4), "a write in flight delivers nothing");
        parent.updates.revoked();
        assert!(!parent.update_status_delivered(4));
        parent.updates.take_due().ok_or("status not due again")?;
        parent.updates.written();
        assert!(parent.update_status_delivered(5));
        assert!(!parent.update_status_delivered(6));
        Ok(())
    }
}
