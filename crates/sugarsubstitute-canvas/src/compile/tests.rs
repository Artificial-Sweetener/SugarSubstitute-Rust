// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preview graphs stay sparse while finished reductions retain canonical Lanczos semantics.

use super::*;
use crate::{Document, DocumentEdit, Layer, LayerId};
use sugarsubstitute_image_ops::{Filter, ImageError};

#[test]
fn navigation_selects_the_nearest_ready_raster_and_mask_pyramids()
-> crate::tests::harness::TestResult {
    crate::tests::harness::run(2 * 1024 * 1024, |context| {
        let (document, view) =
            raster_fixture(context).map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let snapshot = document.snapshot();
        let matte = Pixel::linear(0.0, 0.0, 0.0, 1.0)?;
        let intent = Reconstruction::Finished(Filter::Bilinear);
        let ready = snapshot
            .compile_navigation_view(
                view,
                None,
                matte,
                intent,
                context.memory(),
                |graph, node, region| {
                    assert_eq!(region.intersection(graph.extent(node)?.region()), Some(region));
                    Ok(graph.pyramid_level(node)?.is_some_and(|level| level >= 4))
                },
            )
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(ready.sampling_scale(), 1.0 / 16.0);
        let mut filters = [0; 3];
        collect_filters(ready.graph(), ready.output(), &mut filters)?;
        assert_eq!(filters, [8, 0, 2]);
        let ideal = snapshot
            .compile_navigation_view(view, None, matte, intent, context.memory(), |_, _, _| {
                Ok(true)
            })
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(ideal.sampling_scale(), view.zoom());
        let cold = snapshot
            .compile_navigation_view(view, None, matte, intent, context.memory(), |_, _, _| {
                Ok(false)
            })
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(cold.sampling_scale(), view.zoom());
        let mut filters = [0; 3];
        collect_filters(cold.graph(), cold.output(), &mut filters)?;
        assert_eq!(filters, [6, 0, 2]);
        Ok(())
    })
}

#[test]
fn exact_settling_and_pixel_inspection_do_not_substitute_cached_coarser_levels()
-> crate::tests::harness::TestResult {
    crate::tests::harness::run(2 * 1024 * 1024, |context| {
        let (document, mut view) =
            raster_fixture(context).map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let snapshot = document.snapshot();
        let matte = Pixel::linear(0.0, 0.0, 0.0, 1.0)?;
        let unexpected_query = |_: &Graph, _: NodeId, _: Region| {
            Err(ImageError::Invalid("Unexpected readiness query"))
        };
        let exact = snapshot
            .compile_navigation_view(
                view,
                None,
                matte,
                Reconstruction::Finished(Filter::Lanczos3),
                context.memory(),
                unexpected_query,
            )
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(exact.sampling_scale(), view.zoom());
        view.reframe(view.origin(), 2.5).map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let nearest = snapshot
            .compile_navigation_view(
                view,
                None,
                matte,
                Reconstruction::navigation_for_display_zoom(2.5)?,
                context.memory(),
                unexpected_query,
            )
            .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        assert_eq!(nearest.sampling_scale(), 1.0);
        Ok(())
    })
}

#[test]
fn navigation_preserves_readiness_boundary_errors() -> crate::tests::harness::TestResult {
    crate::tests::harness::run(2 * 1024 * 1024, |context| {
        let (document, view) =
            raster_fixture(context).map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let result = document.snapshot().compile_navigation_view(
            view,
            None,
            Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
            Reconstruction::Finished(Filter::Bilinear),
            context.memory(),
            |_, _, _| Err(ImageError::Invalid("Fixture readiness failure")),
        );
        assert!(matches!(
            result,
            Err(CanvasError::Image(ImageError::Invalid("Fixture readiness failure")))
        ));
        Ok(())
    })
}

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
    let (document, view) = raster_fixture(context)?;
    let snapshot = document.snapshot();
    let compile = |reconstruction| -> Result<[u32; 3], CanvasError> {
        let compiled = snapshot.compile_view(
            view,
            None,
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

fn raster_fixture(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(Document, Viewport), CanvasError> {
    let extent = Extent::new(64, 64)?;
    let mut document = Document::new(1, extent, context.memory())?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
            .with_mask(Some(LayerMask::Raster { asset: AssetId([2; 32]), extent }));
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    let mut view = Viewport::new(Extent::new(8, 8)?)?;
    view.zoom_at(0.125, [0.0; 2])?;
    Ok((document, view))
}
