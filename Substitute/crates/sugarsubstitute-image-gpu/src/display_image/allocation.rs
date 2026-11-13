// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain unpublished bounded display targets until their native writer completes.
use super::*;
impl DisplayImage {
    /// Allocate an unpublished worker-local render target; publish only after actual GPU retirement.
    pub(crate) fn render_target(
        device: Arc<GpuDevice>,
        extent: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        let memory = device
            .memory
            .claim(
                (u64::from(row_bytes(extent.width())) * u64::from(extent.height()))
                    .next_multiple_of(65536)
                    + 4096,
            )
            .map_err(|cause| GpuError::new("gpu_projected_tile_admission", cause))?;
        let texture = device.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SugarSubstitute bounded projected display"),
            size: size(extent.width(), extent.height()),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        device.faults.check()?;
        Ok(Self(Arc::new(Image { texture, extent, device, _memory: memory })))
    }
}
