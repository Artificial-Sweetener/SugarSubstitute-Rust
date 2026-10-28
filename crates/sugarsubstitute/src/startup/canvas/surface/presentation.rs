// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve current surface requests only against actual native presentation effects.

use super::*;

impl NativeSurface {
    /// Keep acknowledged native pixels during a same-attachment source replacement.
    /// Changed geometry, hidden surfaces and revoked bindings must withdraw them instead.
    pub(in crate::startup::canvas) fn presentation<'a>(
        &self,
        session: &'a Session,
    ) -> sugarsubstitute_desktop::canvas::MainCanvasPresentation<'a> {
        use sugarsubstitute_desktop::canvas::MainCanvasPresentation;
        if let Some(frame) = self.frame(session) {
            return MainCanvasPresentation::Frame(frame);
        }
        let retained = self.binding.as_ref().and_then(|binding| {
            let presented = binding.presented?;
            if self.closed
                || !binding.committed
                || binding.placement != Some(presented.placement)
                || binding.allocation != Some(presented.allocation)
            {
                return None;
            }
            Some(presented)
        });
        retained.map_or(MainCanvasPresentation::Hide, MainCanvasPresentation::Retain)
    }

    /// Resolve initial selection only once actual current pixels have been prepared.
    pub(in crate::startup::canvas) fn poll(
        &mut self,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        if session.initialized() {
            self.flush_contexts()?;
        }
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
            && self.binding.as_ref().is_some_and(|binding| !binding.committed)
            && session.selected_cpu_frame().is_some()
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
        Ok(())
    }

    /// Borrow the selected current frame only after the attachment requests commit.
    pub(in crate::startup::canvas) fn frame<'a>(
        &self,
        session: &'a Session,
    ) -> Option<MainCanvasFrame<'a>> {
        let binding = self.binding.as_ref()?;
        if !binding.selected || !binding.committed {
            return None;
        }
        let (frame, memory) = session.selected_cpu_frame()?;
        Some(MainCanvasFrame {
            identity: binding.identity,
            placement: binding.placement?,
            allocation: binding.allocation?,
            memory,
            graphics: session.host_graphics()?,
            frame,
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
        let (frame, _) =
            session.selected_cpu_frame().ok_or("Selected canvas frame is unavailable")?;
        if frame.target != target {
            return Err("Native canvas acknowledged a stale frame".into());
        }
        if frame.pending {
            session.did_present_cpu(target)?;
        }
        let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
        binding.presented = Some(MainCanvasRetention {
            identity: binding.identity,
            target,
            placement: binding.placement.ok_or("Canvas placement disappeared")?,
            allocation: binding.allocation.ok_or("Canvas allocation disappeared")?,
        });
        if let Some(pending) = self.pending.take() {
            self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Applied,
                pending.claim,
            )?;
        }
        Ok(())
    }

    /// Resolve hidden geometry only after the main window's native hide completed in composition.
    pub(in crate::startup::canvas) fn hidden(
        &mut self,
        identity: MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(binding) = &mut self.binding
            && binding.identity == identity
        {
            binding.presented = None;
        }
        let Some(pending) = self.pending.as_ref() else { return Ok(()) };
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
