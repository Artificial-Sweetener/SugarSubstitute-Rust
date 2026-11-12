// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit immutable captures and observe exactly their durable publication outcomes.
use super::*;
use sugarsubstitute_contracts::desktop_session::{validate_checkpoint, wire::SessionCheckpoint};

pub(super) const MAX_REVISION: u64 = 9_007_199_254_740_991;

/// Host-assigned capture identity and frozen complete page-generation set.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureTicket {
    /// Host-allocated immutable checkpoint identity.
    pub checkpoint: CheckpointId,
    /// Independent increasing durable checkpoint revision.
    pub revision: u64,
    /// Complete observation against which capture and publication are admitted.
    pub fence: SessionFence,
    /// Whether this is the frozen final close capture.
    pub closing: bool,
}
/// A verified store's durable publication acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCheckpoint {
    /// Published checkpoint identity.
    pub id: CheckpointId,
    /// Published durable revision.
    pub revision: u64,
}
impl WorkspaceSession {
    /// Retire a superseded capture without reporting an I/O failure or closing prematurely.
    /// The final committed observation is recaptured immediately inside the original cutoff.
    /// # Errors
    /// Refuses foreign or already-publishing captures; their actual terminal outcome is required.
    pub fn superseded(
        &mut self,
        ticket: &CaptureTicket,
        now: Moment,
    ) -> Result<(), WorkspaceError> {
        if self.active.as_ref() != Some(ticket) || self.publishing {
            return Err(SessionError::Stale.into());
        }
        self.active = None;
        self.schedule.changed(now)?;
        if self.closing.is_some() {
            self.schedule.force(now);
        }
        Ok(())
    }
    /// Request the next due immutable capture. One capture/publisher plus one coalesced dirty
    /// observation is admitted. The caller supplies an independently allocated fresh UUID.
    /// # Errors
    /// Refuses protected originals, absent restore/commit/capability, opt-out or exhaustion.
    pub fn capture(
        &mut self,
        now: Moment,
        checkpoint: CheckpointId,
    ) -> Result<Option<CaptureTicket>, WorkspaceError> {
        if self.protected {
            return Err(WorkspaceError::Protected);
        }
        if !self.loaded || !self.reconciled || self.active.is_some() {
            return Ok(None);
        }
        if !self.frontend.persistence {
            return Err(SessionError::OptedOut.into());
        }
        if !self.frontend.committed {
            return Err(SessionError::NotCommitted.into());
        }
        if !self.frontend.capabilities.capture || !self.frontend.capabilities.chunk_transfer {
            return Err(SessionError::Unsupported.into());
        }
        if self.schedule.deadline().is_none_or(|due| due > now) {
            return Ok(None);
        }
        if self.closing.is_some_and(|cutoff| now >= cutoff) {
            return Ok(None);
        }
        let revision = self
            .next_revision
            .checked_add(1)
            .filter(|value| *value <= MAX_REVISION)
            .ok_or(WorkspaceError::Exhausted)?;
        let ticket = CaptureTicket {
            checkpoint,
            revision,
            fence: self.frontend.fence.clone(),
            closing: self.closing.is_some(),
        };
        self.next_revision = revision;
        self.unsaved_since = self.unsaved_since.or(self.schedule.since());
        self.schedule.take();
        self.active = Some(ticket.clone());
        Ok(Some(ticket))
    }

    /// Check a decoded capture before asset/input work and again immediately before publication.
    /// Graph meaning remains with Comfy; exact entry/runtime revisions remain fenced here.
    /// # Errors
    /// Refuses stale/superseded capture, wrong scope/revision/identity or inconsistent entries.
    pub fn admit_capture(
        &self,
        ticket: &CaptureTicket,
        checkpoint: &SessionCheckpoint,
    ) -> Result<(), WorkspaceError> {
        validate_checkpoint(checkpoint)?;
        self.admit_capture_identity(ticket, checkpoint)
    }

    /// Recheck bounded identity/revision facts after a worker validated complete checkpoint
    /// semantics. This avoids serializing graph JSON on the native event-loop thread.
    /// # Errors
    /// Refuses any superseded capture, scope, checkpoint identity or entry-generation facts.
    pub fn admit_capture_identity(
        &self,
        ticket: &CaptureTicket,
        checkpoint: &SessionCheckpoint,
    ) -> Result<(), WorkspaceError> {
        if self.active.as_ref() != Some(ticket)
            || self.publishing
            || !same_fence(&ticket.fence, &self.frontend.fence)
            || checkpoint.scope != self.scope
            || checkpoint.id != ticket.checkpoint.as_str()
            || checkpoint.revision != ticket.revision
            || checkpoint.entries.len() != ticket.fence.bindings.len()
            || checkpoint.entries.iter().any(|entry| {
                !ticket.fence.bindings.iter().any(|binding| {
                    binding.entry_id == entry.id
                        && binding.graph_revision == entry.document.graph_revision
                        && binding.source_revision == entry.source_revision
                        && binding.navigation_revision == entry.navigation_revision
                })
            })
        {
            return Err(SessionError::Stale.into());
        }
        Ok(())
    }

    /// Mark the validated capture as the sole publisher, after its asset/input receipts exist.
    /// # Errors
    /// Refuses the same stale or invalid values as [`Self::admit_capture`].
    pub fn publish(
        &mut self,
        ticket: &CaptureTicket,
        checkpoint: &SessionCheckpoint,
    ) -> Result<(), WorkspaceError> {
        self.admit_capture_identity(ticket, checkpoint)?;
        self.publishing = true;
        Ok(())
    }

    /// Observe the actual store terminal result. `saved` may only describe a validated atomic
    /// publication, after its referenced authoritative assets and input archive were durable.
    /// # Errors
    /// Refuses foreign/duplicate terminal observations without changing acknowledged state.
    pub fn finished(
        &mut self,
        ticket: &CaptureTicket,
        saved: bool,
        now: Moment,
    ) -> Result<Option<DurableCheckpoint>, WorkspaceError> {
        if self.active.as_ref() != Some(ticket) || (saved && !self.publishing) {
            return Err(SessionError::Stale.into());
        }
        self.active = None;
        self.publishing = false;
        if saved {
            let receipt =
                DurableCheckpoint { id: ticket.checkpoint.clone(), revision: ticket.revision };
            self.durable = Some(receipt.clone());
            self.unsaved_since = None;
            if self.closing.is_some() && self.schedule.dirty() {
                self.schedule.force(now);
            }
            Ok(Some(receipt))
        } else {
            self.schedule.changed(now)?;
            if self.closing.is_some() {
                self.close_failed = true;
            }
            Ok(None)
        }
    }
}
