// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Headless material qualification using the exact production projection and patch pipelines.
use crate::{
    DisplayImage, DisplayPatch, DisplayProjection, GpuDevice, GpuError,
    window_shader::{Blitter, StagedDraw},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;

/// Qualification-only target with persistent production pipelines and no window or scheduler.
pub struct OffscreenDisplay {
    device: Arc<GpuDevice>,
    blitter: Blitter,
}
impl OffscreenDisplay {
    /// Retain one actual selected device and compile the production display/overlay/patch shaders.
    /// # Errors
    /// Preserves original execution admission, native and control causes.
    pub fn new(device: Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        let blitter = Blitter::new(&device, wgpu::TextureFormat::Bgra8Unorm, context)?;
        Ok(Self { device, blitter })
    }
    /// Complete a real GPU draw before returning its immutable readback-capable output image.
    /// # Errors
    /// Rejects foreign/mismatched projection and material inputs and observes native retirement.
    pub fn render(
        &mut self,
        source: DisplayProjection<'_>,
        patches: &[DisplayPatch<'_>],
        context: &JobContext,
    ) -> Result<DisplayImage, GpuError> {
        crate::adapter::check(context)?;
        let extent = source.mapping.map_or(source.image.extent(), |m| m.output());
        if !Arc::ptr_eq(&self.device, source.image.device())
            || source.mapping.is_some_and(|m| m.source() != source.image.extent())
            || source.underlay.is_some_and(|(image, m)| {
                !Arc::ptr_eq(&self.device, image.device())
                    || m.source() != image.extent()
                    || m.output() != extent
                    || source.mapping.is_none()
            })
        {
            return Err(GpuError::invalid("Offscreen material projection has foreign input"));
        }
        let staged = StagedDraw::new(
            &self.device,
            source.mapping,
            source.underlay.map(|(_, m)| m),
            source.overlay,
            extent,
            context,
        )?
        .with_patches(&self.device, patches, extent, context)?;
        let image = DisplayImage::render_target(self.device.clone(), extent, context)?;
        self.blitter.submit(
            &self.device,
            staged,
            source.image,
            source.underlay.map(|(image, _)| image),
            &image.view(),
            self.device.submission(context)?,
        )?;
        crate::adapter::check(context)?;
        Ok(image)
    }
}
