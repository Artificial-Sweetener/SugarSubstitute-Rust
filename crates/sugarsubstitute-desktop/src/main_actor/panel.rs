// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deliver bounded panel responses only to the current native top-document context.

use super::*;

pub(super) mod bridge;
mod delivery;
mod replies;
pub(super) use replies::Replies;

impl MainActor {
    /// Submit one host message to the same observed document without exposing native handles.
    pub(crate) fn send_panel(&self, command: &crate::PanelCommand) -> Result<(), DesktopError> {
        let payload = command
            .message()
            .encode()
            .map_err(|cause| DesktopError::new("encode_panel_message", cause))?;
        self.deliver(command.identity(), &payload)
    }

    /// Deliver a correlated surface reply to the same current document and UI apartment.
    pub(crate) fn send_surface(&mut self, reply: crate::SurfaceReply) -> Result<(), DesktopError> {
        self.admit_canvas_reply(reply.identity())?;
        self.canvas_replies.push(replies::Reply::Surface(reply))
    }

    /// Deliver a correlated context reply to the same current document and UI apartment.
    pub(crate) fn send_canvas_context(
        &mut self,
        reply: crate::CanvasContextReply,
    ) -> Result<(), DesktopError> {
        self.admit_canvas_reply(reply.identity())?;
        self.canvas_replies.push(replies::Reply::Context(reply))
    }

    fn admit_canvas_reply(&self, identity: crate::MainIdentity) -> Result<(), DesktopError> {
        self.admission.frontend(identity)?;
        if !self.revealed {
            return Err(DesktopError::message("send_panel", "main is not revealed"));
        }
        if self.readiness.as_ref().and_then(MainReadiness::panel_context).is_none() {
            return Err(DesktopError::message("send_panel", "current top document is absent"));
        }
        Ok(())
    }

    /// Wait for actual completion wakes when browser delivery slots are occupied.
    pub(super) fn canvas_reply_needs_progress(&self) -> bool {
        !self.canvas_replies.is_empty()
            && self
                .receivers
                .as_ref()
                .is_none_or(|receivers| receivers.admits_surface_delivery().unwrap_or(true))
    }

    /// Submit one retained response when its current document has browser capacity.
    pub(super) fn progress_canvas_reply(&mut self) -> Result<bool, DesktopError> {
        let receivers = self
            .receivers
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_receivers", "receivers not created"))?;
        let admitted = receivers.admits_surface_delivery()?;
        let admission = &self.admission;
        let readiness = &self.readiness;
        let revealed = self.revealed;
        self.canvas_replies.progress(admitted, |identity, payload| {
            Self::deliver_to(admission, readiness, revealed, receivers, identity, payload)
        })
    }

    /// Use one navigation-qualified delivery mechanism for panel and canvas replies.
    fn deliver(&self, identity: crate::MainIdentity, payload: &[u8]) -> Result<(), DesktopError> {
        Self::deliver_to(
            &self.admission,
            &self.readiness,
            self.revealed,
            self.receivers()?,
            identity,
            payload,
        )
    }

    fn deliver_to(
        admission: &Admission,
        readiness: &Option<MainReadiness>,
        revealed: bool,
        receivers: &crate::windows_main_receivers::Receivers,
        identity: crate::MainIdentity,
        payload: &[u8],
    ) -> Result<(), DesktopError> {
        admission.frontend(identity)?;
        if !revealed {
            return Err(DesktopError::message("send_panel", "main is not revealed"));
        }
        let context = readiness
            .as_ref()
            .and_then(MainReadiness::panel_context)
            .ok_or_else(|| DesktopError::message("send_panel", "current top document is absent"))?;
        let parameters = delivery::encode(context, payload)?;
        receivers.call(Command::PanelDelivery(context), "Runtime.evaluate", &parameters)
    }
}
