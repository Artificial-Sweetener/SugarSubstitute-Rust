// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find a mandatory sparse leaf without making conditional graph inputs eager.
use super::*;
use crate::{MAX_RASTER_SAMPLE_BATCH, RasterSamples, Region, TILE_EDGE, check};
use sugarsubstitute_contracts::execution::JobContext;

/// Bounded source-prefill eligibility for one immutable output graph.
///
/// Sampling remains on the graph's exact global grid. Exposure and foreground compositing
/// preserve that grid; finite solid/vector clip masks are tested separately for every region.
/// Unknown masks, adjustment mixes and nested reconstruction retain ordinary demand evaluation.
#[derive(Clone, Copy, Debug)]
pub struct SparseSourcePlan {
    output: NodeId,
    source: Option<NodeId>,
    working_set_nodes: usize,
}
impl SparseSourcePlan {
    /// Candidate nearest-raster operation in this graph, before region-specific clip tests.
    pub const fn source(self) -> Option<NodeId> {
        self.source
    }

    /// Maximum sibling regions whose source and ordinary intermediates fit the tile inventory.
    /// One means scalar evaluation; prefill then provides no locality benefit.
    pub fn maximum_regions(self, retained_tiles: usize) -> usize {
        if self.source.is_none() {
            return 1;
        }
        (retained_tiles / self.working_set_nodes).clamp(1, MAX_RASTER_SAMPLE_BATCH)
    }

    /// Address one candidate region only when its known clip actually needs source pixels.
    /// No original bytes are read. The result retains the ordinary nearest sampling phase.
    /// # Errors
    /// Rejects foreign graphs, oversized/outside regions and original sampling failures.
    pub fn samples(
        self,
        graph: &Graph,
        region: Region,
    ) -> Result<Option<(AssetId, RasterSamples)>, ImageError> {
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(graph.extent(self.output)?.region()) != Some(region)
        {
            return Err(ImageError::Invalid("Invalid sparse prefill region"));
        }
        let Some(source) = self.source else { return Ok(None) };
        let mut node = self.output;
        while node != source {
            node = match graph.operation(node)? {
                Operation::Exposure { input, .. } => input,
                Operation::Over { foreground, .. } => foreground,
                Operation::Mask { input, mask } => {
                    if !known_mask_nonzero(graph, mask, region)? {
                        return Ok(None);
                    }
                    input
                }
                _ => return Err(ImageError::Invalid("Sparse prefill graph path changed")),
            };
        }
        graph.sampled_source(source, region)
    }
}

impl Graph {
    /// Plan bounded horizontal source prefill while preserving lazy conditional inputs.
    /// Only known solid/vector clips may be crossed; their zero-coverage regions remain unread.
    /// Metadata uses the caller's original job admission and is released before return.
    /// # Errors
    /// Preserves graph, memory, allocation and original cancellation/deadline failures.
    pub fn sparse_source_plan(
        &self,
        output: NodeId,
        context: &JobContext,
    ) -> Result<SparseSourcePlan, ImageError> {
        check(context)?;
        self.entry(output)?;
        let _flags_claim = context.memory().claim(self.entries.len() as u64)?;
        let mut flags = Vec::new();
        flags.try_reserve_exact(self.entries.len())?;
        flags.resize(self.entries.len(), 0_u8);
        flags[usize::from(output.index)] = 3;
        let mut source = None;
        let mut working_set_nodes = 0;
        for index in (0..=usize::from(output.index)).rev() {
            check(context)?;
            let state = flags[index];
            if state & 1 == 0 {
                continue;
            }
            working_set_nodes += 1;
            let operation = self.entries[index].operation;
            for input in operation.inputs().into_iter().flatten() {
                flags[usize::from(input.index)] |= 1;
            }
            if state & 2 == 0 {
                continue;
            }
            let mandatory = match operation {
                Operation::Exposure { input, .. } => Some(input),
                Operation::Over { foreground, .. } => Some(foreground),
                Operation::Mask { input, mask }
                    if matches!(
                        self.operation(mask)?,
                        Operation::Solid { .. } | Operation::Rectangle { .. }
                    ) =>
                {
                    Some(input)
                }
                _ => None,
            };
            if let Some(input) = mandatory {
                flags[usize::from(input.index)] |= 2;
            } else {
                let node = NodeId { graph: self.identity, index: index as u16 };
                if self.sampled_source_definition(node)?.is_some() {
                    source = Some(node);
                }
            }
        }
        check(context)?;
        Ok(SparseSourcePlan { output, source, working_set_nodes })
    }
}

/// Match the evaluator's actual f32 alpha arithmetic, including underflow at tiny clip edges.
fn known_mask_nonzero(graph: &Graph, mask: NodeId, region: Region) -> Result<bool, ImageError> {
    match graph.operation(mask)? {
        Operation::Solid { color, .. } => Ok(color.channels()[3] > 0.0),
        Operation::Rectangle { shape, translation, .. } => {
            let horizontal = (region.x()..region.right())
                .map(|x| {
                    shape.axis_coverage(
                        crate::SamplingAxis::Horizontal,
                        i64::from(x) - i64::from(translation[0]),
                    )
                })
                .fold(0.0_f32, f32::max);
            let vertical = (region.y()..region.bottom())
                .map(|y| {
                    shape.axis_coverage(
                        crate::SamplingAxis::Vertical,
                        i64::from(y) - i64::from(translation[1]),
                    )
                })
                .fold(0.0_f32, f32::max);
            Ok(shape.color().channels()[3] * (horizontal * vertical) > 0.0)
        }
        _ => Err(ImageError::Invalid("Sparse prefill mask is not statically bounded")),
    }
}
