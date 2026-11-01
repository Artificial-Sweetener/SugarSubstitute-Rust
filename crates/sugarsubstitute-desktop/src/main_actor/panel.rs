// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deliver bounded panel responses only to the current native top-document context.

use super::*;

pub(super) mod admission;
pub(super) mod bridge;
mod delivery;
pub(super) mod ingress;
mod messages;
mod replies;
use crate::main_native_delivery::{DeliveryCorrelation, NativeDelivery, OwnedDelivery};
pub(super) use messages::HostMessages;
pub(super) use replies::Replies;

impl MainActor {
    /// Retain one host message for the current document until a panel command slot is free.
    pub(crate) fn send_panel(&mut self, command: crate::PanelCommand) -> Result<(), DesktopError> {
        let context = self.admitted_panel_context(command.identity())?;
        self.panel_messages.push(messages::HostMessage::new(command, context)?)
    }

    /// Deliver a correlated surface reply to the same current document and UI apartment.
    pub(crate) fn send_surface(&mut self, reply: crate::SurfaceReply) -> Result<(), DesktopError> {
        self.admitted_panel_context(reply.identity())?;
        self.canvas_replies.push(NativeDelivery::Surface(reply))
    }

    /// Retain a full asynchronous snapshot until native capacity returns; stale inputs retire here.
    pub(crate) fn send_surface_presentation(
        &mut self,
        change: crate::SurfacePresentation,
    ) -> Result<(), DesktopError> {
        if change.identity() != self.identity()
            || self.admission.closing()
            || !self.revealed
            || self.readiness.as_ref().and_then(MainReadiness::panel_context).is_none()
        {
            return Ok(());
        }
        self.admitted_panel_context(change.identity())?;
        self.canvas_replies.push(NativeDelivery::Presentation(change))
    }

    /// Deliver a correlated context reply to the same current document and UI apartment.
    pub(crate) fn send_canvas_context(
        &mut self,
        reply: crate::CanvasContextReply,
    ) -> Result<(), DesktopError> {
        self.admitted_panel_context(reply.identity())?;
        self.canvas_replies.push(NativeDelivery::Context(reply))
    }

    /// Acknowledge consumed control/cancellation without asserting original mutation completion.
    pub(crate) fn complete_panel_ingress(
        &mut self,
        receipt: crate::PanelIngressReceipt,
    ) -> Result<(), DesktopError> {
        self.admitted_panel_context(receipt.identity())?;
        self.canvas_replies.push(NativeDelivery::Ingress(receipt))
    }

    /// Return the revealed current top-document context that may receive this identity's traffic.
    fn admitted_panel_context(&self, identity: crate::MainIdentity) -> Result<i64, DesktopError> {
        self.admission.frontend(identity)?;
        if !self.revealed {
            return Err(DesktopError::message("send_panel", "main is not revealed"));
        }
        self.readiness
            .as_ref()
            .and_then(MainReadiness::panel_context)
            .ok_or_else(|| DesktopError::message("send_panel", "current top document is absent"))
    }

    /// Wait for actual completion wakes when the panel command slots are occupied.
    pub(super) fn panel_message_needs_progress(&self) -> bool {
        !self.panel_messages.is_empty()
            && self
                .receivers
                .as_ref()
                .is_none_or(|receivers| receivers.admits_panel_delivery().unwrap_or(true))
    }

    /// Retire host messages for a superseded document, then submit one when a slot is free.
    pub(super) fn progress_panel_messages(&mut self) -> Result<bool, DesktopError> {
        let current = (self.revealed && !self.admission.closing())
            .then(|| self.readiness.as_ref().and_then(MainReadiness::panel_context))
            .flatten()
            .map(|context| (self.identity(), context));
        let receivers = self
            .receivers
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_receivers", "receivers not created"))?;
        let admitted = receivers.admits_panel_delivery()?;
        self.panel_messages.progress(current, admitted, |message| {
            receivers.call(
                Command::PanelDelivery(message.context()),
                "Runtime.evaluate",
                message.parameters(),
            )
        })
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
        let current = (self.revealed
            && !self.admission.closing()
            && self.readiness.as_ref().and_then(MainReadiness::panel_context).is_some())
        .then(|| self.identity());
        if self.canvas_replies.retire_presentations(current)? {
            return Ok(true);
        }
        let receivers = self
            .receivers
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_receivers", "receivers not created"))?;
        let admitted = receivers.admits_surface_delivery()?;
        let admission = &self.admission;
        let readiness = &self.readiness;
        let revealed = self.revealed;
        let transport = &mut self.panel_transport;
        self.canvas_replies.progress(admitted, |delivery| {
            if delivery.lane().is_none()
                && (delivery.identity() != admission.identity()
                    || admission.closing()
                    || !revealed
                    || readiness.as_ref().and_then(MainReadiness::panel_context).is_none())
            {
                return Ok(());
            }
            admission.frontend(delivery.identity())?;
            if !revealed {
                return Err(DesktopError::message("send_panel", "main is not revealed"));
            }
            let context =
                readiness.as_ref().and_then(MainReadiness::panel_context).ok_or_else(|| {
                    DesktopError::message("send_panel", "current top document is absent")
                })?;
            let (parameters, correlation) = if let Some(lane) = delivery.lane() {
                let ticket = transport.begin_delivery(&delivery)?;
                let parameters = match delivery.payload() {
                    Some(payload) => delivery::encode_owned(context, payload, ticket)?,
                    None => delivery::transport(context, "a", lane, ticket)?,
                };
                (parameters, DeliveryCorrelation::Transport(ticket))
            } else {
                let payload = delivery.payload().ok_or_else(|| {
                    DesktopError::message("send_panel", "notification payload is absent")
                })?;
                (delivery::encode(context, payload)?, DeliveryCorrelation::Presentation)
            };
            receivers.deliver(&parameters, OwnedDelivery { context, correlation, delivery })
        })
    }

    /// Return capacity only through finite native grant/refusal work, never through a timer.
    pub(super) fn panel_transport_needs_progress(&self) -> bool {
        self.panel_transport.needs_progress()
            && self
                .receivers
                .as_ref()
                .is_some_and(|receivers| receivers.admits_surface_delivery().unwrap_or(true))
    }

    /// Drive one private transport action after observing the current native document.
    pub(super) fn progress_panel_transport(&mut self) -> Result<bool, DesktopError> {
        let context = self
            .revealed
            .then(|| self.readiness.as_ref().and_then(MainReadiness::panel_context))
            .flatten();
        self.panel_transport.synchronize(context);
        if self.events()?.take_panel_overflow() {
            self.panel_transport.overflow();
        }
        let Some(context) = context else {
            return Ok(false);
        };
        if !self.receivers()?.admits_surface_delivery()? {
            return Ok(false);
        }
        let Some(action) = self.panel_transport.action()? else {
            return Ok(false);
        };
        let parameters = delivery::transport(context, action.code(), action.lane, action.ticket)?;
        self.receivers()?.call(
            Command::PanelTransport { context, action },
            "Runtime.evaluate",
            &parameters,
        )?;
        Ok(true)
    }
}
