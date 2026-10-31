// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the committed binding while an exact-address candidate prepares or rolls back.

use super::*;

impl NativeSurface {
    /// Admit a single candidate without destroying the preceding native presentation.
    pub(super) fn prepare(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if self.binding.as_ref().is_some_and(|binding| {
            binding.identity == identity && binding.address == *request.address()
        }) {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Stale, claim);
        }
        if self.retained.is_some()
            || self.binding.as_ref().is_some_and(|binding| binding.document.is_some())
        {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        self.fence_previews(session)?;
        session.input(Input::Drawable(false))?;
        self.retained = self.binding.take();
        self.binding = Some(Binding {
            document: None,
            identity,
            backend: self.backend,
            address: request.address().clone(),
            revision: request.revision(),
            placement: None,
            allocation: None,
            selected: false,
            selection_revision: 0,
            source_intent_revision: None,
            presented_selection_revision: 0,
            presented_viewport: None,
            committed: false,
            presented: None,
        });
        self.pending = Some(Pending { identity, request, claim, kind: PendingKind::Prepare });
        self.poll_attachment(session)?;
        Ok(())
    }

    /// Release a candidate through observed rollback, or withdraw the committed attachment.
    pub(super) fn release(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        self.fence_previews(session)?;
        if let Some(token) = self.binding.as_ref().and_then(|binding| binding.document) {
            session.restore_document(token)?;
            self.restoring = true;
            self.pending = Some(Pending { identity, request, claim, kind: PendingKind::Restore });
            return Ok(());
        }
        self.binding = None;
        session.revoke_acquirer()?;
        session.revoke_presentation()?;
        self.reply(identity, &request, CanvasSurfaceReplyStatus::Applied, claim)
    }

    /// Resolve preparation/restoration only after the session transferred complete custody.
    pub(super) fn poll_attachment(
        &mut self,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        if self.restoring {
            let token = self
                .binding
                .as_ref()
                .and_then(|binding| binding.document)
                .ok_or("Restoring canvas document identity disappeared")?;
            if session.document_transaction_pending(token) {
                return Ok(true);
            }
            self.binding = self.retained.take();
            self.restoring = false;
            self.resume_document(session)?;
            if let Some(pending) = self.pending.take() {
                self.reply(
                    pending.identity,
                    &pending.request,
                    CanvasSurfaceReplyStatus::Applied,
                    pending.claim,
                )?;
            }
            return Ok(true);
        }
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::Prepare))
        {
            return Ok(false);
        }
        if let Some(token) = session.prepare_document()? {
            self.binding.as_mut().ok_or("Preparing canvas binding disappeared")?.document =
                Some(token);
            let pending = self.pending.take().ok_or("Canvas preparation disappeared")?;
            self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Ready,
                pending.claim,
            )?;
        }
        Ok(true)
    }

    /// Resume presentation only when the preceding binding remains committed and visible.
    pub(super) fn resume_document(&self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        session.input(Input::Drawable(
            self.binding
                .as_ref()
                .is_some_and(|binding| binding.committed && binding.placement.is_some()),
        ))
    }

    /// Revoke all presentation authority while the session retains native cleanup custody.
    pub(super) fn discard_attachment(
        &mut self,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(session) = session {
            self.fence_previews(session)?;
            if let Some(token) = self.binding.as_ref().and_then(|binding| binding.document) {
                session.restore_document(token)?;
            } else {
                session.cancel_document_preparation();
            }
            session.revoke_acquirer()?;
            session.revoke_presentation()?;
        }
        self.binding = None;
        self.retained = None;
        self.restoring = false;
        Ok(())
    }
}
