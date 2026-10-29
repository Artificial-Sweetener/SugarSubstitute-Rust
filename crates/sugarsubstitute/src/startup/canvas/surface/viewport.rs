// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate shared viewport commands with the authoritative camera and native acceptance.

use super::*;
use crate::native_canvas::session::{CameraCommandResult, CameraSettlement};
use sugarsubstitute_canvas::{NavigationCommand, ZoomMode};
use sugarsubstitute_contracts::canvas_surface::{
    CanvasViewportCommand, CanvasViewportMode, CanvasViewportState,
};

impl NativeSurface {
    /// Admit commands only on the selected, committed and visible attachment already authorized.
    /// A read uses the same revision stream and waits for settled presentation without moving it.
    pub(super) fn viewport(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        command: CanvasViewportCommand,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if !self.binding.as_ref().is_some_and(|binding| {
            binding.committed && binding.selected && binding.placement.is_some()
        }) {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        let navigation = match command {
            CanvasViewportCommand::Read => None,
            CanvasViewportCommand::Fit => Some(NavigationCommand::Fit),
            CanvasViewportCommand::Native => Some(NavigationCommand::OneToOne),
            CanvasViewportCommand::Zoom { factor } => Some(NavigationCommand::Zoom(factor)),
            CanvasViewportCommand::Pan { x, y } => Some(NavigationCommand::Pan([x, y])),
        };
        let admitted = match navigation {
            Some(command) => session.navigate(command)?,
            None => session.camera_intent(),
        };
        let CameraCommandResult::Accepted(intent) = admitted else {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        };
        self.pending = Some(Pending {
            identity,
            request,
            claim,
            kind: PendingKind::Viewport { intent, commanded: navigation.is_some() },
        });
        self.poll_viewport(session)
    }

    /// Complete only exact settled native evidence, including an already accepted unchanged view.
    /// Intermediate frames still pass through ordinary presentation to release their owned slot.
    pub(super) fn poll_viewport(&mut self, session: &Session) -> Result<(), Box<dyn Error>> {
        let Some(Pending { kind: PendingKind::Viewport { intent, .. }, .. }) = &self.pending else {
            return Ok(());
        };
        let state = match session.settled_camera(*intent) {
            CameraSettlement::Pending => return Ok(()),
            CameraSettlement::Failed => {
                let pending = self.pending.take().ok_or("Failed viewport request disappeared")?;
                return self.reply(
                    pending.identity,
                    &pending.request,
                    CanvasSurfaceReplyStatus::Error,
                    pending.claim,
                );
            }
            CameraSettlement::Superseded => None,
            CameraSettlement::Settled(camera) => {
                let mode = match intent.mode() {
                    ZoomMode::Fit => CanvasViewportMode::Fit,
                    ZoomMode::OneToOne => CanvasViewportMode::Native,
                    ZoomMode::Custom => CanvasViewportMode::Custom,
                };
                Some(CanvasViewportState::new(
                    mode,
                    camera.viewport.zoom(),
                    camera.extent.width(),
                    camera.extent.height(),
                )?)
            }
        };
        let pending = self.pending.take().ok_or("Viewport request disappeared")?;
        match state {
            Some(state) => {
                self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::viewport(
                    pending.identity,
                    &pending.request,
                    &state,
                    "",
                    pending.claim,
                )?))
            }
            None => self.reply(
                pending.identity,
                &pending.request,
                CanvasSurfaceReplyStatus::Stale,
                pending.claim,
            ),
        }
    }
}
