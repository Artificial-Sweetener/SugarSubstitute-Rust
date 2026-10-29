// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Complete refused or cancelled ingress while preserving its original response custody.

use super::*;
use sugarsubstitute_contracts::canvas_surface::{
    CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryStatus,
};
use sugarsubstitute_desktop::PanelIngressReceipt;

impl NativeSurface {
    /// Retire an exact context wait or mutation before acknowledging cancellation consumption.
    /// The ordinary terminal reply keeps its own claim independently of the cancel lane.
    pub(in crate::startup::canvas) fn cancel(
        &mut self,
        identity: MainIdentity,
        request_id: u64,
        claim: MemoryClaim,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        if let Some(index) = self.contexts.iter().position(|slot| {
            slot.as_ref().is_some_and(|context| {
                context.identity == identity && context.request.request_id() == request_id
            })
        }) {
            let context = self.contexts[index].take().ok_or("Canvas context disappeared")?;
            self.enqueue(DesktopStartupCommand::SendCanvasContext(CanvasContextReply::cancelled(
                context.identity,
                &context.request,
                context.claim,
            )?))?;
        } else if self.pending.as_ref().is_some_and(|pending| {
            pending.identity == identity && pending.request.request_id() == request_id
        }) {
            let session = session.ok_or("An active canvas operation lost its session")?;
            let pending = self.pending.take().ok_or("Pending canvas request disappeared")?;
            if let PendingKind::Viewport { intent, commanded: true } = pending.kind {
                session.cancel_camera(intent)?;
            }
            if matches!(pending.kind, PendingKind::Select)
                && let Some(binding) = &mut self.binding
            {
                session.cancel_selection()?;
                binding.selected = false;
            }
            if matches!(pending.kind, PendingKind::Prepare) {
                self.binding = self.retained.take();
                session.cancel_document_preparation();
                self.resume_document(session)?;
            }
            self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Cancelled,
                pending.claim,
            )?;
        }
        self.enqueue(DesktopStartupCommand::CompletePanelIngress(
            PanelIngressReceipt::cancellation(identity, request_id, claim)?,
        ))
    }

    /// Refuse current-page work that has no application policy or initialized canvas authority.
    /// Cancellation still retires the exact earlier wait without admitting a new operation.
    pub(in crate::startup::canvas) fn refuse(
        &mut self,
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        claim: MemoryClaim,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        match message {
            CanvasSurfaceMessage::Cancel { request_id, .. } => {
                self.cancel(identity, request_id, claim, session)
            }
            CanvasSurfaceMessage::Request(request) => {
                self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim)
            }
            CanvasSurfaceMessage::Discovery(request) => {
                self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::discovery(
                    identity,
                    &request,
                    CanvasSurfaceDiscoveryStatus::Unavailable,
                    &CanvasSurfaceCapabilities::default(),
                    "",
                    claim,
                )?))
            }
        }
    }

    /// Return a correlated empty context rather than abandoning an admitted policy refusal.
    pub(in crate::startup::canvas) fn refuse_context(
        &mut self,
        identity: MainIdentity,
        request: &CanvasContextRequest,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        self.enqueue(DesktopStartupCommand::SendCanvasContext(CanvasContextReply::new(
            identity,
            request,
            CanvasDisplayCapability::Unavailable,
            claim,
        )?))
    }
}
