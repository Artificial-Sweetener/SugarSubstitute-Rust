// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify semantic camera intent with current document/source and actual native display evidence.

use super::*;
use sugarsubstitute_canvas::{NavigationCommand, NavigationCommandOutcome, ZoomMode};
use sugarsubstitute_image_ops::Extent;

/// Current native-accepted CPU camera, independent of a later interpolation destination.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AcceptedCamera {
    /// Native canvas document identity, independent of source intent revision.
    pub(crate) document: u64,
    /// Authored document revision represented by these accepted pixels.
    pub(crate) revision: u64,
    /// Existing session source intent represented by these accepted pixels.
    pub(crate) source_revision: u64,
    /// Authored image dimensions in document pixels.
    pub(crate) extent: Extent,
    /// Accepted physical viewport, document-space origin and physical pixels per document pixel.
    pub(crate) viewport: Viewport,
    /// Exact frame accepted by the native CPU presentation owner.
    pub(crate) frame: FrameStamp,
}

/// A destination qualified by existing document and source authority; owns no worker or pixels.
/// The application owns its waiter and matches request identity before cancelling this intent.
/// Identical destinations can reuse already accepted current evidence without new frame allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CameraIntent {
    document: work::DocumentKey,
    source_revision: u64,
    target: Viewport,
    mode: ZoomMode,
}

impl CameraIntent {
    /// Destination mode, suitable for display state only after this exact intent settles.
    pub(crate) const fn mode(self) -> ZoomMode {
        self.mode
    }
}

/// Explicit semantic command admission, before the application starts waiting for presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CameraCommandResult {
    /// The existing navigation owner admitted this qualified destination.
    Accepted(CameraIntent),
    /// Policy, lifecycle, native mode or a live tool gesture currently owns the camera.
    Denied,
    /// The session has no selected current image/document to navigate.
    NoContent,
}

/// A waiter never equates animation intent or prepared pixels with native acceptance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CameraSettlement {
    /// Current navigation or exact native acceptance has not reached this destination.
    Pending,
    /// Final navigation and native acceptance agree on this exact qualified camera.
    Settled(AcceptedCamera),
    /// Document, source, physical geometry or a newer destination replaced this intent.
    Superseded,
    /// The still-current owner retained a preparation/device failure; its diagnostic stays owned.
    Failed,
}

impl Session {
    /// Admit semantic navigation on the event-loop thread through the existing tool owner.
    /// This never synthesizes device input or releases pending pixels/workers. Cancellation of
    /// an application waiter does not undo camera motion that has already been sampled or accepted.
    ///
    /// # Errors
    /// Preserves navigation validation/time failures and obsolete-refinement cancellation errors.
    pub(crate) fn navigate(
        &mut self,
        command: NavigationCommand,
    ) -> Result<CameraCommandResult, Box<dyn Error>> {
        let document = match self.camera_intent() {
            CameraCommandResult::Accepted(current) => current.document,
            unavailable => return Ok(unavailable),
        };
        let now = self.navigation_time();
        match self.document.tools.navigate(command, now)? {
            NavigationCommandOutcome::Applied { target } => {
                self.input_observed = now;
                self.cancel_obsolete_refinement()?;
                Ok(CameraCommandResult::Accepted(CameraIntent {
                    document,
                    source_revision: self.selection,
                    target,
                    mode: self.document.tools.navigation().zoom_mode(),
                }))
            }
            NavigationCommandOutcome::Denied => Ok(CameraCommandResult::Denied),
            NavigationCommandOutcome::NoContent => Ok(CameraCommandResult::NoContent),
        }
    }

    /// Present the latest offered source's first frame at `frame` instead of Fit, or at Fit.
    /// The camera applies when that exact source intent loads or its retained document returns;
    /// a newer offer replaces it.
    pub(crate) fn frame_selection(&mut self, frame: Option<NavigationCommand>) {
        self.initial_frame = frame.map(|frame| (self.selection, frame));
    }

    /// Qualify the current navigation destination for a read without moving or cancelling it.
    /// The application waits for this intent's native evidence just as for a camera command.
    pub(crate) fn camera_intent(&self) -> CameraCommandResult {
        if self.closing
            || self.presentation.gpu_window().is_some()
            || self.source_failed()
            || self.document_suspended
            || self.restoring_document()
        {
            return CameraCommandResult::Denied;
        }
        let selected = self.draws_current_selection();
        let Some(document) = self.document.shown_key().filter(|_| selected) else {
            return CameraCommandResult::NoContent;
        };
        CameraCommandResult::Accepted(CameraIntent {
            document,
            source_revision: self.selection,
            target: self.document.tools.navigation().target_viewport(),
            mode: self.document.tools.navigation().zoom_mode(),
        })
    }

    /// Withdraw only this current semantic destination, sampling motion without rolling it back.
    /// The application first matches the original request; a newer destination or live physical
    /// gesture remains authoritative. Pending pixels and jobs retain their normal native custody.
    ///
    /// # Errors
    /// Preserves injected-clock, camera geometry and refinement-cancellation failures.
    pub(crate) fn cancel_camera(&mut self, intent: CameraIntent) -> Result<bool, Box<dyn Error>> {
        if self.camera_intent() != CameraCommandResult::Accepted(intent) {
            return Ok(false);
        }
        let now = self.navigation_time();
        if !self.document.tools.cancel_navigation(intent.target, intent.mode, now)? {
            return Ok(false);
        }
        self.input_observed = now;
        self.cancel_obsolete_refinement()?;
        Ok(true)
    }

    /// Read actual native-accepted evidence for the current CPU image, even during later motion.
    /// Prepared CPU frames and semantic destination modes do not become display state here.
    pub(crate) fn accepted_camera(&self) -> Option<AcceptedCamera> {
        if self.closing || self.presentation.gpu_window().is_some() || self.source_failed() {
            return None;
        }
        if !self.draws_current_selection() {
            return None;
        }
        let evidence = self.document.display_evidence?;
        let (key, snapshot) = self.document.shown()?;
        if self.document.native_presented != Some(evidence.target)
            || evidence.document != key
            || evidence.source_revision != self.selection
        {
            return None;
        }
        Some(AcceptedCamera {
            document: evidence.document.0,
            revision: evidence.document.1,
            source_revision: evidence.source_revision,
            extent: snapshot.extent(),
            viewport: evidence.view,
            frame: evidence.target,
        })
    }

    /// Resolve a qualified destination only after motion ends and its exact camera is accepted.
    /// An unchanged command can settle against the existing accepted frame without allocation.
    pub(crate) fn settled_camera(&self, intent: CameraIntent) -> CameraSettlement {
        let current = self.draws_current_selection()
            && intent.source_revision == self.selection
            && self.document.shown_key() == Some(intent.document);
        let navigation = self.document.tools.navigation();
        if self.closing
            || self.document_suspended
            || self.restoring_document()
            || !current
            || navigation.target_viewport() != intent.target
            || navigation.zoom_mode() != intent.mode
        {
            return CameraSettlement::Superseded;
        }
        if self.source_failed() {
            return CameraSettlement::Failed;
        }
        if navigation.interacting() || navigation.viewport() != intent.target {
            return CameraSettlement::Pending;
        }
        match self.accepted_camera() {
            Some(camera) if camera.viewport == intent.target => CameraSettlement::Settled(camera),
            _ => CameraSettlement::Pending,
        }
    }
}
