// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish completed region inventories after bounded batches and dynamic mask decisions settle.
use crate::{GpuError, GpuEvaluator, GpuTile, batch::Batch, kernel::Computed};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{NodeId, RasterSource, Region};

/// Immutable completed device regions with their original inventory admission.
pub struct GpuRegions {
    tiles: Vec<GpuTile>,
    _memory: MemoryClaim,
}
impl GpuRegions {
    /// Exact requested regions in caller order; all native commands and validation have retired.
    pub fn tiles(&self) -> &[GpuTile] {
        &self.tiles
    }
}
impl<S: RasterSource> GpuEvaluator<S> {
    /// Prepare up to 4096 exact regions through the same graph in bounded native batches.
    /// Returned immutable allocations may outlive this job and evaluator. Cancellation prevents
    /// publication after actual retirement; no queued resource charge escapes unfinished work.
    /// # Errors
    /// Preserves original graph/source/control/admission/native causes and rejects excessive input.
    pub fn regions(
        &mut self,
        node: NodeId,
        regions: &[Region],
        context: &JobContext,
    ) -> Result<GpuRegions, GpuError> {
        if regions.len() > 4096 {
            return Err(GpuError::invalid("GPU region inventory exceeds its bound"));
        }
        let memory = context
            .memory()
            .claim((regions.len() * size_of::<GpuTile>()) as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_region_inventory", cause))?;
        let mut tiles = Vec::new();
        tiles
            .try_reserve_exact(regions.len())
            .map_err(|cause| GpuError::new("gpu_region_allocation", cause))?;
        let result = (|| {
            for region in regions {
                tiles.push(self.computed(node, *region, context)?.tile.clone());
            }
            Ok(())
        })();
        self.settle(result, context)?;
        Ok(GpuRegions { tiles, _memory: memory })
    }
    pub(crate) fn ready(&mut self, context: &JobContext) -> Result<&mut Batch, GpuError> {
        crate::adapter::check(context)?;
        if self.batch.as_ref().is_some_and(Batch::full) {
            self.finish_batch()?;
        }
        if self.batch.is_none() {
            self.batch = Some(Batch::new(self.device.clone(), context)?);
        }
        let batch =
            self.batch.as_mut().ok_or_else(|| GpuError::invalid("Image batch is unavailable"))?;
        batch.start()?;
        Ok(batch)
    }
    pub(crate) fn coverage(&mut self, computed: &Computed) -> Result<bool, GpuError> {
        if let Some(value) = computed.coverage.get()? {
            return Ok(value);
        }
        self.finish_batch()?;
        computed.coverage.get()?.ok_or_else(|| GpuError::invalid("Image coverage was not retired"))
    }
    pub(crate) fn finish_batch(&mut self) -> Result<(), GpuError> {
        if let Some(batch) = self.batch.take() {
            self.work.add(batch.finish()?);
        }
        Ok(())
    }
    pub(crate) fn settle<T>(
        &mut self,
        result: Result<T, GpuError>,
        context: &JobContext,
    ) -> Result<T, GpuError> {
        let retired = self.finish_batch();
        let result = match (result, retired) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(primary), Err(retirement)) => Err(GpuError::retirement(primary, retirement)),
            (Err(cause), _) | (_, Err(cause)) => Err(cause),
        }
        .and_then(|value| {
            crate::adapter::check(context)?;
            Ok(value)
        });
        if result.is_err() {
            self.cache.clear();
        }
        result
    }
}
impl<S> Drop for GpuEvaluator<S> {
    fn drop(&mut self) {
        if let Some(batch) = self.batch.take()
            && let Err(cause) = batch.finish()
        {
            tracing::error!(operation="image_gpu_evaluator_retirement", cause=%cause, "Retired queued image work before evaluator teardown");
        }
    }
}
