// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw retained display images into imported composition textures on admitted workers.
use crate::{
    DisplayImage, DisplayMapping, GpuDevice, GpuError, OverlayPixel, SharedTarget,
    window_shader::{Blitter, StagedDraw},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Worker-owned display shader for composition textures of one selected device.
///
/// Each draw uses the same production shader and mappings as native window presentation and
/// observes actual GPU completion before returning, so the composition owner may read the target
/// as soon as it receives the completed frame. Preparation is serialized by `&mut self`.
pub struct SharedSurface {
    device: Arc<GpuDevice>,
    blitter: Option<Blitter>,
    _memory: MemoryClaim,
}

impl SharedSurface {
    /// Retain the selected device; shader compilation is deferred to the first draw.
    /// # Errors
    /// Preserves device admission failures.
    pub fn new(device: Arc<GpuDevice>) -> Result<Self, GpuError> {
        let memory = device
            .memory
            .claim(64 * 1024)
            .map_err(|cause| GpuError::new("gpu_shared_surface_metadata", cause))?;
        Ok(Self { device, blitter: None, _memory: memory })
    }
    /// Copy a retained image of exactly the target's extent.
    /// `overlay` pixels, such as selection outlines, are drawn opaque over the display.
    /// # Errors
    /// Rejects foreign devices and differing extents; preserves control, admission and GPU causes.
    pub fn draw(
        &mut self,
        target: &SharedTarget,
        image: &DisplayImage,
        overlay: &[OverlayPixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        if image.extent() != target.extent {
            return Err(GpuError::invalid("Composition target differs from retained image"));
        }
        self.render(target, image, None, None, overlay, context)
    }
    /// Project a retained image into the target through one validated display mapping.
    /// `overlay` pixels, such as selection outlines, are drawn opaque over the display.
    /// # Errors
    /// Rejects foreign devices and mismatched mappings; preserves original native causes.
    pub fn draw_projected(
        &mut self,
        target: &SharedTarget,
        image: &DisplayImage,
        mapping: DisplayMapping<'_>,
        overlay: &[OverlayPixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        if mapping.source() != image.extent() || mapping.output() != target.extent {
            return Err(GpuError::invalid("Composition projection differs from its images"));
        }
        self.render(target, image, Some(mapping), None, overlay, context)
    }
    /// Project a retained detail image over a retained fallback image and its mapping, as native
    /// window composition does. `overlay` pixels, such as selection outlines, are drawn opaque over the display.
    /// # Errors
    /// Rejects foreign devices and mismatched mappings; preserves original native causes.
    pub fn draw_composited(
        &mut self,
        target: &SharedTarget,
        image: &DisplayImage,
        mapping: DisplayMapping<'_>,
        (underlay, underlay_mapping): (&DisplayImage, DisplayMapping<'_>),
        overlay: &[OverlayPixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        if mapping.source() != image.extent()
            || underlay_mapping.source() != underlay.extent()
            || mapping.output() != target.extent
            || underlay_mapping.output() != target.extent
        {
            return Err(GpuError::invalid("Composition projection differs from its images"));
        }
        self.render(
            target,
            image,
            Some(mapping),
            Some((underlay, underlay_mapping)),
            overlay,
            context,
        )
    }
    fn render(
        &mut self,
        target: &SharedTarget,
        image: &DisplayImage,
        mapping: Option<DisplayMapping<'_>>,
        underlay: Option<(&DisplayImage, DisplayMapping<'_>)>,
        overlay: &[OverlayPixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        crate::adapter::check(context)?;
        let foreign = |other: &Arc<GpuDevice>| !Arc::ptr_eq(other, &self.device);
        if foreign(&target.device)
            || foreign(image.device())
            || underlay.is_some_and(|(underlay, _)| foreign(underlay.device()))
        {
            return Err(GpuError::invalid("Composition target and images use different GPUs"));
        }
        self.device.faults.check()?;
        if self.blitter.is_none() {
            self.blitter =
                Some(Blitter::new(&self.device, wgpu::TextureFormat::Bgra8Unorm, context)?);
        }
        let staged = StagedDraw::new(
            mapping,
            underlay.map(|(_, mapping)| mapping),
            overlay,
            target.extent,
            context,
        )?;
        let output = target.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let submission = self.device.submission(context)?;
        let blitter = self
            .blitter
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Composition display shader is unavailable"))?;
        blitter.submit(
            &self.device,
            staged,
            image,
            underlay.map(|(image, _)| image),
            &output,
            submission,
        )?;
        crate::adapter::check(context)
    }
}
