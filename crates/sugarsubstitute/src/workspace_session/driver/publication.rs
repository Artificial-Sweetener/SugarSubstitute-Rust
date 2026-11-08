// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance saved revisions only after matching actual durable store receipts.
use super::*;
impl WorkspaceDriver {
    /// Recheck a worker-decoded capture before requesting native input/asset retention. The
    /// composing owner augments native input receipts and placement, then the store validates
    /// full checkpoint semantics again on its worker before any publication.
    /// # Errors
    /// Refuses foreign, superseded or duplicate capture completion.
    pub fn decoded_capture(
        &mut self,
        request: &str,
        ticket: &CaptureTicket,
        captured: &mut transfer::CapturedWorkspace,
        now: Moment,
    ) -> Result<(), StoreError> {
        let capture = self.capture.as_ref().ok_or(SessionError::Stale)?;
        if capture.request != request || capture.ticket != *ticket {
            return Err(SessionError::Stale.into());
        }
        if self.policy.admit_capture_identity(ticket, &captured.checkpoint).is_err() {
            self.capture = None;
            self.policy.superseded(ticket, now).map_err(application)?;
            return Err(SessionError::Stale.into());
        }
        captured.checkpoint.placement = self.placement.clone();
        captured.checkpoint.previous_id =
            self.policy.durable().map(|durable| durable.id.as_str().to_owned());
        Ok(())
    }
    /// Admit the sole publisher after its matching native input and immutable asset work was
    /// submitted under the original request. This never acknowledges a saved revision.
    /// # Errors
    /// Refuses stale runtime/generation/entry facts without dispatching storage.
    pub fn publishing(
        &mut self,
        ticket: &CaptureTicket,
        captured: &transfer::CapturedWorkspace,
    ) -> Result<(), StoreError> {
        self.policy.publish(ticket, &captured.checkpoint).map_err(application)
    }
    /// Observe checked atomic publication. An unrelated receipt cannot change durable state.
    /// # Errors
    /// Refuses stale/duplicate results or a receipt not naming this exact captured checkpoint.
    pub fn published(
        &mut self,
        ticket: &CaptureTicket,
        captured: transfer::CapturedWorkspace,
        receipt: &SaveReceipt,
        now: Moment,
    ) -> Result<(), StoreError> {
        let capture = self.capture.as_ref().ok_or(SessionError::Stale)?;
        if capture.ticket != *ticket
            || receipt.checkpoint_id != ticket.checkpoint.as_str()
            || receipt.revision != ticket.revision
        {
            return Err(SessionError::Stale.into());
        }
        self.originals.remember(&captured.checkpoint)?;
        self.policy.finished(ticket, true, now).map_err(application)?;
        let capture = self.capture.take().ok_or(SessionError::Stale)?;
        self.published = Some(Arc::new(captured));
        self.restored = None;
        self.last_ack = Some((
            capture.request.clone(),
            receipt.checkpoint_id.clone(),
            receipt.revision,
            ticket.fence.bindings.clone(),
            false,
        ));
        self.queue(
            capture.request,
            SessionOperation::Acknowledged {
                checkpoint_id: receipt.checkpoint_id.clone(),
                revision: receipt.revision,
                phase: "durable".into(),
                bindings: ticket.fence.bindings.clone(),
            },
        )
    }
    /// Observe a failed/cancelled publisher while preserving the last durable checkpoint.
    /// # Errors
    /// Refuses foreign/duplicate terminal observations; original adapter causes stay with caller.
    pub fn publication_failed(
        &mut self,
        ticket: &CaptureTicket,
        now: Moment,
    ) -> Result<(), StoreError> {
        let capture = self.capture.as_ref().ok_or(SessionError::Stale)?;
        if capture.ticket != *ticket {
            return Err(SessionError::Stale.into());
        }
        self.policy.finished(ticket, false, now).map_err(application)?;
        let capture = self.capture.take().ok_or(SessionError::Stale)?;
        self.queue(
            capture.request,
            SessionOperation::Error {
                failure: SessionFailure {
                    code: "storage".into(),
                    retryable: true,
                    entry_id: None,
                    output_id: None,
                },
            },
        )
    }
    /// Resolve a retained asset only for a fresh acknowledged binding and exact published or
    /// owned materializing checkpoint. A caller may defer `None` during materialization until
    /// the fresh binding acknowledgement arrives; a page UUID alone grants no file access.
    pub fn asset(&self, output_id: &str, binding: &SessionBinding) -> Option<SessionAsset> {
        if self.replay_retired && self.published.is_none() {
            return None;
        }
        let current = self.policy.fence().bindings.iter().any(|live| same_runtime(live, binding));
        let admitted = current
            && (self.policy.live()
                && (self.policy.fence().bindings.contains(binding)
                    || self.media_bindings.contains(binding))
                || self
                    .restore
                    .as_ref()
                    .and_then(|restore| restore.bindings.as_ref())
                    .is_some_and(|bindings| bindings.contains(binding)));
        if !admitted {
            return None;
        }
        self.asset_in_checkpoint(output_id, binding)
    }
    /// Read-only browser metadata/bytes admission for an owned pending restore after its exact
    /// fresh Ready proposal is observed. Native Select still requires acknowledged bindings.
    pub fn pending_asset(&self, output_id: &str, binding: &SessionBinding) -> Option<SessionAsset> {
        if self.restore.as_ref().is_none_or(|restore| !restore.sent)
            || self.policy.live()
            || !self.policy.fence().bindings.contains(binding)
        {
            return None;
        }
        let restore = self.restore.as_ref()?;
        let checkpoint =
            self.restored.as_ref()?.checkpoint.as_ref().map(|(_, checkpoint)| checkpoint)?;
        let fence = self.policy.fence();
        if fence.revision <= restore.fence.revision
            || fence.bindings.len() != checkpoint.entries.len()
            || fence.bindings.iter().any(|fresh| {
                !checkpoint.entries.iter().any(|entry| entry.id == fresh.entry_id)
                    || restore.fence.bindings.iter().any(|prior| same_runtime(prior, fresh))
            })
        {
            return None;
        }
        self.asset_in_checkpoint(output_id, binding)
    }
    /// Recheck exact immutable media authority after the worker terminates and before native
    /// delivery. Generation removal, output retirement or restore cancellation revokes a loan.
    pub fn asset_valid_for_delivery(
        &self,
        webview: &str,
        output_id: &str,
        binding: &SessionBinding,
        expected: &SessionAsset,
    ) -> bool {
        self.webview() == webview
            && self
                .asset(output_id, binding)
                .or_else(|| self.pending_asset(output_id, binding))
                .as_ref()
                == Some(expected)
    }
    fn asset_in_checkpoint(
        &self,
        output_id: &str,
        binding: &SessionBinding,
    ) -> Option<SessionAsset> {
        let checkpoint = self.published.as_ref().map(|value| &value.checkpoint).or_else(|| {
            self.restored
                .as_ref()
                .and_then(|value| value.checkpoint.as_ref().map(|(_, value)| value))
        })?;
        let output = checkpoint
            .entries
            .iter()
            .find(|entry| entry.id == binding.entry_id)?
            .outputs
            .iter()
            .find(|output| output.id == output_id)?;
        let digest = output.asset_digest.as_ref()?;
        checkpoint.assets.iter().find(|asset| &asset.digest == digest).cloned()
    }
}
