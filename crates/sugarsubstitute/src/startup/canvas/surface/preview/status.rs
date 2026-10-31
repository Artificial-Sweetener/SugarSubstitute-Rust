// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish monotonic full-state observations only after native acceptance or observed failure.
use super::*;
use crate::native_canvas::session::CameraCommandResult;
use sugarsubstitute_canvas::ZoomMode;

impl NativeSurface {
    /// Snapshot the actual last native image; pending decode never changes this evidence.
    fn displayed_preview_state(&self) -> CanvasSurfacePresentationDisplay {
        if let Some(shown) = &self.preview.shown {
            return shown.display();
        }
        self.binding
            .as_ref()
            .filter(|binding| binding.presented_viewport.is_some())
            .or(self.retained.as_ref())
            .and_then(|binding| {
                if binding.presented_overview {
                    return Some(CanvasSurfacePresentationDisplay::Overview {
                        selection_revision: binding.presented_selection_revision,
                    });
                }
                Some(CanvasSurfacePresentationDisplay::Final {
                    selection_revision: binding.presented_selection_revision,
                    viewport: binding.presented_viewport?,
                })
            })
            .unwrap_or(CanvasSurfacePresentationDisplay::Empty)
    }
    /// Coalesce complete observations while preserving the authoritative sequence monotonicity.
    pub(super) fn preview_change(
        &mut self,
        lease: &Lease,
        status: CanvasSurfacePresentationStatus,
    ) -> Result<(), Box<dyn Error>> {
        if !self.preview.watch.as_ref().is_some_and(|watch| watch.identity == lease.identity) {
            return Ok(());
        }
        self.preview_observation(lease, status, self.displayed_preview_state())
    }
    /// Encode a complete explicitly observed display even while moving its lease between owners.
    fn preview_observation(
        &mut self,
        lease: &Lease,
        status: CanvasSurfacePresentationStatus,
        displayed: CanvasSurfacePresentationDisplay,
    ) -> Result<(), Box<dyn Error>> {
        if !self.preview.watch.as_ref().is_some_and(|watch| watch.identity == lease.identity) {
            return Ok(());
        }
        let sequence =
            self.preview.sequence.checked_add(1).ok_or("Presentation sequence exhausted")?;
        let change = CanvasSurfacePresentationChange::new(
            lease.address.clone(),
            sequence,
            lease.token.clone(),
            lease.revision,
            status,
            displayed,
            String::new(),
        )?;
        self.preview.sequence = sequence;
        self.preview.pending_change = Some((lease.identity, change));
        self.queue_preview_notification()
    }
    /// A callback snapshot has independent admission and cannot replenish an ordinary ticket.
    pub(super) fn queue_preview_notification(&mut self) -> Result<(), Box<dyn Error>> {
        let Some((identity, change)) = &self.preview.pending_change else { return Ok(()) };
        let Some(watch) = &self.preview.watch else {
            self.preview.pending_change = None;
            return Ok(());
        };
        if watch.identity != *identity {
            self.preview.pending_change = None;
            return Ok(());
        }
        self.preview.notification = None;
        let Ok(claim) = watch.memory.claim(SurfacePresentation::STORAGE_BYTES) else {
            return Ok(());
        };
        let command = SurfacePresentation::new(*identity, change, claim)?;
        self.preview.pending_change = None;
        self.preview.notification = Some(DesktopStartupCommand::SendSurfacePresentation(command));
        Ok(())
    }
    /// Translate the current native-accepted camera, never an interpolating destination.
    pub(in crate::startup::canvas::surface) fn observed_viewport(
        session: &Session,
        preview: bool,
    ) -> Result<Option<CanvasViewportState>, Box<dyn Error>> {
        let Some(camera) = session.accepted_camera() else { return Ok(None) };
        let mode = match session.camera_intent() {
            CameraCommandResult::Accepted(intent) => match intent.mode() {
                ZoomMode::Fit => CanvasViewportMode::Fit,
                ZoomMode::OneToOne => CanvasViewportMode::Native,
                ZoomMode::Custom => CanvasViewportMode::Custom,
            },
            _ => CanvasViewportMode::Custom,
        };
        if preview && mode != CanvasViewportMode::Fit {
            return Err("Native-accepted preview camera is not in Fit mode".into());
        }
        let state = super::super::viewport::with_center(
            CanvasViewportState::new(
                mode,
                camera.viewport.zoom(),
                camera.extent.width(),
                camera.extent.height(),
            )?,
            camera.viewport,
            camera.extent,
        )?;
        if preview {
            return Ok(Some(state));
        }
        Ok(Some(super::super::viewport::with_comparison(state, session)?))
    }
    /// Update preview/final identity only after the session accepts this exact native target.
    pub(in crate::startup::canvas::surface) fn preview_presented(
        &mut self,
        target: FrameStamp,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        let accepted = self.preview.current.as_ref().is_some_and(|lease| {
            lease.native.is_some_and(|token| {
                session.preview_status(token) == PreviewStatus::Presented(target)
            })
        });
        if accepted {
            let lease = self.preview.current.take().ok_or("Presented preview lease disappeared")?;
            let viewport = Self::observed_viewport(session, true)?
                .ok_or("Accepted preview viewport is unavailable")?;
            let shown = Shown { lease, viewport };
            self.preview_observation(
                &shown.lease,
                CanvasSurfacePresentationStatus::Presented,
                shown.display(),
            )?;
            self.preview.shown = Some(shown);
        } else if let Some(mut shown) = self.preview.shown.take() {
            if session.preview_active() {
                if let Some(viewport) = Self::observed_viewport(session, true)? {
                    shown.viewport = viewport;
                }
                self.preview_observation(
                    &shown.lease,
                    CanvasSurfacePresentationStatus::Presented,
                    shown.display(),
                )?;
                self.preview.shown = Some(shown);
            } else {
                self.preview_change(&shown.lease, CanvasSurfacePresentationStatus::Cancelled)?;
            }
        }
        if self.preview.withdrawing && !session.preview_active() {
            if let Some(lease) = self.preview.current.take() {
                self.preview_change(&lease, CanvasSurfacePresentationStatus::Cancelled)?;
            }
            self.preview.withdrawing = false;
        }
        self.queue_preview_notification()
    }
    /// Clear shown preview evidence only after the native adapter acknowledges hidden content
    /// for a lease the session no longer presents. A geometry change hides the old placement
    /// while the session refits the same accepted preview; that lease stays shown so its refit
    /// frame republishes it instead of reporting a withdrawal the page never requested.
    pub(in crate::startup::canvas::surface) fn preview_hidden(
        &mut self,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        let refitting = self.preview.shown.as_ref().is_some_and(|shown| {
            shown.lease.native.is_some_and(|token| {
                matches!(session.preview_status(token), PreviewStatus::Presented(_))
            })
        });
        if !refitting && let Some(shown) = self.preview.shown.take() {
            self.preview_observation(
                &shown.lease,
                CanvasSurfacePresentationStatus::Cancelled,
                CanvasSurfacePresentationDisplay::Empty,
            )?;
        }
        if self.preview.withdrawing {
            if let Some(lease) = self.preview.current.take() {
                self.preview_observation(
                    &lease,
                    CanvasSurfacePresentationStatus::Cancelled,
                    CanvasSurfacePresentationDisplay::Empty,
                )?;
            }
            self.preview.withdrawing = false;
        }
        Ok(())
    }
    /// Distinguish preview acceptance from final-image evidence without using selection intent.
    pub(in crate::startup::canvas::surface) fn is_preview_frame(
        &self,
        target: FrameStamp,
        session: &Session,
    ) -> bool {
        self.preview.current.as_ref().is_some_and(|lease| {
            lease.native.is_some_and(|token| {
                session.preview_status(token) == PreviewStatus::Presented(target)
            })
        }) || (self.preview.shown.is_some() && session.preview_active())
    }
    /// Native preview evidence permits viewport reads while session tools enforce the actual lock.
    pub(in crate::startup::canvas::surface) fn has_shown_preview(&self) -> bool {
        self.preview.shown.is_some()
    }
    /// Unpublished preview intent can produce the initial frame of an otherwise empty binding.
    pub(in crate::startup::canvas) fn has_preview_candidate(&self) -> bool {
        self.preview.current.is_some()
    }
    /// Hide only after the application has actually restored the parked empty document.
    pub(in crate::startup::canvas::surface) fn preview_restore_requires_hide(
        &self,
        session: &Session,
    ) -> bool {
        (self.preview.withdrawing || self.preview.shown.is_some())
            && session.preview_restored_empty()
    }
}
