// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe the admitted native output surface without changing attachment authority.

use super::*;
use sugarsubstitute_contracts::canvas_surface::{
    CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryRequest, CanvasSurfaceDiscoveryStatus,
    CanvasSurfaceImageFormat, CanvasSurfaceViewportCommand,
};

impl NativeSurface {
    /// Publish only implemented support while this exact navigation has a usable native owner.
    /// Discovery neither prepares a document nor changes the attachment's operation revision.
    pub(super) fn discover(
        &mut self,
        identity: MainIdentity,
        request: &CanvasSurfaceDiscoveryRequest,
        claim: MemoryClaim,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        let available = session.initialized() && self.available(identity);
        let capabilities = CanvasSurfaceCapabilities {
            image_formats: if available { vec![CanvasSurfaceImageFormat::Png] } else { Vec::new() },
            viewport_commands: if available {
                vec![
                    CanvasSurfaceViewportCommand::Fit,
                    CanvasSurfaceViewportCommand::Native,
                    CanvasSurfaceViewportCommand::Zoom,
                    CanvasSurfaceViewportCommand::Pan,
                ]
            } else {
                Vec::new()
            },
            viewport_state: available,
            ..Default::default()
        };
        let status = if available {
            CanvasSurfaceDiscoveryStatus::Available
        } else {
            CanvasSurfaceDiscoveryStatus::Unavailable
        };
        let reply = SurfaceReply::discovery(identity, request, status, &capabilities, "", claim)?;
        self.enqueue(DesktopStartupCommand::SendSurface(reply))
    }
}
