// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare sRGB8 GPU damage tiles through the same document compilation as CPU presentation.
use crate::{GpuSurface, RenderError, SurfaceWork, update::UpdatePlan};
use std::sync::Arc;
use sugarsubstitute_canvas::{DamagePlan, FrameStamp, RenderRequest};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_gpu::{DisplayRegions, DisplayTile, GpuDevice, GpuEvaluator, GpuWork};
use sugarsubstitute_image_ops::{EvaluationStats, Graph, ImageError, RasterSource, Region};

/// Worker-prepared immutable opaque sRGB8 GPU regions with an exact retained-frame dependency.
/// Pixel allocations, inventory and update metadata retain their original execution charges.
/// Native presentation owns window/surface affinity and consumes these completed results.
pub struct PreparedGpuUpdate {
    pub(crate) update: UpdatePlan,
    regions: DisplayRegions,
    _memory: MemoryClaim,
    sampling_scale: f64,
}
impl PreparedGpuUpdate {
    /// Conservative samples per document pixel represented by the selected source pyramids.
    pub const fn sampling_scale(&self) -> f64 {
        self.sampling_scale
    }
    /// Exact application-assigned destination generation.
    pub const fn target(&self) -> FrameStamp {
        self.update.target
    }
    /// Exact retained base required by this repair, or none for full replacement.
    pub const fn base(&self) -> Option<FrameStamp> {
        self.update.base
    }
    /// Retained-pixel translation followed by the exact physical damage inventory.
    pub const fn plan(&self) -> &DamagePlan {
        &self.update.damage
    }
    /// Completed opaque sRGB8 regions in row-major tile order within each damage rectangle.
    pub fn tiles(&self) -> &[DisplayTile] {
        self.regions.tiles()
    }
}
/// Canvas preparation for one startup-selected GPU, retaining compatible graph content across views.
/// Construction, preparation and release run on an admitted worker; native handles stay outside.
pub struct GpuRenderer<S> {
    evaluator: GpuEvaluator<S>,
    memory: MemoryLease,
}
impl<S: RasterSource> GpuRenderer<S> {
    /// Compile kernels and retain source/memory custody for the selected device.
    /// # Errors
    /// Preserves original admission, graph, allocation, control and driver errors.
    pub fn new(
        source: S,
        maximum_tiles: usize,
        device: Arc<GpuDevice>,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, RenderError> {
        let graph = Graph::new(1, memory)?;
        Ok(Self {
            evaluator: GpuEvaluator::new(graph, source, maximum_tiles, device, memory, context)?,
            memory: memory.clone(),
        })
    }
    /// Logical image work, including compatible content reuse across document/view replacement.
    pub fn stats(&self) -> EvaluationStats {
        self.evaluator.stats()
    }
    /// Completed native batches, operation dispatches and actual host transfer bytes.
    pub fn work(&self) -> GpuWork {
        self.evaluator.work()
    }
    /// Release one cache-only graph reference for another admitted GPU presentation allocation.
    /// Outside frames and native commands retain their own original allocation custody.
    pub fn evict_one(&mut self) -> bool {
        self.evaluator.evict_one()
    }
    /// Complete retained GPU copies under the same finite device admission as DAG evaluation.
    /// On execution memory pressure, evict only unpinned reconstructible graph tiles before retrying.
    /// Immutable published frames and prepared display regions keep their original custody.
    /// # Errors
    /// Rejects stale dependencies and preserves admission/control/native causes when no further
    /// eligible tile can be released. Failures leave the preceding retained frame unchanged.
    pub fn retain(
        &mut self,
        surface: &mut GpuSurface,
        update: &PreparedGpuUpdate,
        context: &JobContext,
    ) -> Result<SurfaceWork, RenderError> {
        loop {
            match surface.apply(update, context) {
                Err(RenderError::Gpu(cause))
                    if cause.is_memory_pressure() && self.evaluator.evict_one() => {}
                outcome => return outcome,
            }
        }
    }
    /// Compile this document/view and prepare only its requested physical damage on the GPU.
    /// No CPU-rendering fallback, native window work or mutable producer callback reaches the result.
    /// All native batches retire and validate before publication; the caller rejects stale requests.
    /// # Errors
    /// Rejects invalid dependencies, damage/view mismatch and inventories above 4096 tiles;
    /// preserves original document, source, admission, control and native causes.
    pub fn prepare(
        &mut self,
        request: RenderRequest<'_>,
        context: &JobContext,
    ) -> Result<PreparedGpuUpdate, RenderError> {
        self.prepare_request(request, context, false)
    }
    /// Prepare navigation from the nearest ready canonical source level covering the viewport.
    /// Selection inspects shared retention metadata and performs no native calls or source I/O.
    /// # Errors
    /// Preserves ordinary graph, native, control and admission errors through the owning job.
    pub fn prepare_navigation(
        &mut self,
        request: RenderRequest<'_>,
        context: &JobContext,
    ) -> Result<PreparedGpuUpdate, RenderError> {
        self.prepare_request(request, context, true)
    }
    fn prepare_request(
        &mut self,
        request: RenderRequest<'_>,
        context: &JobContext,
        navigation: bool,
    ) -> Result<PreparedGpuUpdate, RenderError> {
        let RenderRequest { document, view, reconstruction, matte, target, base, damage } = request;
        if damage.dirty().surface() != view.surface() {
            return Err(RenderError::Invalid("Canvas damage and viewport dimensions differ"));
        }
        let update = UpdatePlan::new(target, base, damage)?;
        let inventory = RepairInventory::new(&damage, context.memory())?;
        let metadata = context
            .memory()
            .claim(size_of::<PreparedGpuUpdate>() as u64 + 128)
            .map_err(ImageError::from)?;
        let compiled = if navigation {
            document.compile_navigation_view(
                view,
                matte,
                reconstruction,
                &self.memory,
                |graph, node, region| {
                    Ok(self.evaluator.has_cached_region(graph.fingerprint(node)?, region))
                },
            )?
        } else {
            document.compile_view(view, matte, reconstruction, &self.memory)?
        };
        let sampling_scale = compiled.sampling_scale();
        let (graph, output) = compiled.into_parts();
        self.evaluator.replace_graph(graph);
        let regions = self.evaluator.display_regions(output, &inventory.regions, context)?;
        Ok(PreparedGpuUpdate { update, regions, _memory: metadata, sampling_scale })
    }
}
struct RepairInventory {
    regions: Vec<Region>,
    _memory: MemoryClaim,
}
impl RepairInventory {
    fn new(damage: &DamagePlan, memory: &MemoryLease) -> Result<Self, RenderError> {
        let count: u64 = damage.dirty().regions().map(Region::tile_count).sum();
        if count > 4096 {
            return Err(RenderError::Invalid("Canvas repair tile inventory exceeds its bound"));
        }
        let claim =
            memory.claim(count * size_of::<Region>() as u64 + 128).map_err(ImageError::from)?;
        let mut regions = Vec::new();
        regions.try_reserve_exact(count as usize).map_err(ImageError::from)?;
        for region in damage.dirty().regions() {
            regions.extend(region.tiles());
        }
        Ok(Self { regions, _memory: claim })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::harness;
    use sugarsubstitute_image_ops::Extent;

    #[test]
    fn gpu_repair_inventory_preserves_exact_pan_strips_and_shared_tile_phase() -> harness::TestResult
    {
        harness::run(1024 * 1024, |context| {
            let extent = Extent::new(513, 385)?;
            for (dx, dy) in [(0, 0), (7, -11), (-129, 131), (513, 0)] {
                let damage = DamagePlan::pan(extent, dx, dy)
                    .map_err(|cause| ImageError::Source(Box::new(cause)))?;
                let inventory = RepairInventory::new(&damage, context.memory())
                    .map_err(|cause| ImageError::Source(Box::new(cause)))?;
                assert_eq!(
                    inventory.regions.iter().map(|region| region.pixels()).sum::<u64>(),
                    damage.dirty().pixels()
                );
                for region in &inventory.regions {
                    assert!(region.width() <= 128 && region.height() <= 128);
                    assert!(
                        damage
                            .dirty()
                            .regions()
                            .any(|dirty| dirty.intersection(*region) == Some(*region))
                    );
                }
                for (index, a) in inventory.regions.iter().enumerate() {
                    assert!(
                        inventory.regions[index + 1..].iter().all(|b| a.intersection(*b).is_none())
                    );
                }
            }
            Ok(())
        })
    }
    #[test]
    fn gpu_repair_inventory_rejects_excessive_viewports_before_allocating() -> harness::TestResult {
        harness::run(1024 * 1024, |context| {
            let damage = DamagePlan::full(Extent::new(16384, 16384)?);
            assert!(RepairInventory::new(&damage, context.memory()).is_err());
            Ok(())
        })
    }
}
