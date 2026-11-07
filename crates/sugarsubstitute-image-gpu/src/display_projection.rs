// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture complete local projected raster crops on the selected GPU without pixel readback.
use crate::{
    DisplayImage, DisplayMapping, GpuDevice, GpuError, GpuRasterBinding, OverlayPixel,
    display_decode::DisplayDecoder,
    window_shader::{Blitter, StagedDraw},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{AssetId, DisplaySample, Extent, Region};

/// Borrow the exact clean display inputs of one immutable presentation snapshot.
/// Overlay coordinates and every mapping use physical viewport pixels.
#[derive(Clone, Copy)]
pub struct DisplayProjection<'a> {
    /// Retained detail image.
    pub image: &'a DisplayImage,
    /// Detail-to-viewport map, or exact identity projection.
    pub mapping: Option<DisplayMapping<'a>>,
    /// Retained fallback and its viewport map.
    pub underlay: Option<(&'a DisplayImage, DisplayMapping<'a>)>,
    /// Selection or other clean-canvas overlay pixels, before backdrop materials.
    pub overlay: &'a [OverlayPixel],
}
/// Persistent projection/decoding pipelines, confined to admitted preparation workers.
pub struct DisplayProjector {
    device: Arc<GpuDevice>,
    blitter: Blitter,
    decoder: DisplayDecoder,
}
impl DisplayProjector {
    /// Compile the production projection and shared canonical conversion on one selected device.
    /// # Errors
    /// Preserves original admission, cancellation and native compilation causes.
    pub fn new(device: Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        let blitter = Blitter::new(&device, wgpu::TextureFormat::Bgra8Unorm, context)?;
        let decoder = DisplayDecoder::new(&device, context)?;
        Ok(Self { device, blitter, decoder })
    }
    /// Project only `crop`, preserving detail/fallback/matte/overlay semantics and local coordinates.
    /// All BGRA scratch textures and canonical source tiles stay GPU resident. Each native draw
    /// and conversion retires before returning the complete immutable binding.
    /// `retained` admits its inventory metadata independently of this preparation job.
    /// # Errors
    /// Rejects foreign devices, inconsistent maps, out-of-viewport or excessive crops.
    pub fn project_raster(
        &mut self,
        asset: AssetId,
        crop: Region,
        source: DisplayProjection<'_>,
        retained: &MemoryLease,
        context: &JobContext,
    ) -> Result<GpuRasterBinding, GpuError> {
        let output = source.mapping.map_or(source.image.extent(), DisplayMapping::output);
        validate(&self.device, crop, source, output)?;
        let extent =
            Extent::new(crop.width(), crop.height()).map_err(crate::evaluate::image_error)?;
        let count = extent.region().tile_count() as usize;
        if count > 4096 {
            return Err(GpuError::invalid("Projected source crop exceeds its tile bound"));
        }
        let _inventory = context
            .memory()
            .claim(count as u64 * size_of::<crate::GpuTile>() as u64 + 256)
            .map_err(|cause| GpuError::new("gpu_projected_source_inventory", cause))?;
        let mut tiles = Vec::new();
        tiles
            .try_reserve_exact(count)
            .map_err(|cause| GpuError::new("gpu_projected_source_allocation", cause))?;
        for tile in extent.region().tiles() {
            crate::adapter::check(context)?;
            let viewport =
                Region::new(crop.x() + tile.x(), crop.y() + tile.y(), tile.width(), tile.height())
                    .map_err(crate::evaluate::image_error)?;
            let image = self.project_image(viewport, source, context)?;
            tiles.push(self.decoder.decode(&image, tile, context)?);
        }
        GpuRasterBinding::from_tiles(asset, extent, tiles, retained, context)
    }
    /// Produce one bounded encoded clean projection image, independent of canonical image tiles.
    /// This includes the same canvas overlays and never samples web controls or another patch.
    /// # Errors
    /// Rejects foreign/outside projection inputs and preserves native/control/allocation causes.
    pub fn project_image(
        &mut self,
        crop: Region,
        source: DisplayProjection<'_>,
        context: &JobContext,
    ) -> Result<DisplayImage, GpuError> {
        let output = source.mapping.map_or(source.image.extent(), DisplayMapping::output);
        validate(&self.device, crop, source, output)?;
        let extent =
            Extent::new(crop.width(), crop.height()).map_err(crate::evaluate::image_error)?;
        let _axes = context
            .memory()
            .claim(
                (u64::from(crop.width()) + u64::from(crop.height()))
                    * size_of::<DisplaySample>() as u64
                    + 256,
            )
            .map_err(|cause| GpuError::new("gpu_projected_identity_maps", cause))?;
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mapping = match source.mapping {
            Some(mapping) => mapping.cropped(crop)?,
            None => {
                columns
                    .try_reserve_exact(crop.width() as usize)
                    .map_err(|cause| GpuError::new("gpu_projection_columns", cause))?;
                rows.try_reserve_exact(crop.height() as usize)
                    .map_err(|cause| GpuError::new("gpu_projection_rows", cause))?;
                columns.extend((0..crop.width()).map(|x| {
                    DisplaySample::from_corner(f64::from(crop.x() + x) + 0.5, output.width(), false)
                }));
                rows.extend((0..crop.height()).map(|y| {
                    DisplaySample::from_corner(
                        f64::from(crop.y() + y) + 0.5,
                        output.height(),
                        false,
                    )
                }));
                DisplayMapping::new(output, extent, &columns, &rows, [0, 0, 0, 0])?
            }
        };
        let underlay = source.underlay.map(|(_, map)| map.cropped(crop)).transpose()?;
        let count = source.overlay.iter().filter(|p| crop.contains(p.x, p.y)).count();
        let _overlay = context
            .memory()
            .claim(count as u64 * size_of::<OverlayPixel>() as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_projected_overlay", cause))?;
        let mut overlay = Vec::new();
        overlay
            .try_reserve_exact(count)
            .map_err(|cause| GpuError::new("gpu_projected_overlay_allocation", cause))?;
        overlay.extend(
            source.overlay.iter().filter(|p| crop.contains(p.x, p.y)).map(|p| OverlayPixel {
                x: p.x - crop.x(),
                y: p.y - crop.y(),
                color: p.color,
            }),
        );
        let staged =
            StagedDraw::new(&self.device, Some(mapping), underlay, &overlay, extent, context)?;
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

fn validate(
    device: &Arc<GpuDevice>,
    crop: Region,
    source: DisplayProjection<'_>,
    output: Extent,
) -> Result<(), GpuError> {
    if crop.intersection(output.region()) != Some(crop)
        || !Arc::ptr_eq(device, source.image.device())
        || source.mapping.is_some_and(|mapping| mapping.source() != source.image.extent())
        || source.underlay.is_some_and(|(image, map)| {
            source.mapping.is_none()
                || map.source() != image.extent()
                || map.output() != output
                || !Arc::ptr_eq(device, image.device())
        })
        || source.overlay.iter().any(|p| p.x >= output.width() || p.y >= output.height())
    {
        return Err(GpuError::invalid("Projected source crop has foreign or inconsistent inputs"));
    }
    Ok(())
}
