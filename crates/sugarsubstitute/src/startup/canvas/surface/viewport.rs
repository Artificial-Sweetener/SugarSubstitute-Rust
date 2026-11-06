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
use sugarsubstitute_contracts::execution::Moment;

/// Longest a viewport request waits for its camera to settle before it is answered
/// `unavailable`. A camera settles within a few frames; the budget keeps a canvas hidden for
/// long, or a presenter that never acknowledges its frame, from holding the request and every
/// later surface request behind it until the page's own 30 second deadline.
const SETTLE_BUDGET: Duration = Duration::from_secs(10);

impl NativeSurface {
    /// Admit commands only on the committed and visible attachment already authorized while it
    /// shows a document or a preview.
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
                && (binding.shows_document(session) || self.has_shown_preview())
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
            Some(command) => match session.navigate(command) {
                Ok(admitted) => admitted,
                Err(cause) => {
                    tracing::warn!(operation = "canvas_viewport_rejected", request_id = request.request_id(), cause = %cause,
                        "The requested camera cannot be represented; the preceding camera is retained");
                    super::lifecycle::replied(&request, CanvasSurfaceReplyStatus::Error);
                    return self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::new(
                        identity, &request, CanvasSurfaceReplyStatus::Error,
                        "The saved camera cannot be represented by the native canvas. Its preceding camera is retained.", claim,
                    )?));
                }
            },
            None => session.camera_intent(),
        };
        let CameraCommandResult::Accepted(intent) = admitted else {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        };
        let admitted = session.preview_now();
        self.hold(
            identity,
            request,
            claim,
            PendingKind::Viewport { intent, commanded: navigation.is_some(), admitted },
        );
        self.poll_viewport(session)
    }

    /// Complete only exact settled native evidence, including an already accepted unchanged view.
    /// Intermediate frames still pass through ordinary presentation to release their owned slot.
    ///
    /// Every request reaches one terminal reply: `applied` at its settled camera, `stale` once a
    /// newer destination, document or source replaced it, `error` on a retained source failure,
    /// or `unavailable` once it outlives [`SETTLE_BUDGET`] without settling. Until then the
    /// reason it waits is recorded whenever it changes.
    pub(super) fn poll_viewport(&mut self, session: &Session) -> Result<(), Box<dyn Error>> {
        let Some(Pending { kind: PendingKind::Viewport { intent, admitted, .. }, .. }) =
            &self.pending
        else {
            return Ok(());
        };
        let (intent, admitted) = (*intent, *admitted);
        let state = match session.settled_camera(intent) {
            CameraSettlement::Pending => {
                let wait = self.trace_viewport_wait(session);
                if session.preview_now() < admitted.checked_add(SETTLE_BUDGET)? {
                    return Ok(());
                }
                let pending = self.pending.take().ok_or("Expired viewport request disappeared")?;
                tracing::warn!(
                    operation = "canvas_viewport_settle_expired",
                    request_id = pending.request.request_id(),
                    wait = %wait,
                    "Viewport request outlived its settle budget before its camera settled"
                );
                super::lifecycle::expired(&pending.request, wait);
                return self.reply(
                    pending.identity,
                    &pending.request,
                    CanvasSurfaceReplyStatus::Unavailable,
                    pending.claim,
                );
            }
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
                super::lifecycle::replied(&pending.request, CanvasSurfaceReplyStatus::Applied);
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

    /// When the pending viewport request's settle budget ends, so the event loop wakes to
    /// answer it even when no canvas work remains.
    pub(in crate::startup::canvas) fn viewport_deadline(&self, now: Moment) -> Option<Instant> {
        let Some(Pending { kind: PendingKind::Viewport { admitted, .. }, .. }) = &self.pending
        else {
            return None;
        };
        let due = admitted.checked_add(SETTLE_BUDGET).ok()?;
        Instant::now()
            .checked_add(Duration::from_nanos(due.as_nanos().saturating_sub(now.as_nanos())))
    }
}

/// Report the image point at the accepted viewport's center, normalized to the image extent,
/// so the web inspection owner can record this camera and restore it later.
/// `viewport` is the native-accepted camera and image the presented image's extent.
pub(in crate::startup::canvas::surface) fn with_center(
    state: CanvasViewportState,
    viewport: Viewport,
    image: Extent,
) -> Result<CanvasViewportState, Box<dyn Error>> {
    let surface = viewport.surface();
    let point = viewport
        .document_at([f64::from(surface.width()) * 0.5, f64::from(surface.height()) * 0.5])?;
    let normalized = |value: f64, extent: u32| value / f64::from(extent);
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
