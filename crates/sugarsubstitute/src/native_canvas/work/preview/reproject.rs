// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt unpublished Fit display pixels while retaining the same canonical candidate backing.

use super::*;

impl ReadyPreview {
    /// Reproject this candidate to a later physical Fit display on an admitted CPU worker.
    /// The backing, source, renderer and original evaluation evidence remain unchanged.
    /// This performs the same single-frame CPU step as `DrawInput`; the caller retains its
    /// original bounded job deadline. Final release of replaced pixels occurs on this worker.
    ///
    /// # Errors
    /// Rejects foreign or nonmonotonic targets. Preserves geometry, original admission and
    /// execution-control failures while retaining the preceding complete drawing on failure.
    pub(in crate::native_canvas) fn reproject(
        &mut self,
        target: FrameStamp,
        physical_extent: Extent,
        backdrops: DisplayBackdrops,
        context: &JobContext,
    ) -> Result<(), WorkError> {
        check_control(context)?;
        if target.surface != self.drawing.target.surface
            || target.generation <= self.drawing.target.generation
            || self.backing.current() != Some(self.drawing.source)
        {
            return Err(WorkError::invalid("Preview reprojection identity is invalid"));
        }
        let mut view = Viewport::new(physical_extent)?;
        view.fit(self.scene.document.snapshot().extent())?;
        let projection = self
            .drawing
            .coverage
            .crop(view, self.scene.document.snapshot().extent())?
            .ok_or_else(|| WorkError::invalid("Preview reprojection lost Fit coverage"))?;
        {
            let engine = self
                .resources
                .engine
                .lock()
                .map_err(|_| WorkError::invalid("Preview preparation state failed"))?;
            if engine.source_revision != Some(self.scene.source_revision) {
                return Err(WorkError::invalid("Preview renderer source identity changed"));
            }
        }
        let buffers = projection_buffers(&self.resources, physical_extent, context)?;
        let (storage, spare) = buffers.into_parts();
        let mut input = DrawInput {
            underlay: None,
            tier: self.drawing.tier,
            resources: self.resources.clone(),
            source: DrawSource::Cpu {
                surface: self.backing.clone(),
                storage: Some(storage),
                underlay: None,
            },
            target,
            view: projection,
            coverage: self.drawing.coverage,
            document: self.drawing.document,
            source_revision: self.drawing.source_revision,
            overlay: None,
            overlay_generation: 0,
            backdrops,
        };
        let CpuStep::Complete(drawing) = input.step(context)? else {
            return Err(WorkError::invalid("Preview display projection did not complete its step"));
        };
        self.drawing = drawing;
        self.spare_buffer = spare;
        Ok(())
    }
}
