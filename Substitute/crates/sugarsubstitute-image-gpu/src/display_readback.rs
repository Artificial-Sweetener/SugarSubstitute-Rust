// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse one admitted native readback buffer without allocating after surface acquisition.

use crate::{
    GpuDevice, GpuError,
    buffer::Buffer,
    coordination::SubmissionPermit,
    display::row_bytes,
    transfer::{Mapping, Transfer},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Extent, Region, TILE_EDGE, is_premultiplied_word};

const WORKSPACE_PIXELS: usize = TILE_EDGE as usize * TILE_EDGE as usize;
const WORKSPACE_BYTES: u64 = WORKSPACE_PIXELS as u64 * size_of::<u32>() as u64;

/// Native display pixels copied into CPU custody for qualification or export.
pub struct DisplayReadback {
    region: Region,
    words: Vec<u32>,
    _memory: MemoryClaim,
}

impl DisplayReadback {
    /// Exact device-pixel coordinates represented by these row-major words.
    pub const fn region(&self) -> Region {
        self.region
    }

    /// Premultiplied sRGB words in `0xAARRGGBB` channel order, independent of host byte order.
    pub fn pixels(&self) -> &[u32] {
        &self.words
    }
}

/// Reusable storage for one complete 128-by-128 native display tile.
///
/// Construction admits both the selected-device MAP_READ buffer and CPU word inventory. Region
/// reads reuse that storage without another memory claim or vector growth. The workspace belongs
/// to exactly one selected GPU and rejects foreign images and window frames.
pub struct DisplayReadbackWorkspace {
    receiving: Arc<Buffer>,
    words: Vec<u32>,
    _memory: MemoryClaim,
}

impl DisplayReadbackWorkspace {
    pub(crate) fn new(device: &Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        Self::with_capacity(device, WORKSPACE_BYTES, WORKSPACE_PIXELS, context)
    }

    pub(crate) fn for_region(
        device: &Arc<GpuDevice>,
        extent: Extent,
        region: Region,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        validate_region(extent, region)?;
        Self::with_capacity(
            device,
            u64::from(row_bytes(region.width())) * u64::from(region.height()),
            region.pixels() as usize,
            context,
        )
    }

    fn with_capacity(
        device: &Arc<GpuDevice>,
        buffer_bytes: u64,
        pixels: usize,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        let receiving = device.allocate(
            buffer_bytes,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            context,
        )?;
        let memory = context
            .memory()
            .claim(pixels as u64 * size_of::<u32>() as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_display_readback", cause))?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(pixels)
            .map_err(|cause| GpuError::new("gpu_display_readback_allocation", cause))?;
        words.resize(pixels, 0);
        Ok(Self { receiving, words, _memory: memory })
    }

    pub(crate) fn into_readback(mut self, region: Region) -> DisplayReadback {
        self.words.truncate(region.pixels() as usize);
        DisplayReadback { region, words: self.words, _memory: self._memory }
    }

    pub(crate) fn swap_red_blue(&mut self, pixels: usize) {
        for word in &mut self.words[..pixels] {
            *word = (*word & 0xff00ff00) | ((*word & 255) << 16) | ((*word >> 16) & 255);
        }
    }

    pub(crate) fn pixels(&self, pixels: usize) -> &[u32] {
        &self.words[..pixels]
    }
}

pub(crate) fn read_texture_region_into<'a>(
    texture: &wgpu::Texture,
    device: &Arc<GpuDevice>,
    extent: Extent,
    region: Region,
    workspace: &'a mut DisplayReadbackWorkspace,
    context: &JobContext,
) -> Result<&'a [u32], GpuError> {
    let submission = device.submission(context)?;
    read_texture_region_into_with_submission(
        texture, device, extent, region, workspace, context, submission,
    )
}

pub(crate) fn read_texture_region_into_with_submission<'a>(
    texture: &wgpu::Texture,
    device: &Arc<GpuDevice>,
    extent: Extent,
    region: Region,
    workspace: &'a mut DisplayReadbackWorkspace,
    context: &JobContext,
    submission: SubmissionPermit,
) -> Result<&'a [u32], GpuError> {
    validate_workspace(device, extent, region, workspace)?;
    crate::adapter::check(context)?;
    let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SugarSubstitute retained display readback"),
    });
    commands.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: region.x(), y: region.y(), z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &workspace.receiving.raw,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes(region.width())),
                rows_per_image: Some(region.height()),
            },
        },
        wgpu::Extent3d { width: region.width(), height: region.height(), depth_or_array_layers: 1 },
    );
    Transfer::submit(device.clone(), commands, submission.clone()).retire()?;
    decode_workspace(region, workspace, context, submission)
}

pub(crate) fn read_buffer_region_into<'a>(
    source: Arc<Buffer>,
    region: Region,
    workspace: &'a mut DisplayReadbackWorkspace,
    context: &JobContext,
) -> Result<&'a [u32], GpuError> {
    let extent =
        Extent::new(region.right(), region.bottom()).map_err(crate::evaluate::image_error)?;
    validate_workspace(&source.device, extent, region, workspace)?;
    if source.bytes != u64::from(row_bytes(region.width())) * u64::from(region.height()) {
        return Err(GpuError::invalid("GPU display tile differs from its readback region"));
    }
    crate::adapter::check(context)?;
    let submission = source.device.submission(context)?;
    Transfer::copy(source, workspace.receiving.clone(), submission.clone()).retire()?;
    decode_workspace(region, workspace, context, submission)
}

fn validate_workspace(
    device: &Arc<GpuDevice>,
    extent: Extent,
    region: Region,
    workspace: &DisplayReadbackWorkspace,
) -> Result<(), GpuError> {
    validate_region(extent, region)?;
    if !Arc::ptr_eq(device, &workspace.receiving.device) {
        return Err(GpuError::invalid("GPU readback workspace belongs to another device"));
    }
    let required = u64::from(row_bytes(region.width())) * u64::from(region.height());
    if workspace.receiving.bytes < required || workspace.words.len() < region.pixels() as usize {
        return Err(GpuError::invalid("GPU readback workspace is too small for the region"));
    }
    Ok(())
}

fn validate_region(extent: Extent, region: Region) -> Result<(), GpuError> {
    if region.width() > TILE_EDGE
        || region.height() > TILE_EDGE
        || region.pixels() as usize > WORKSPACE_PIXELS
        || region.intersection(extent.region()) != Some(region)
    {
        return Err(GpuError::invalid("Invalid retained GPU readback region"));
    }
    Ok(())
}

fn decode_workspace<'a>(
    region: Region,
    workspace: &'a mut DisplayReadbackWorkspace,
    context: &JobContext,
    submission: SubmissionPermit,
) -> Result<&'a [u32], GpuError> {
    let mut mapping = Mapping::new(workspace.receiving.clone(), submission);
    mapping.wait()?;
    {
        let view = mapping.view()?;
        let stride = row_bytes(region.width()) as usize;
        let mut output = workspace.words[..region.pixels() as usize].iter_mut();
        for row in view.chunks_exact(stride).take(region.height() as usize) {
            for bytes in row[..region.width() as usize * 4].chunks_exact(4) {
                let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if !is_premultiplied_word(value) {
                    return Err(GpuError::invalid("GPU display result is not premultiplied"));
                }
                let word = output.next().ok_or_else(|| {
                    GpuError::invalid("GPU display readback exceeded its admitted word inventory")
                })?;
                *word = value;
            }
        }
        if output.len() != 0 {
            return Err(GpuError::invalid(
                "GPU display readback did not fill its requested region",
            ));
        }
    }
    mapping.finish()?;
    crate::adapter::check(context)?;
    Ok(workspace.pixels(region.pixels() as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_region_validation_accepts_one_complete_shared_tile() -> Result<(), GpuError> {
        let extent = Extent::new(256, 256).map_err(crate::evaluate::image_error)?;
        let complete = Region::new(128, 128, 128, 128).map_err(crate::evaluate::image_error)?;
        assert!(validate_region(extent, complete).is_ok());
        assert_eq!(complete.pixels() as usize, WORKSPACE_PIXELS);
        assert_eq!(u64::from(row_bytes(TILE_EDGE)) * u64::from(TILE_EDGE), WORKSPACE_BYTES);
        assert!(
            validate_region(
                extent,
                Region::new(0, 0, 129, 128).map_err(crate::evaluate::image_error)?,
            )
            .is_err()
        );
        assert!(
            validate_region(
                extent,
                Region::new(192, 192, 65, 64).map_err(crate::evaluate::image_error)?,
            )
            .is_err()
        );
        Ok(())
    }
}
