// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish the settled camera of a host-routed gesture once, through the presentation watch.

use super::*;
use crate::native_canvas::session::{CameraCommandResult, CameraSettlement};
use sugarsubstitute_contracts::canvas_surface::CanvasSurfaceCameraChange;

impl NativeSurface {
    /// Await settlement after the desktop routed a gesture, or its cancellation, to navigation.
    pub(in crate::startup::canvas::surface) fn await_settled_gesture(&mut self) {
        self.preview.gesture = true;
    }

    /// Publish one settled camera for the shown, committed final once motion and capture end
    /// and native acceptance matches the destination. Moves, previews, overview grids and
    /// superseded selections publish nothing, nor does an input document shown without a
    /// selection, which has no selection revision to name. The camera shares the presentation
    /// sequence and watch storage; without a current watch or storage it is dropped like any
    /// observation.
    ///
    /// # Errors
    /// Preserves viewport snapshot, sequence and notification encoding failures.
    pub(in crate::startup::canvas::surface) fn publish_settled_gesture(
        &mut self,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        if !self.preview.gesture {
            return Ok(());
        }
        let shown = self.binding.as_ref().filter(|binding| {
            binding.committed
                && binding.selected
                && !binding.overview
                && binding.presented_selection_revision == binding.selection_revision
        });
        let intent = match session.camera_intent() {
            CameraCommandResult::Accepted(intent)
                if shown.is_some() && !self.has_shown_preview() =>
            {
                intent
            }
            _ => {
                self.preview.gesture = false;
                return Ok(());
            }
        };
        match session.settled_camera(intent) {
            CameraSettlement::Pending => return Ok(()),
            CameraSettlement::Settled(_) => {}
            CameraSettlement::Superseded | CameraSettlement::Failed => {
                self.preview.gesture = false;
                return Ok(());
            }
        }
        self.preview.gesture = false;
        let (Some(binding), Some(viewport)) = (shown, Self::observed_viewport(session, false)?)
        else {
            return Ok(());
        };
        let Some(watch) =
            self.preview.watch.as_ref().filter(|watch| watch.identity == binding.identity)
        else {
            return Ok(());
        };
        let Ok(claim) = watch.memory.claim(SurfacePresentation::STORAGE_BYTES) else {
            return Ok(());
        };
        let sequence =
            self.preview.sequence.checked_add(1).ok_or("Presentation sequence exhausted")?;
        let change = CanvasSurfaceCameraChange::new(
            binding.address.clone(),
            sequence,
            binding.selection_revision,
            viewport,
        )?;
        let command = SurfacePresentation::camera(binding.identity, &change, claim)?;
        self.preview.sequence = sequence;
        self.preview.camera = Some(DesktopStartupCommand::SendSurfacePresentation(command));
        Ok(())
    }
}
