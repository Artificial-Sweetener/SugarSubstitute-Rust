// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate shared viewport commands with the authoritative camera and native acceptance.

use super::*;
use crate::native_canvas::session::{CameraCommandResult, CameraSettlement};
use sugarsubstitute_canvas::WipeOrientation;
use sugarsubstitute_canvas::{NavigationCommand, Viewport, ZoomMode};
use sugarsubstitute_contracts::canvas_surface::{
    CanvasViewportCommand, CanvasViewportMode, CanvasViewportState, CanvasWipeOrientation,
};

impl NativeSurface {
    /// Admit commands only on the selected, committed and visible attachment already authorized.
    /// A read uses the same revision stream and waits for settled presentation without moving it.
    /// A camera command is refused with `unavailable` while a routed native gesture holds
    /// capture; the navigation tool owns the camera until that gesture's release or cancel.
    pub(super) fn viewport(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        command: CanvasViewportCommand,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if !self.binding.as_ref().is_some_and(|binding| {
            binding.committed
                && (binding.selected || self.has_shown_preview())
                && binding.placement.is_some()
        }) {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        if self.showing_overview() {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        if let CanvasViewportCommand::Wipe { orientation, split } = command {
            if session.compared_extent().is_none() {
                return self.reply(
                    identity,
                    &request,
                    CanvasSurfaceReplyStatus::Unavailable,
                    claim,
                );
            }
            let orientation = match orientation {
                CanvasWipeOrientation::Vertical => WipeOrientation::Vertical,
                CanvasWipeOrientation::Horizontal => WipeOrientation::Horizontal,
            };
            if session.set_wipe(orientation, split).is_err() {
                return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
            }
        }
        let navigation = match command {
            CanvasViewportCommand::Read | CanvasViewportCommand::Wipe { .. } => None,
            CanvasViewportCommand::Fit => Some(NavigationCommand::Fit),
            CanvasViewportCommand::Native => Some(NavigationCommand::OneToOne),
            CanvasViewportCommand::Zoom { factor } => Some(NavigationCommand::Zoom(factor)),
            CanvasViewportCommand::Pan { x, y } => Some(NavigationCommand::Pan([x, y])),
            CanvasViewportCommand::Inspect(inspection) => Some(NavigationCommand::Frame {
                center: inspection.center(),
                span: inspection.span(),
            }),
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
                Some(with_comparison(
                    with_center(
                        CanvasViewportState::new(
                            mode,
                            camera.viewport.zoom(),
                            camera.extent.width(),
                            camera.extent.height(),
                        )?,
                        camera.viewport,
                        camera.extent,
                    )?,
                    session,
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

/// Report the image point at the accepted viewport's center, normalized to the image extent,
/// so the web inspection owner can record this camera and restore it later.
/// iewport is the native-accepted camera and image the presented image's extent.
pub(in crate::startup::canvas::surface) fn with_center(
    state: CanvasViewportState,
    viewport: Viewport,
    image: Extent,
) -> Result<CanvasViewportState, Box<dyn Error>> {
    let surface = viewport.surface();
    let point = viewport
        .document_at([f64::from(surface.width()) * 0.5, f64::from(surface.height()) * 0.5])?;
    let normalized = |value: f64, extent: u32| (value / f64::from(extent)).clamp(0.0, 1.0);
    Ok(state
        .with_center(normalized(point[0], image.width()), normalized(point[1], image.height()))?)
}

/// Report a loaded comparison original's extent beside the primary image's state.
pub(in crate::startup::canvas::surface) fn with_comparison(
    state: CanvasViewportState,
    session: &Session,
) -> Result<CanvasViewportState, Box<dyn Error>> {
    Ok(match session.compared_extent() {
        Some(extent) => state.with_secondary_dimensions(extent.width(), extent.height())?,
        None => state,
    })
}
