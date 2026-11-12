// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the coordinator's newest update status for the desktop's update surfaces.
//!
//! The coordinator sends a snapshot after every change. Pipe order already delivers them in
//! sequence; the revision still decides, so a replayed or reordered older snapshot never
//! replaces a newer one. The status is presentation only and grants nothing.
use sugarsubstitute_contracts::update::status::UpdateStatus;
use sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload;

/// What the coordinator last said about updates and the backend's workflow queue.
#[derive(Debug, Default)]
pub(in crate::production) struct CoordinatorUpdates {
    revision: u64,
    status: Option<UpdateStatus>,
    workflow_running: bool,
}
impl CoordinatorUpdates {
    /// The newest status, once the coordinator has sent one.
    pub(in crate::production) fn status(&self) -> Option<&UpdateStatus> {
        self.status.as_ref()
    }

    /// Whether the backend's queue held a running or queued workflow at the newest snapshot.
    pub(in crate::production) fn workflow_running(&self) -> bool {
        self.workflow_running
    }

    /// Adopt one decoded Kind22 payload when its revision is newer than the one kept.
    /// # Errors
    /// Rejects other payloads and statuses that fail their contract owner's bounds; the kept
    /// snapshot is unchanged.
    pub(super) fn receive(
        &mut self,
        payload: &DecodedPayload<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let DecodedPayload::UpdateStatus { revision, workflow_running, status } = payload else {
            return Err("Lifecycle frame is not an update status".into());
        };
        let status = UpdateStatus::decode(status)?;
        if revision.get() > self.revision {
            self.revision = revision.get();
            self.status = Some(status);
            self.workflow_running = *workflow_running;
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
        running: bool,
        bytes: &[u8],
    ) -> Result<DecodedPayload<'_>, &'static str> {
        Ok(DecodedPayload::UpdateStatus {
            revision: NonZeroU64::new(revision).ok_or("zero revision")?,
            workflow_running: running,
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
        updates.receive(&snapshot(2, true, &checking_bytes)?)?;
        updates.receive(&snapshot(1, false, &idle_bytes)?)?;
        assert_eq!(updates.status(), Some(&checking));
        assert!(updates.workflow_running());
        assert!(updates.receive(&snapshot(3, false, b"{\"forged\":true}")?).is_err());
        assert_eq!(updates.status(), Some(&checking));
        updates.receive(&snapshot(3, false, &idle_bytes)?)?;
        assert_eq!(updates.status(), Some(&UpdateStatus::default()));
        assert!(!updates.workflow_running());
        Ok(())
    }
}
