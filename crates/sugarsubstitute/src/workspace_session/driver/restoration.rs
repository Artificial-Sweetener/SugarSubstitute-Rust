// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Freeze startup replay against authored intent while observing every encoding terminal.
use super::*;
impl WorkspaceDriver {
    /// Whether the exact host-owned restore still permits delivery. A worker whose request
    /// was superseded must be cancelled and its terminal observed before another capture.
    pub fn owns_restore(&self, request: &str) -> bool {
        self.restore.as_ref().is_some_and(|restore| restore.request == request)
    }
    /// Observe cancellation/failure of the exact encoding worker without adopting its bytes.
    /// # Errors
    /// Refuses an unrelated or duplicate terminal observation.
    pub fn encoding_stopped(&mut self, request: &str, transfer: &str) -> Result<(), StoreError> {
        if self.encoding.as_ref().is_none_or(|owned| owned.0 != request || owned.1 != transfer) {
            return Err(SessionError::Stale.into());
        }
        self.encoding = None;
        Ok(())
    }
    /// Preserve the durable checkpoint after an observed restore failure, while leaving the
    /// current native page usable. Call only after observing any encoding worker's terminal.
    /// # Errors
    /// Refuses an unrelated request or bounded cancellation queue overload.
    pub fn restore_failed(&mut self, request: &str) -> Result<(), StoreError> {
        if !self.owns_restore(request) {
            return Err(SessionError::Stale.into());
        }
        self.cancel_restore()?;
        self.policy.protect_originals();
        let cancellation = self.identities.next()?;
        self.queue(cancellation, SessionOperation::Cancel { target_request_id: request.into() })
    }
    pub(super) fn update_restore_frontier(&mut self) -> Result<(), StoreError> {
        let current = self.policy.fence();
        let Some(restore) = &mut self.restore else {
            return Ok(());
        };
        if restore.sent {
            return Ok(());
        }
        if same_authored_frontier(&restore.fence, current) {
            restore.fence = current.clone();
            return Ok(());
        }
        let request = restore.request.clone();
        self.cancel_restore()?;
        let cancellation = self.identities.next()?;
        self.queue(cancellation, SessionOperation::Cancel { target_request_id: request })
    }
    pub(super) fn cancel_restore(&mut self) -> Result<(), StoreError> {
        let restore = self.restore.take().ok_or(SessionError::Stale)?;
        self.outbound = None;
        self.messages.retain(|message| message.request_id != restore.request);
        self.replay_retired = true;
        if self.policy.committed() {
            self.policy.reconciled(&self.policy.fence().clone()).map_err(application)?;
        }
        Ok(())
    }
}
fn same_authored_frontier(left: &SessionFence, right: &SessionFence) -> bool {
    left.webview == right.webview
        && left.bindings.len() == right.bindings.len()
        && left.bindings.iter().all(|binding| {
            right.bindings.iter().any(|current| {
                same_runtime(binding, current) && binding.graph_revision == current.graph_revision
            })
        })
}
