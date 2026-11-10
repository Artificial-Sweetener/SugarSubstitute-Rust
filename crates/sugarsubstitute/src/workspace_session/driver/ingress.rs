// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate full ready observations and ordered transfers before requesting worker effects.
use super::*;
impl WorkspaceDriver {
    /// Consume a message already attributed to the current native document/allocation. Trust
    /// and complete scope are still compared here; transport correlation grants no effects.
    /// A matching captured receipt may follow publication without acknowledging durable delivery.
    /// # Errors
    /// Refuses foreign, stale, out-of-order, malformed and unrequested work.
    pub fn receive(
        &mut self,
        message: SessionMessage,
        now: Moment,
    ) -> Result<Option<DriverEvent>, StoreError> {
        validate_message(&message, &self.scope, &self.origin)?;
        if let SessionOperation::Ready { .. } = &message.operation {
            self.policy.observe(&message, now).map_err(application)?;
            self.update_restore_frontier()?;
            self.finish_restore()?;
            if (self.restored.is_none() || self.replay_retired)
                && !self.policy.live()
                && self.policy.committed()
            {
                self.policy.reconciled(&self.policy.fence().clone()).map_err(application)?;
            }
            if self.restore.is_none()
                && !self.replay_retired
                && !self.policy.live()
                && self.policy.committed()
                && let Some(restored) = self.restored.clone()
            {
                let checkpoint = restored
                    .checkpoint
                    .as_ref()
                    .map(|(_, value)| value)
                    .ok_or(SessionError::Invalid)?;
                let request = self.identities.next()?;
                let transfer = self.identities.next()?;
                self.restore = Some(Restore {
                    request: request.clone(),
                    transfer: transfer.clone(),
                    checkpoint: checkpoint.id.clone(),
                    revision: checkpoint.revision,
                    bindings: None,
                    fence: self.policy.fence().clone(),
                    sent: false,
                });
                self.encoding = Some((request.clone(), transfer.clone()));
                return Ok(Some(DriverEvent::EncodeRestore {
                    request_id: request,
                    transfer_id: transfer,
                    restored,
                }));
            }
            return Ok(None);
        }
        if let SessionOperation::Cancel { target_request_id } = &message.operation {
            if self.restore.as_ref().is_some_and(|restore| &restore.request == target_request_id) {
                self.cancel_restore()?;
                return Ok(None);
            }
            if self
                .capture
                .as_ref()
                .is_some_and(|capture| &capture.request == target_request_id && !capture.processing)
            {
                self.abandon_capture(now)?;
                return Ok(None);
            }
            return Err(SessionError::Stale.into());
        }
        if let SessionOperation::Error { failure } = &message.operation
            && self.owns_restore(&message.request_id)
        {
            if failure.code == "stale" || failure.code == "cancelled" {
                self.cancel_restore()?;
                return Ok(None);
            }
            // The page observed that it cannot materialize the saved workspace.
            self.abandon_restore(&RestoreFailure::Refused {
                code: failure.code.clone(),
                retryable: failure.retryable,
            })?;
            return Ok(None);
        }
        if let SessionOperation::Acknowledged { checkpoint_id, revision, phase, bindings } =
            &message.operation
        {
            if phase == "durable" {
                let ack = self.last_ack.as_mut().ok_or(SessionError::Stale)?;
                if message.request_id != ack.0
                    || *checkpoint_id != ack.1
                    || *revision != ack.2
                    || *bindings != ack.3
                {
                    return Err(SessionError::Stale.into());
                }
                ack.4 = true;
                return Ok(None);
            }
            if phase == "materialized" {
                let restore = self.restore.as_mut().ok_or(SessionError::Stale)?;
                let checkpoint = self
                    .restored
                    .as_ref()
                    .and_then(|restored| restored.checkpoint.as_ref())
                    .map(|(_, value)| value)
                    .ok_or(SessionError::Stale)?;
                if message.request_id != restore.request
                    || !restore.sent
                    || checkpoint_id != &restore.checkpoint
                    || *revision != restore.revision
                    || bindings.len() != checkpoint.entries.len()
                    || bindings.iter().any(|binding| {
                        !checkpoint.entries.iter().any(|entry| entry.id == binding.entry_id)
                            || restore
                                .fence
                                .bindings
                                .iter()
                                .any(|prior| same_runtime(prior, binding))
                    })
                {
                    return Err(SessionError::Stale.into());
                }
                restore.bindings = Some(bindings.clone());
                self.finish_restore()?;
                return Ok(None);
            }
            if phase == "captured"
                && (self.capture.as_ref().is_some_and(|capture| {
                    message.request_id == capture.request
                        && checkpoint_id == capture.ticket.checkpoint.as_str()
                        && *revision == capture.ticket.revision
                }) || self.last_ack.as_ref().is_some_and(|ack| {
                    message.request_id == ack.0
                        && *checkpoint_id == ack.1
                        && *revision == ack.2
                        && *bindings == ack.3
                }))
            {
                return Ok(None);
            }
            return Err(SessionError::Stale.into());
        }
        let capture = self.capture.as_mut().ok_or(SessionError::Stale)?;
        if message.request_id != capture.request || now >= capture.deadline {
            return Err(SessionError::Stale.into());
        }
        match message.operation {
            SessionOperation::Begin { transfer } => {
                if capture.processing || capture.transfer.is_some() {
                    return Err(SessionError::Invalid.into());
                }
                capture.transfer = Some(
                    CaptureTransfer::new(
                        transfer,
                        &capture.ticket,
                        self.scope.clone(),
                        capture.deadline,
                        now,
                        &self.memory,
                    )
                    .map_err(io::Error::other)?,
                );
                Ok(None)
            }
            SessionOperation::Chunk { transfer_id, index, text } => {
                capture
                    .transfer
                    .as_mut()
                    .ok_or(SessionError::Invalid)?
                    .chunk(&transfer_id, index, &text, now)
                    .map_err(io::Error::other)?;
                Ok(None)
            }
            SessionOperation::Commit { transfer_id } => {
                let transfer = capture.transfer.take().ok_or(SessionError::Invalid)?;
                capture.processing = true;
                Ok(Some(DriverEvent::DecodeCapture {
                    request_id: capture.request.clone(),
                    ticket: capture.ticket.clone(),
                    transfer_id,
                    transfer,
                }))
            }
            SessionOperation::Error { failure } if failure.retryable && !capture.processing => {
                let capture = self.capture.take().ok_or(SessionError::Stale)?;
                self.policy.superseded(&capture.ticket, now).map_err(application)?;
                Ok(None)
            }
            SessionOperation::Error { .. } if !capture.processing => {
                let capture = self.capture.take().ok_or(SessionError::Stale)?;
                self.policy.finished(&capture.ticket, false, now).map_err(application)?;
                Ok(None)
            }
            _ => Err(SessionError::Invalid.into()),
        }
    }
    fn finish_restore(&mut self) -> Result<(), StoreError> {
        let Some(restore) = &self.restore else {
            return Ok(());
        };
        let Some(bindings) = &restore.bindings else {
            return Ok(());
        };
        let current = self.policy.fence().clone();
        if !self.policy.committed()
            || bindings.len() != current.bindings.len()
            || !bindings.iter().all(|binding| {
                current.bindings.iter().any(|live| {
                    same_runtime(binding, live)
                        && live.graph_revision >= binding.graph_revision
                        && live.source_revision >= binding.source_revision
                        && live.navigation_revision >= binding.navigation_revision
                })
            })
        {
            return Ok(());
        }
        self.policy.reconciled(&current).map_err(application)?;
        self.media_bindings = bindings.clone();
        let restore = self.restore.take().ok_or(SessionError::Stale)?;
        self.queue(
            restore.request,
            SessionOperation::Acknowledged {
                checkpoint_id: restore.checkpoint,
                revision: restore.revision,
                phase: "restored".into(),
                bindings: current.bindings.clone(),
            },
        )
    }
    /// Start a due capture using host revisions, after materialization has acknowledged fresh
    /// bindings. Admission failures retain dirty state rather than publishing an empty workspace.
    /// # Errors
    /// Preserves explicit application refusal and original time arithmetic failures.
    pub fn tick(&mut self, now: Moment) -> Result<(), StoreError> {
        if self.close_cutoff.is_some_and(|cutoff| now >= cutoff) {
            return Ok(());
        }
        if let Some(capture) = &self.capture
            && !capture.processing
            && now >= capture.deadline
        {
            let capture = self.capture.take().ok_or(SessionError::Stale)?;
            self.policy.finished(&capture.ticket, false, now).map_err(application)?;
            let request = self.identities.next()?;
            self.queue(request, SessionOperation::Cancel { target_request_id: capture.request })?;
        }
        if self.capture.is_some()
            || self.encoding.is_some()
            || !self.policy.live()
            || !self.policy.committed()
            || self.policy.deadline().is_none_or(|due| due > now)
        {
            return Ok(());
        }
        let checkpoint = CheckpointId::new(self.identities.next()?)?;
        let Some(ticket) = self.policy.capture(now, checkpoint).map_err(application)? else {
            return Ok(());
        };
        let request = self.identities.next()?;
        let deadline = now.checked_add(Duration::from_secs(30)).map_err(io::Error::other)?;
        let deadline = self.close_cutoff.map_or(deadline, |cutoff| cutoff.min(deadline));
        self.queue(
            request.clone(),
            SessionOperation::Capture {
                fence: ticket.fence.clone(),
                checkpoint_id: ticket.checkpoint.as_str().into(),
                revision: ticket.revision,
            },
        )?;
        self.capture =
            Some(Capture { request, ticket, deadline, transfer: None, processing: false });
        Ok(())
    }
}
