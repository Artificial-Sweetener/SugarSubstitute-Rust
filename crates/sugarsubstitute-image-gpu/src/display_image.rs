// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose immutable retained display images with native copies and original execution custody.
use crate::{
    DisplayReadback, DisplayTile, GpuDevice, GpuError, buffer::Buffer, display::row_bytes,
    transfer::Transfer,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Extent, Region, TILE_EDGE};

/// Exact native pixel copy between two immutable display generations.
#[derive(Clone, Copy, Debug)]
pub struct DisplayCopy {
    /// Global source rectangle in the preceding device image.
    pub source: Region,
    /// Destination top-left physical pixel coordinates in the replacement image.
    pub destination: [u32; 2],
}
/// Completed opaque sRGB8 GPU image, independent of native windows and document identities.
/// Clones retain the actual selected device and its original allocation admission. No write API
/// can modify a published image; replacement construction finishes before returning a new value.
#[derive(Clone)]
pub struct DisplayImage(Arc<Image>);
struct Image {
    texture: wgpu::Texture,
    extent: Extent,
    device: Arc<GpuDevice>,
    _memory: MemoryClaim,
}
impl DisplayImage {
    /// Exact physical dimensions of this completed immutable image.
    pub fn extent(&self) -> Extent {
        self.0.extent
    }
    /// Compose an opaque replacement on an admitted worker using one bounded native submission.
    /// A supplied previous image is copied in full, or by the explicitly translated rectangle.
    /// Other pixels begin opaque black; prepared tiles overwrite their exact global rectangles.
    /// Every input and original allocation stays alive through actual native completion.
    /// # Errors
    /// Rejects foreign devices, out-of-bounds copies, excessive inventories and dimensions above
    /// 16384; preserves original admission/control/driver causes. Cancellation prevents publication.
    pub fn compose(
        device: Arc<GpuDevice>,
        extent: Extent,
        previous: Option<&Self>,
        translation: Option<DisplayCopy>,
        tiles: &[DisplayTile],
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        if extent.width() > 16384 || extent.height() > 16384 || tiles.len() > 4096 {
            return Err(GpuError::invalid("Retained GPU image exceeds its bounds"));
        }
        let copy = copy_region(extent, previous.map(Self::extent), translation)?;
        if previous.is_some_and(|image| !Arc::ptr_eq(&image.0.device, &device))
            || tiles.iter().any(|tile| {
                !Arc::ptr_eq(&tile.buffer.device, &device)
                    || tile.region.intersection(extent.region()) != Some(tile.region)
            })
        {
            return Err(GpuError::invalid(
                "Retained GPU image has foreign or out-of-bounds inputs",
            ));
        }
        let _commands = context
            .memory()
            .claim(tiles.len() as u64 * 512 + 4096)
            .map_err(|cause| GpuError::new("gpu_display_copy_commands", cause))?;
        let memory = device
            .memory
            .claim(
                (u64::from(row_bytes(extent.width())) * u64::from(extent.height()))
                    .next_multiple_of(65536)
                    + 4096,
            )
            .map_err(|cause| GpuError::new("gpu_display_image_admission", cause))?;
        device.faults.check()?;
        let texture = device.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SugarSubstitute immutable display image"),
            size: size(extent.width(), extent.height()),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute retained display copy"),
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let _clear = commands.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SugarSubstitute opaque display initialization"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        if let (Some(previous), Some(copy)) = (previous, copy) {
            commands.copy_texture_to_texture(
                location(&previous.0.texture, [copy.source.x(), copy.source.y()]),
                location(&texture, copy.destination),
                size(copy.source.width(), copy.source.height()),
            );
        }
        for tile in tiles {
            commands.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &tile.buffer.raw,
                    layout: layout(tile.region.width(), tile.region.height()),
                },
                location(&texture, [tile.region.x(), tile.region.y()]),
                size(tile.region.width(), tile.region.height()),
            );
        }
        device.faults.check()?;
        Transfer::submit(device.clone(), commands).retire()?;
        crate::adapter::check(context)?;
        Ok(Self(Arc::new(Image { texture, extent, device, _memory: memory })))
    }
    /// Read one bounded physical region for qualification or export, excluding row padding.
    /// Native presentation consumes the device image directly.
    /// # Errors
    /// Rejects regions beyond one 128-pixel tile or outside this image; preserves original causes.
    pub fn read_region(
        &self,
        region: Region,
        context: &JobContext,
    ) -> Result<DisplayReadback, GpuError> {
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(self.extent().region()) != Some(region)
        {
            return Err(GpuError::invalid("Invalid retained GPU readback region"));
        }
        let device = &self.0.device;
        let receiving: Arc<Buffer> = device.allocate(
            u64::from(row_bytes(region.width())) * u64::from(region.height()),
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            context,
        )?;
        let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute retained display readback"),
        });
        commands.copy_texture_to_buffer(
            location(&self.0.texture, [region.x(), region.y()]),
            wgpu::TexelCopyBufferInfo {
                buffer: &receiving.raw,
                layout: layout(region.width(), region.height()),
            },
            size(region.width(), region.height()),
        );
        Transfer::submit(device.clone(), commands).retire()?;
        crate::adapter::check(context)?;
        DisplayReadback::receive(receiving, region, context)
    }
}
fn size(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d { width, height, depth_or_array_layers: 1 }
}
fn layout(width: u32, height: u32) -> wgpu::TexelCopyBufferLayout {
    wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(row_bytes(width)),
        rows_per_image: Some(height),
    }
}
fn location(texture: &wgpu::Texture, origin: [u32; 2]) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x: origin[0], y: origin[1], z: 0 },
        aspect: wgpu::TextureAspect::All,
    }
}
fn copy_region(
    extent: Extent,
    previous: Option<Extent>,
    translation: Option<DisplayCopy>,
) -> Result<Option<DisplayCopy>, GpuError> {
    let Some(previous) = previous else {
        return if translation.is_some() {
            Err(GpuError::invalid("GPU translation requires a preceding image"))
        } else {
            Ok(None)
        };
    };
    let copy =
        translation.unwrap_or(DisplayCopy { source: previous.region(), destination: [0, 0] });
    let destination = Region::new(
        copy.destination[0],
        copy.destination[1],
        copy.source.width(),
        copy.source.height(),
    )
    .map_err(crate::evaluate::image_error)?;
    if previous.region().intersection(copy.source) != Some(copy.source)
        || extent.region().intersection(destination) != Some(destination)
    {
        return Err(GpuError::invalid("GPU translation exceeds a display image"));
    }
    Ok(Some(copy))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_translation_validates_both_generations_before_native_effects() -> Result<(), GpuError>
    {
        let extent = Extent::new(513, 385).map_err(crate::evaluate::image_error)?;
        let copy = DisplayCopy {
            source: Region::new(0, 11, 506, 374).map_err(crate::evaluate::image_error)?,
            destination: [7, 0],
        };
        assert!(copy_region(extent, Some(extent), Some(copy))?.is_some());
        assert!(copy_region(extent, None, Some(copy)).is_err());
        assert!(
            copy_region(extent, Some(extent), Some(DisplayCopy { destination: [8, 0], ..copy }))
                .is_err()
        );
        assert!(
            copy_region(
                extent,
                Some(Extent::new(505, 385).map_err(crate::evaluate::image_error)?),
                Some(copy)
            )
            .is_err()
        );
        assert!(copy_region(extent, None, None)?.is_none());
        assert_eq!(crate::display::row_bytes(1), 256);
        assert_eq!(crate::display::row_bytes(64), 256);
        assert_eq!(crate::display::row_bytes(65), 512);
        Ok(())
    }
}
