// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare only requested viewport damage through the shared image-operation DAG.
use crate::{PreparedUpdate, RenderError, update::UpdatePlan};
use sugarsubstitute_canvas::RenderRequest;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{EvaluationStats, Evaluator, Graph, RasterSource, SrgbEncoder};

/// CPU canvas preparation retaining compatible image tiles across document/view graph replacement.
/// Native presentation receives immutable device regions; authored data remains in the document.
pub struct CpuRenderer<S> {
    evaluator: Evaluator<S>,
    encoder: SrgbEncoder,
    memory: MemoryLease,
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
        let RenderRequest { document, view, filter, matte, target, base, damage } = request;
        if damage.dirty().surface() != view.surface() {
            return Err(RenderError::Invalid("Canvas damage and viewport dimensions differ"));
        }
        let update = UpdatePlan::new(target, base, damage)?;
        let compiled = document
            .compile_view(view, matte, filter, &self.memory)
            .map_err(RenderError::Canvas)?;
        let (graph, output) = compiled.into_parts();
        self.evaluator.replace_graph(graph);
        let evaluator = &mut self.evaluator;
        let encoder = &self.encoder;
        PreparedUpdate::prepare_validated(update, context.memory(), |region, pixels| {
            for overlap in region.tiles() {
                let tile = evaluator.region(output, overlap, context)?;
                for y in overlap.y()..overlap.bottom() {
                    for x in overlap.x()..overlap.right() {
                        let pixel = tile
                            .pixel(x, y)
                            .ok_or(RenderError::Invalid("Missing prepared canvas pixel"))?;
                        let [red, green, blue, _] = encoder.encode(pixel)?;
                        let index = (y - region.y()) as usize * region.width() as usize
                            + (x - region.x()) as usize;
                        pixels[index] = u32::from_be_bytes([255, red, green, blue]);
                    }
                }
            }
            Ok(())
        })
    }
}
