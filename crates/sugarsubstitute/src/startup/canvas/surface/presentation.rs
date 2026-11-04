// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve current surface requests only against actual native presentation effects.

use super::*;

impl NativeSurface {
    /// Keep acknowledged native pixels during a same-attachment source replacement, and show
    /// them at changed geometry until a frame at that geometry is presented. Hidden surfaces
    /// and revoked bindings withdraw them instead, as does a binding whose presented input
    /// document left the screen without a document to replace it, such as the Output route
    /// with no selected output.
    pub(in crate::startup::canvas) fn presentation<'a>(
        &self,
        session: &'a Session,
    ) -> sugarsubstitute_desktop::canvas::MainCanvasPresentation<'a> {
        use sugarsubstitute_desktop::canvas::MainCanvasPresentation;
        if self.preview_restore_requires_hide(session) {
            return MainCanvasPresentation::Hide;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::EmptyCommit))
        {
            return MainCanvasPresentation::Hide;
        }
        if let Some(frame) = self.frame(session) {
            return MainCanvasPresentation::Frame(frame);
        }
        let retained = self
            .binding
            .as_ref()
            .filter(|binding| binding.presented.is_some())
            .or(self.retained.as_ref())
            .and_then(|binding| {
                let presented = binding.presented?;
                if self.closed
                    || !binding.committed
                    || (binding.presented_input && !binding.shows_document(session))
                {
                    return None;
                }
                Some(MainCanvasRetention {
                    shown: binding.placement?,
                    allocation: binding.allocation?,
                    ..presented
                })
            });
        retained.map_or(MainCanvasPresentation::Hide, MainCanvasPresentation::Retain)
    }

    /// Resolve initial selection only once actual current pixels have been prepared.
    pub(in crate::startup::canvas) fn poll(
        &mut self,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        self.poll_previews(session)?;
        if self.closed {
            return Ok(());
        }
        self.poll_editing(session)?;
        self.publish_settled_gesture(session)?;
        if self.poll_attachment(session)? {
            return Ok(());
        }
        if session.initialized() {
            self.flush_contexts(session)?;
            self.poll_workflow(session)?;
            self.poll_sources(session)?;
        }
        self.follow_rebuilt_selection(session);
        let Some(pending) = &self.pending else { return Ok(()) };
        if !matches!(pending.kind, PendingKind::Hide) && session.source_failed() {
            let pending = self.pending.take().ok_or("Pending canvas request disappeared")?;
            if let Some(binding) = &mut self.binding {
                binding.selected = false;
            }
            return self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Error,
                pending.claim,
            );
        }
        if matches!(pending.kind, PendingKind::Select)
            && self.binding.as_ref().is_some_and(|binding| {
                !binding.committed
                    && binding.source_intent_revision == Some(session.source_intent_revision())
            })
            && session.selected_frame().is_some()
            && session.host_graphics().is_some()
        {
            let pending = self.pending.take().ok_or("Pending canvas request disappeared")?;
            self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Applied,
                pending.claim,
            )?;
        }
        self.trace_selection_wait(session);
        self.poll_viewport(session)
    }

    /// Keep a pending selection answerable when the session rebuilt its corrupt cached source.
    /// The rebuild re-imports the same original under a new source intent; the selection's
    /// reply then waits on that rebuild's frame instead of the fenced corrupt one.
    fn follow_rebuilt_selection(&mut self, session: &Session) {
        let (Some(pending), Some(binding)) = (&self.pending, &mut self.binding) else { return };
        if !matches!(pending.kind, PendingKind::Select) {
            return;
        }
        let Some(offered) = binding.source_intent_revision else { return };
        if let Some(rebuilt) = session.rebuilt_intent(offered) {
            binding.source_intent_revision = Some(rebuilt);
            super::lifecycle::rebound(&pending.request, offered, rebuilt);
        }
    }

    /// Borrow the current frame only after the attachment requests commit and the binding
    /// shows a document or a preview.
    pub(in crate::startup::canvas) fn frame<'a>(
        &self,
        session: &'a Session,
    ) -> Option<MainCanvasFrame<'a>> {
        let binding = if self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::Prepare))
        {
            self.retained.as_ref()?
        } else if self.restoring {
            return None;
        } else {
            self.binding.as_ref()?
        };
        if (!binding.shows_document(session)
            && !self.has_preview_candidate()
            && !self.has_shown_preview())
            || !binding.committed
        {
            return None;
        }
        let (content, memory) = session.selected_frame()?;
        Some(MainCanvasFrame {
            identity: binding.identity,
            placement: binding.placement?,
            allocation: binding.allocation?,
            memory,
            graphics: session.host_graphics()?,
            content,
        })
    }

    /// Resolve the operation only after this exact frame and placement reached the native tree.
    pub(in crate::startup::canvas) fn presented(
        &mut self,
        target: FrameStamp,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if self.pending.as_ref().is_some_and(|pending| matches!(pending.kind, PendingKind::Hide)) {
            return Err("Native canvas presented while a hide is pending".into());
        }
        self.follow_rebuilt_selection(session);
        if session.preview_frame_acknowledgeable(target) {
            session.did_present_cpu(target)?;
        } else {
            let (content, _) =
                session.selected_frame().ok_or("Selected canvas frame is unavailable")?;
            if content.target() != target {
                return Err("Native canvas acknowledged a stale frame".into());
            }
            if content.pending() {
                session.did_present_selected(target)?;
            }
        }
        let preview = self.is_preview_frame(target, session);
        let observed = Self::observed_viewport(session, preview)?;
        let accepted = session.accepted_camera();
        let preparing = self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::Prepare));
        let binding = if preparing { self.retained.as_mut() } else { self.binding.as_mut() }
            .ok_or("Canvas binding disappeared")?;
        let promoting = binding.document.is_some();
        if let Some(token) = binding.document {
            session.promote_document(token, target)?;
            binding.document = None;
        }
        let placement = binding.placement.ok_or("Canvas placement disappeared")?;
        binding.presented = Some(MainCanvasRetention {
            identity: binding.identity,
            target,
            placement,
            shown: placement,
            allocation: binding.allocation.ok_or("Canvas allocation disappeared")?,
        });
        binding.presented_input = !preview && session.input_shown();
        let final_selected = !preview
            && accepted.is_some_and(|camera| {
                Some(camera.source_revision) == binding.source_intent_revision
            });
        if final_selected {
            binding.presented_selection_revision = binding.selection_revision;
            binding.presented_overview = binding.overview;
            binding.presented_viewport = observed;
        }
        if promoting {
            self.retained = None;
        }
        self.preview_presented(target, session)?;
        self.publish_settled_gesture(session)?;
        let viewport = self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::Viewport { .. }));
        if !preparing
            && !viewport
            && !self.pending.as_ref().is_some_and(|pending| {
                matches!(pending.kind, PendingKind::Select) && !final_selected
            })
            && let Some(pending) = self.pending.take()
        {
            self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Applied,
                pending.claim,
            )?;
        }
        self.trace_selection_wait(session);
        self.poll_viewport(session)
    }

    /// Resolve hidden geometry only after the main window's native hide completed in composition.
    pub(in crate::startup::canvas) fn hidden(
        &mut self,
        identity: MainIdentity,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if !self.binding.as_ref().is_some_and(|binding| binding.identity == identity) {
            return Ok(());
        }
        if let Some(binding) = &mut self.binding
            && binding.identity == identity
        {
            binding.presented = None;
            binding.presented_viewport = None;
        }
        self.preview_hidden(session)?;
        let Some(pending) = self.pending.as_ref() else { return Ok(()) };
        if pending.identity == identity && matches!(pending.kind, PendingKind::EmptyCommit) {
            let binding = self.binding.as_mut().ok_or("Empty canvas binding disappeared")?;
            if let Some(token) = binding.document {
                session.promote_empty_document(token)?;
                binding.document = None;
            }
            self.retained = None;
            let pending = self.pending.take().ok_or("Empty commit disappeared")?;
            return self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Applied,
                pending.claim,
            );
        }
        if pending.identity != identity || !matches!(pending.kind, PendingKind::Hide) {
            return Ok(());
        }
        let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
        if binding.identity != identity
            || binding.placement.is_some()
            || binding.revision != pending.request.revision()
            || binding.address != *pending.request.address()
        {
            return Err("Native canvas hide acknowledgement is stale".into());
        }
        let pending = self.pending.take().ok_or("Pending canvas hide disappeared")?;
        self.reply(
            pending.identity,
            &pending.request,
            CanvasSurfaceReplyStatus::Applied,
            pending.claim,
        )
    }
}
