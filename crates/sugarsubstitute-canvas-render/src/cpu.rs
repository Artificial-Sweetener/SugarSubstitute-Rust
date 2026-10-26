// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare only requested viewport damage through the shared image-operation DAG.
use crate::{
    PreparedUpdate, RenderError,
    projection::{ProjectionBuffer, ProjectionBuffers},
    surface::Preparation,
    update::UpdatePlan,
};
use sugarsubstitute_canvas::RenderRequest;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{
    EvaluationStats, Evaluator, Extent, Graph, NodeId, RasterSource, SrgbEncoder,
};

/// CPU canvas preparation retaining compatible image tiles across document/view graph replacement.
/// Native presentation receives immutable device regions; authored data remains in the document.
pub struct CpuRenderer<S> {
    evaluator: Evaluator<S>,
    encoder: SrgbEncoder,
    memory: MemoryLease,
}
/// Unpublished CPU patches and their exact compiled graph identity across cooperative steps.
/// Keep this value on execution-owned workers. Dropping it releases its original pixel claims;
/// it exposes no partial update that a native presenter could accidentally publish.
pub struct CpuPreparation {
    output: NodeId,
    context: JobContext,
    pending: Option<Preparation>,
    batch_remaining: usize,
}
impl<S: RasterSource> CpuRenderer<S> {
    /// Prepare canonical display conversion and retain the application's source/memory boundary.
    /// # Errors
    /// Rejects zero/excessive retention and original display-table memory/allocation failures.
    pub fn new(source: S, maximum_tiles: usize, memory: &MemoryLease) -> Result<Self, RenderError> {
        if maximum_tiles == 0 || maximum_tiles > 4096 {
            return Err(RenderError::Invalid("Invalid canvas tile retention"));
        }
        let graph = Graph::new(1, memory)?;
        let evaluator = Evaluator::new(graph, source, maximum_tiles, memory)?;
        Ok(Self { evaluator, encoder: SrgbEncoder::new(memory)?, memory: memory.clone() })
    }
    /// Cumulative actual graph work, excluding inexpensive device-pixel blits.
    pub fn stats(&self) -> EvaluationStats {
        self.evaluator.stats()
    }
    /// Release every reconstructible graph tile after preparation is terminal or abandoned.
    /// Published updates and surfaces retain their own admitted allocations and remain valid.
    pub fn clear_graph_tiles(&mut self) {
        self.evaluator.clear_tiles();
    }
    /// Release one unpinned reconstructible DAG tile for another retained presentation allocation.
    /// Call on the preparation worker; source, document and published frame custody stay intact.
    pub fn evict_one(&mut self) -> bool {
        self.evaluator.evict_one()
    }
    /// Admit the two exclusive CPU projection allocations retained across native presentation.
    /// Both allocations use this renderer's original reservation. Under pressure only cache-only
    /// reconstructible DAG tiles are evicted; the first allocation remains owned while the second
    /// is admitted.
    /// # Errors
    /// Preserves allocation and worker-control failures, or the final memory admission failure
    /// when no eligible cached tile remains.
    pub fn projection_buffers(
        &mut self,
        output: Extent,
        context: &JobContext,
    ) -> Result<ProjectionBuffers, RenderError> {
        self.evaluator.release_sparse_batch();
        let first = self.projection_buffer(output, context)?;
        let second = self.projection_buffer(output, context)?;
        Ok(ProjectionBuffers::new(first, second))
    }
    /// Evaluate the requested document/view into immutable physical damage regions.
    /// Caller assigns revisions and chooses full damage when document content changed. Every row
    /// observes current cancellation/deadline through graph requests; no UI/native work runs here.
    /// # Errors
    /// Reports invalid revisions/plans and preserves document, source, memory and cancellation errors.
    pub fn prepare(
        &mut self,
        request: RenderRequest<'_>,
        context: &JobContext,
    ) -> Result<PreparedUpdate, RenderError> {
        let mut preparation = self.begin(request, context)?;
        loop {
            if let Some(update) = self.step(&mut preparation, context)? {
                return Ok(update);
            }
        }
    }
    /// Compile one immutable request and admit its complete bounded patch storage.
    /// The caller may yield through its existing execution job between `step` calls.
    /// Only one preparation may use this renderer's current graph at a time.
    /// # Errors
    /// Preserves document, graph, control, allocation and original admission failures.
    pub fn begin(
        &mut self,
        request: RenderRequest<'_>,
        context: &JobContext,
    ) -> Result<CpuPreparation, RenderError> {
        self.evaluator.release_sparse_batch();
        check(context)?;
        let RenderRequest { document, view, reconstruction, matte, target, base, damage } = request;
        if damage.dirty().surface() != view.surface() {
            return Err(RenderError::Invalid("Canvas damage and viewport dimensions differ"));
        }
        let update = UpdatePlan::new(target, base, damage)?;
        let compiled = document.compile_view(view, matte, reconstruction, &self.memory)?;
        let (graph, output) = compiled.into_parts();
        let pending = Some(Preparation::new(update, context.memory())?);
        self.evaluator.replace_graph(graph);
        Ok(CpuPreparation { output, pending, context: context.clone(), batch_remaining: 0 })
    }
    /// Evaluate at most one aligned viewport tile and return only a complete immutable update.
    /// A bounded same-row group may stage its mandatory sparse source leaf before that tile;
    /// composition and publication remain tile-local and demand-driven.
    /// `None` asks the caller to yield within the same original admitted job. Its original
    /// cancellation, deadline and memory custody remain authoritative. Tile dependencies use the shared DAG's bounded region
    /// rules and row-level cancellation checks; a cold pyramid dependency may require many tiles.
    /// # Errors
    /// Rejects superseded/completed preparations and preserves image/control failures. A failed
    /// tile permanently prevents its incomplete patches from becoming a published update.
    pub fn step(
        &mut self,
        preparation: &mut CpuPreparation,
        context: &JobContext,
    ) -> Result<Option<PreparedUpdate>, RenderError> {
        check(&preparation.context)?;
        if context.owner_id() != preparation.context.owner_id()
            || context.scope_id() != preparation.context.scope_id()
            || context.spec().id != preparation.context.spec().id
            || !context.memory().same_reservation(preparation.context.memory())
        {
            return Err(RenderError::Invalid("Canvas preparation belongs to another job"));
        }
        if preparation.pending.is_none() {
            return Err(RenderError::Invalid("Canvas preparation already completed"));
        }
        self.evaluator.graph().extent(preparation.output)?;
        if preparation.batch_remaining == 0 {
            let maximum =
                match self.evaluator.sparse_region_batch_limit(preparation.output, context) {
                    Ok(maximum) => maximum,
                    Err(cause) => {
                        preparation
                            .pending
                            .as_mut()
                            .ok_or(RenderError::Invalid("Canvas preparation already completed"))?
                            .fail();
                        self.evaluator.release_sparse_batch();
                        return Err(cause.into());
                    }
                };
            let siblings = preparation
                .pending
                .as_mut()
                .ok_or(RenderError::Invalid("Canvas preparation already completed"))?
                .sibling_regions(maximum);
            if let Some(siblings) = siblings {
                let length = siblings.len();
                let outcome = self.evaluator.prepare_sparse_regions(
                    preparation.output,
                    siblings.as_slice(),
                    context,
                );
                if let Err(cause) = outcome {
                    preparation
                        .pending
                        .as_mut()
                        .ok_or(RenderError::Invalid("Canvas preparation already completed"))?
                        .fail();
                    self.evaluator.release_sparse_batch();
                    return Err(cause.into());
                }
                preparation.batch_remaining = length;
            }
        }
        let pending = preparation
            .pending
            .as_mut()
            .ok_or(RenderError::Invalid("Canvas preparation already completed"))?;
        let evaluator = &mut self.evaluator;
        let encoder = &self.encoder;
        let outcome = pending.step(|region, patch, pixels| {
            let tile = evaluator.region(preparation.output, region, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let pixel = tile
                        .pixel(x, y)
                        .ok_or(RenderError::Invalid("Missing prepared canvas pixel"))?;
                    let [red, green, blue, _] = encoder.encode(pixel)?;
                    let index = (y - patch.y()) as usize * patch.width() as usize
                        + (x - patch.x()) as usize;
                    pixels[index] = u32::from_be_bytes([255, red, green, blue]);
                }
            }
            Ok(())
        });
        let complete = match outcome {
            Ok(complete) => complete,
            Err(cause) => {
                preparation.batch_remaining = 0;
                self.evaluator.release_sparse_batch();
                return Err(cause);
            }
        };
        preparation.batch_remaining = preparation.batch_remaining.saturating_sub(1);
        if preparation.batch_remaining == 0 {
            self.evaluator.release_sparse_batch();
        }
        if complete {
            let pending = preparation
                .pending
                .take()
                .ok_or(RenderError::Invalid("Canvas preparation lost its pixels"))?;
            Ok(Some(pending.finish()))
        } else {
            Ok(None)
        }
    }
    fn projection_buffer(
        &mut self,
        output: Extent,
        context: &JobContext,
    ) -> Result<ProjectionBuffer, RenderError> {
        loop {
            check(context)?;
            match ProjectionBuffer::new(output, &self.memory, context) {
                Err(
                    cause @ RenderError::Image(sugarsubstitute_image_ops::ImageError::Memory(_)),
                ) => {
                    if !self.evaluator.evict_one() {
                        return Err(cause);
                    }
                }
                outcome => return outcome,
            }
        }
    }
}

/// Check original worker controls before allocating or advancing unpublished preparation.
pub(crate) fn check(context: &JobContext) -> Result<(), RenderError> {
    if context.cancellation_requested() {
        return Err(sugarsubstitute_image_ops::ImageError::Cancelled.into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(sugarsubstitute_image_ops::ImageError::Deadline.into());
    }
    Ok(())
}
