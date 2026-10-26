// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preview graphs stay sparse while finished reductions retain canonical Lanczos semantics.

use super::*;
use crate::{Document, DocumentEdit, Layer, LayerId};
use sugarsubstitute_image_ops::{Filter, ImageError};

#[test]
fn preview_skips_pyramids_while_finished_raster_and_mask_keep_lanczos_reductions()
-> crate::tests::harness::TestResult {
    crate::tests::harness::run(2 * 1024 * 1024, |context| {
        compile_policy(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn compile_policy(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), CanvasError> {
    let extent = Extent::new(64, 64)?;
    let mut document = Document::new(1, extent, context.memory())?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
            .with_mask(Some(LayerMask::Raster { asset: AssetId([2; 32]), extent }));
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    let mut view = Viewport::new(Extent::new(8, 8)?)?;
    view.zoom_at(0.125, [0.0; 2])?;
    let snapshot = document.snapshot();
    let compile = |reconstruction| -> Result<[u32; 3], CanvasError> {
        let compiled = snapshot.compile_view(
            view,
            Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
            reconstruction,
            context.memory(),
        )?;
        let mut filters = [0_u32; 3];
        collect_filters(compiled.graph(), compiled.output(), &mut filters)?;
        Ok(filters)
    };
    assert_eq!(compile(Reconstruction::Preview)?, [0, 2, 0]);
    assert_eq!(compile(Reconstruction::Finished(Filter::Nearest))?, [6, 2, 0]);
    assert_eq!(compile(Reconstruction::Finished(Filter::Lanczos3))?, [8, 0, 0]);
    assert_eq!(compile(Reconstruction::Finished(Filter::Bilinear))?, [6, 0, 2]);
    Ok(())
}

fn collect_filters(graph: &Graph, node: NodeId, filters: &mut [u32; 3]) -> Result<(), ImageError> {
    match graph.operation(node)? {
        Operation::Raster { .. } | Operation::Solid { .. } | Operation::Rectangle { .. } => {}
        Operation::Resample { input, sampling, .. } => {
            filters[match sampling.filter() {
                Filter::Lanczos3 => 0,
                Filter::Nearest => 1,
                Filter::Bilinear => 2,
            }] += 1;
            collect_filters(graph, input, filters)?;
        }
        Operation::Exposure { input, .. } => collect_filters(graph, input, filters)?,
        Operation::Over { background, foreground } => {
            collect_filters(graph, background, filters)?;
            collect_filters(graph, foreground, filters)?;
        }
        Operation::Mask { input, mask } => {
            collect_filters(graph, input, filters)?;
            collect_filters(graph, mask, filters)?;
        }
        Operation::Mix { original, adjusted, mask, .. } => {
            collect_filters(graph, original, filters)?;
            collect_filters(graph, adjusted, filters)?;
            collect_filters(graph, mask, filters)?;
        }
    }
    Ok(())
}
