// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare small disposable material patches from the exact clean GPU display projection.
use super::{BackdropCrop, BackdropPlan};
use crate::RenderError;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_gpu::{
    DisplayImage, DisplayPatch, DisplayProjection, DisplayProjector, GpuDevice, GpuEvaluator,
};
use sugarsubstitute_image_ops::{AssetId, Region};

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// One complete small rounded patch; image and mask claims remain owned through native drawing.
pub struct GpuBackdropPatch {
    original: DisplayImage,
    filtered: DisplayImage,
    destination: [u32; 2],
    coverage: Vec<u8>,
    _memory: MemoryClaim,
}
impl GpuBackdropPatch {
    /// Borrow exactly the clean/filtered pixels and shared deterministic geometric mask.
    pub fn layer(&self) -> DisplayPatch<'_> {
        DisplayPatch {
            original: &self.original,
            filtered: &self.filtered,
            destination: self.destination,
            coverage: &self.coverage,
        }
    }
}
/// At most two immutable patches in the shared plan's composition order.
pub struct GpuBackdropPatches {
    patches: Vec<GpuBackdropPatch>,
    _memory: MemoryClaim,
}
impl GpuBackdropPatches {
    /// Patches are transient presentation products, never retained document or renderer backings.
    pub fn patches(&self) -> &[GpuBackdropPatch] {
        &self.patches
    }
}
/// Worker-owned selected-device pipelines. Input bindings and computed tiles are disposable;
/// instance/prepare identities never alias another session's private presentation sources.
pub struct GpuBackdrop {
    device: Arc<GpuDevice>,
    projector: DisplayProjector,
    evaluator: Option<GpuEvaluator<()>>,
    retained: MemoryLease,
    owner: u64,
    sequence: u64,
}
impl GpuBackdrop {
    /// Conservative host scratch for one complete prepare/draw, in addition to retained pixels.
    /// Includes bounded DAG batch metadata/uploads, initial shared decode table, mapping staging,
    /// recursive coefficient tables, inventory and two copies of each encoded mask upload.
    /// # Errors
    /// Rejects arithmetic overflow before the execution owner admits a job.
    pub fn scratch_bytes(plan: &BackdropPlan, overlay_pixels: usize) -> Result<u64, RenderError> {
        if plan.regions().is_empty() {
            return Ok(0);
        }
        let mut bytes = 16_u64 * 1024 * 1024;
        bytes = bytes
            .checked_add(
                (u64::from(plan.viewport().width()) + u64::from(plan.viewport().height())) * 64,
            )
            .and_then(|n| n.checked_add((overlay_pixels as u64).checked_mul(24)?))
            .ok_or(RenderError::Invalid("GPU backdrop metadata admission overflow"))?;
        for crop in plan.regions() {
            bytes = bytes
                .checked_add(
                    crop.inner_region()
                        .pixels()
                        .checked_mul(9)
                        .ok_or(RenderError::Invalid("GPU backdrop mask admission overflow"))?,
                )
                .and_then(|n| n.checked_add(crop.crop().tile_count() * 256))
                .ok_or(RenderError::Invalid("GPU backdrop admission overflow"))?;
        }
        Ok(bytes)
    }
    /// Compile reusable projection/linearization on the existing explicitly selected GPU.
    /// `retained` admits graph, source inventory and cache metadata for this target lifetime;
    /// each prepare uses its fresh job context only for controls and temporary scratch.
    /// # Errors
    /// Preserves original device, control, memory and process-local identity exhaustion causes.
    pub fn new(
        device: Arc<GpuDevice>,
        retained: MemoryLease,
        context: &JobContext,
    ) -> Result<Self, RenderError> {
        let owner = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| RenderError::Invalid("GPU backdrop identity exhausted"))?;
        let projector = DisplayProjector::new(device.clone(), context)?;
        Ok(Self { device, projector, evaluator: None, retained, owner, sequence: 0 })
    }
    /// Evaluate the exact shared phased crop DAG and retain only its inner encoded patch.
    /// Every source is sampled from the clean immutable projection, including when bars overlap.
    /// No host image read, pixel upload, static material or full-viewport float image is used.
    /// # Errors
    /// Preserves original graph/control/native causes and rejects foreign projection dimensions.
    pub fn prepare(
        &mut self,
        plan: &BackdropPlan,
        source: DisplayProjection<'_>,
        context: &JobContext,
    ) -> Result<GpuBackdropPatches, RenderError> {
        let output = source.mapping.map_or(source.image.extent(), |map| map.output());
        if output != plan.viewport() {
            return Err(RenderError::Invalid("GPU backdrop differs from its physical viewport"));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(RenderError::Invalid("GPU backdrop source sequence exhausted"))?;
        let memory = context
            .memory()
            .claim((plan.regions().len() * size_of::<GpuBackdropPatch>()) as u64 + 256)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut patches = Vec::new();
        patches
            .try_reserve_exact(plan.regions().len())
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        for (index, crop) in plan.regions().iter().enumerate() {
            let mut identity = [0_u8; 32];
            identity[..8].copy_from_slice(b"SS-BACKD");
            identity[8..16].copy_from_slice(&self.owner.to_le_bytes());
            identity[16..24].copy_from_slice(&self.sequence.to_le_bytes());
            identity[24..].copy_from_slice(&(index as u64).to_le_bytes());
            patches.push(self.prepare_crop(*crop, AssetId(identity), source, context)?);
        }
        Ok(GpuBackdropPatches { patches, _memory: memory })
    }
    fn prepare_crop(
        &mut self,
        crop: BackdropCrop,
        asset: AssetId,
        source: DisplayProjection<'_>,
        context: &JobContext,
    ) -> Result<GpuBackdropPatch, RenderError> {
        let binding =
            self.projector.project_raster(asset, crop.crop(), source, &self.retained, context)?;
        let graph = crop.graph(asset, &self.retained)?;
        let capacity = GpuEvaluator::raster_tile_capacity(&graph.graph, graph.output)?;
        if let Some(evaluator) = &mut self.evaluator {
            evaluator.reserve_raster_tiles(capacity, &self.retained)?;
            evaluator.replace_raster(graph.graph, graph.output, binding)?;
        } else {
            self.evaluator = Some(GpuEvaluator::from_raster(
                graph.graph,
                graph.output,
                binding,
                capacity,
                self.device.clone(),
                &self.retained,
                context,
            )?);
        }
        let evaluator =
            self.evaluator.as_mut().ok_or(RenderError::Invalid("GPU backdrop evaluator absent"))?;
        let inner = crop.inner_region();
        let _inventory = context
            .memory()
            .claim(inner.tile_count() * size_of::<Region>() as u64 + 128)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(inner.tile_count() as usize)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        regions.extend(inner.tiles());
        let prepared = evaluator.display_regions(graph.output, &regions, context)?;
        let filtered =
            DisplayImage::compose_region(self.device.clone(), inner, prepared.tiles(), context)?;
        let destination = [crop.crop().x() + inner.x(), crop.crop().y() + inner.y()];
        let viewport = Region::new(destination[0], destination[1], inner.width(), inner.height())?;
        let original = self.projector.project_image(viewport, source, context)?;
        let memory = context
            .memory()
            .claim(inner.pixels() + 128)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut coverage = Vec::new();
        coverage
            .try_reserve_exact(inner.pixels() as usize)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        for y in viewport.y()..viewport.bottom() {
            crate::cpu::check(context)?;
            for x in viewport.x()..viewport.right() {
                coverage.push((crop.coverage(x, y) * 16.0).round() as u8);
            }
        }
        Ok(GpuBackdropPatch { original, filtered, destination, coverage, _memory: memory })
    }
}
