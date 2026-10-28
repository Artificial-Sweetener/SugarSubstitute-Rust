// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute framed browser input and transfer only its preclaimed lane allocation.

use crate::{
    DesktopError, DesktopStartupObservation,
    main_actor::{MainActor, MainReadiness},
};

/// Keep decoded application input separate from native publication after callback-queue borrowing.
pub(in crate::main_actor) enum Ingress {
    Panel(crate::PanelObservation),
    Surface(crate::SurfaceObservation),
    Context(crate::CanvasContextObservation),
}

impl MainActor {
    /// Decode and consume tickets using authored bounded work only; never call native effects here.
    pub(in crate::main_actor) fn decode_panel_ingress(
        &mut self,
        payload: &str,
    ) -> Result<Option<Ingress>, DesktopError> {
        if !self.revealed {
            return Ok(None);
        }
        let Some(context) = self.readiness.as_ref().and_then(MainReadiness::panel_context) else {
            return Ok(None);
        };
        let Some(frame) = crate::main_panel_ingress::frame(payload, Some(context)) else {
            return Ok(None);
        };
        if !self.panel_transport.issued(context, frame.lane, frame.ticket) {
            return Ok(None);
        }
        let session = sugarsubstitute_contracts::panel::PanelSession::for_main(self.identity())
            .map_err(|cause| DesktopError::new("panel_session_identity", cause))?;
        let Some(message) = crate::main_panel_ingress::decode_payload(frame.payload(), &session)
        else {
            self.panel_transport.reject(frame.lane, frame.ticket);
            return Ok(None);
        };
        if message.lane() != frame.lane {
            self.panel_transport.reject(frame.lane, frame.ticket);
            return Ok(None);
        }
        let claim = match self.panel_transport.consume(
            context,
            frame.lane,
            frame.ticket,
            message.request_id(),
        )? {
            Some(claim) => claim,
            None => {
                tracing::debug!(operation="panel_ticket_rejected", request_id=?message.request_id(),
                    "Cancellation no longer owns the active ordinary request");
                self.panel_transport.reject(frame.lane, frame.ticket);
                return Ok(None);
            }
        };
        let lease = self.request()?.storage_lease().clone();
        Ok(Some(match message {
            crate::main_panel_ingress::ClientMessage::Panel(message) => {
                Ingress::Panel(crate::PanelObservation::new(self.identity(), message, lease, claim))
            }
            crate::main_panel_ingress::ClientMessage::Surface(message) => Ingress::Surface(
                crate::SurfaceObservation::new(self.identity(), message, lease, claim),
            ),
            crate::main_panel_ingress::ClientMessage::Context(request) => Ingress::Context(
                crate::CanvasContextObservation::new(self.identity(), request, lease, claim),
            ),
        }))
    }

    /// Publish after queue borrowing ends so native geometry getters cannot reenter a borrowed queue.
    pub(in crate::main_actor) fn publish_panel_ingress(
        &mut self,
        ingress: Ingress,
    ) -> DesktopStartupObservation {
        match ingress {
            Ingress::Panel(observation) => DesktopStartupObservation::PanelMessage(observation),
            Ingress::Surface(observation) => {
                #[cfg(feature = "native-composition")]
                let mut observation = observation;
                #[cfg(feature = "native-composition")]
                if matches!(observation.message(), sugarsubstitute_contracts::canvas_surface::CanvasSurfaceMessage::Request(request)
                    if matches!(request.operation(), sugarsubstitute_contracts::canvas_surface::CanvasSurfaceOperation::Geometry(_)))
                {
                    match self.canvas_allocation() {
                        Ok(allocation) => observation.set_allocation(allocation),
                        Err(cause) => {
                            tracing::warn!(operation="canvas_geometry_observation", cause=%cause, "Native WebView allocation is unavailable")
                        }
                    }
                }
                DesktopStartupObservation::SurfaceMessage(observation)
            }
            Ingress::Context(observation) => {
                #[cfg(feature = "k1-native-qualification")]
                crate::qualification_trace::progress(
                    crate::qualification_trace::Progress::CanvasContextObserved,
                    self.identity(),
                );
                DesktopStartupObservation::CanvasContextMessage(observation)
            }
        }
    }
}
