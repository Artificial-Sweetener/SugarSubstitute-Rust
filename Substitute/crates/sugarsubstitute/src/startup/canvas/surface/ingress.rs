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
            self.enqueue(DesktopStartupCommand::SendCanvasNative(CanvasNativeReply::new(
                context.identity,
                context.request.envelope().cancelled_reply()?,
                context.claim,
            )?))?;
        } else if self.cancel_deferred(identity, request_id)? {
            // The held candidate was answered `cancelled`; no session work was ever admitted.
        } else if self.pending.as_ref().is_some_and(|pending| {
            pending.identity == identity && pending.request.request_id() == request_id
        }) {
            let session = session.ok_or("An active canvas operation lost its session")?;
            self.withdraw_pending(session, CanvasSurfaceReplyStatus::Cancelled)?;
        } else if !self.cancel_workflow(identity, request_id)?
            && !self.cancel_capture(identity, request_id, session)?
        {
            super::lifecycle::unmatched(request_id);
        }
        self.enqueue(DesktopStartupCommand::CompletePanelIngress(
            PanelIngressReceipt::cancellation(identity, request_id, claim)?,
        ))
    }

    /// Withdraw the pending request's session intent and answer it with `status`.
    fn withdraw_pending(
        &mut self,
        session: &mut Session,
        status: CanvasSurfaceReplyStatus,
    ) -> Result<(), Box<dyn Error>> {
        let pending = self.pending.take().ok_or("Pending canvas request disappeared")?;
        if let PendingKind::Viewport { intent, commanded: true, .. } = pending.kind {
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
        self.reply(pending.identity, &pending.request, status, pending.claim)
    }

    /// Let a release of the attachment end its pending selection or viewport wait, which a
    /// hidden committed attachment defers until it is shown again. The wait is answered
    /// `unavailable` before the release applies. Returns whether a wait was withdrawn.
    pub(super) fn withdraw_for_release(
        &mut self,
        identity: MainIdentity,
        release: &CanvasSurfaceRequest,
        session: &mut Session,
    ) -> Result<bool, Box<dyn Error>> {
        let releasable = matches!(release.operation(), CanvasSurfaceOperation::Release)
            && self.pending.as_ref().is_some_and(|pending| {
                matches!(pending.kind, PendingKind::Select | PendingKind::Viewport { .. })
                    && pending.identity == identity
                    && pending.request.address() == release.address()
                    && pending.request.revision() < release.revision()
            });
        if releasable {
            self.withdraw_pending(session, CanvasSurfaceReplyStatus::Unavailable)?;
        }
        Ok(releasable)
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
                self.rejected_schema = (!matches!(request.schema(), 2 | 3)).then_some(identity);
                self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::discovery(
                    identity,
                    &request,
                    CanvasSurfaceDiscoveryStatus::Unavailable,
                    &CanvasSurfaceCapabilities::default(),
                    "",
                    claim,
                )?))
            }
            CanvasSurfaceMessage::Presentation(request) => {
                self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::presentation(
                    identity,
                    &request,
                    CanvasSurfaceDiscoveryStatus::Unavailable,
                    false,
                    "",
                    claim,
                )?))
            }
        }
    }
}
