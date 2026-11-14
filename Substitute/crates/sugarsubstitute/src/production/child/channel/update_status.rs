// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the coordinator's newest update status for the desktop's update surfaces.
//!
//! The coordinator sends a snapshot after every change. Pipe order already delivers them in
//! sequence; the revision still decides, so a replayed or reordered older snapshot never
//! replaces a newer one. The status is presentation only and grants nothing.
//!
//! This is the child's one update-status source. The window reads it through `StartupHost`; the
//! host side of SugarCubes' `sugarcubes.updates` protocol is meant to read the same snapshot, so
//! both surfaces always agree.
use sugarsubstitute_contracts::update::status::{StartupNotice, UpdateStatus};
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, WorkflowQueue};

/// What the coordinator last said about updates and the backend's workflow queue.
#[derive(Debug)]
pub(in crate::production) struct CoordinatorUpdates {
    revision: u64,
    status: Option<UpdateStatus>,
    workflow: WorkflowQueue,
    /// Start-up messages the window already took during this launch.
    taken: Vec<StartupNotice>,
}
impl Default for CoordinatorUpdates {
    fn default() -> Self {
        Self { revision: 0, status: None, workflow: WorkflowQueue::Unobserved, taken: Vec::new() }
    }
}
impl CoordinatorUpdates {
    /// The next start-up message the newest status states that the window has not taken during
    /// this launch. Each is taken at most once, however often the coordinator resends it.
    pub(in crate::production) fn take_notice(&mut self) -> Option<StartupNotice> {
        let next = self
            .status
            .as_ref()?
            .startup_notices()
            .into_iter()
            .find(|notice| !self.taken.contains(notice))?;
        self.taken.push(next.clone());
        Some(next)
    }

    /// The newest status, once the coordinator has sent one.
    pub(in crate::production) fn status(&self) -> Option<&UpdateStatus> {
        self.status.as_ref()
    }

    /// The backend's workflow queue at the newest snapshot; unobserved before the first.
    pub(in crate::production) fn workflow(&self) -> WorkflowQueue {
        self.workflow
    }

    /// Adopt one decoded Kind22 payload when its revision is newer than the one kept.
    /// # Errors
    /// Rejects other payloads and statuses that fail their contract owner's bounds; the kept
    /// snapshot is unchanged.
    pub(super) fn receive(
        &mut self,
        payload: &DecodedPayload<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let DecodedPayload::UpdateStatus { revision, workflow, status } = payload else {
            return Err("Lifecycle frame is not an update status".into());
        };
        let status = UpdateStatus::decode(status)?;
        if revision.get() > self.revision {
            self.revision = revision.get();
            self.status = Some(status);
            self.workflow = *workflow;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    fn snapshot(
        revision: u64,
        workflow: WorkflowQueue,
        bytes: &[u8],
    ) -> Result<DecodedPayload<'_>, &'static str> {
        Ok(DecodedPayload::UpdateStatus {
            revision: NonZeroU64::new(revision).ok_or("zero revision")?,
            workflow,
            status: bytes,
        })
    }

    #[test]
    fn the_newest_revision_wins_and_invalid_statuses_change_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut checking = UpdateStatus::default();
        checking.begin();
        let checking_bytes = checking.encode()?;
        let idle_bytes = UpdateStatus::default().encode()?;
        let mut updates = CoordinatorUpdates::default();
        assert!(updates.status().is_none());
        assert_eq!(updates.workflow(), WorkflowQueue::Unobserved);
        updates.receive(&snapshot(2, WorkflowQueue::Busy, &checking_bytes)?)?;
        updates.receive(&snapshot(1, WorkflowQueue::Idle, &idle_bytes)?)?;
        assert_eq!(updates.status(), Some(&checking));
        assert_eq!(updates.workflow(), WorkflowQueue::Busy);
        assert!(updates.receive(&snapshot(3, WorkflowQueue::Idle, b"{\"forged\":true}")?).is_err());
        assert_eq!(updates.status(), Some(&checking));
        updates.receive(&snapshot(3, WorkflowQueue::Idle, &idle_bytes)?)?;
        assert_eq!(updates.status(), Some(&UpdateStatus::default()));
        assert_eq!(updates.workflow(), WorkflowQueue::Idle);
        Ok(())
    }

    #[test]
    fn each_start_up_message_is_taken_once_in_order_however_often_it_is_resent()
    -> Result<(), Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::update::status::{LaunchVersion, RecordedUpdates};
        let mut updates = CoordinatorUpdates::default();
        assert_eq!(updates.take_notice(), None, "nothing before the first snapshot");
        let mut status = UpdateStatus::default();
        status.observe_launch(LaunchVersion::Applying("2.0.0".into()));
        updates.receive(&snapshot(1, WorkflowQueue::Idle, &status.encode()?)?)?;
        assert_eq!(updates.take_notice(), None, "an unconfirmed version is not announced");
        status.observe_launch(LaunchVersion::Updated("2.0.0".into()));
        status.reconcile(RecordedUpdates { repairing: true, ..RecordedUpdates::default() });
        for revision in [2, 3] {
            updates.receive(&snapshot(revision, WorkflowQueue::Idle, &status.encode()?)?)?;
        }
        assert_eq!(updates.take_notice(), Some(StartupNotice::Updated("2.0.0".into())));
        updates.receive(&snapshot(4, WorkflowQueue::Idle, &status.encode()?)?)?;
        assert_eq!(updates.take_notice(), Some(StartupNotice::Repairing));
        assert_eq!(updates.take_notice(), None);
        Ok(())
    }
}
