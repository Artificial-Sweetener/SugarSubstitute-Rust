// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sparse prefill preserves clip demand, exact addressing and graph ownership.
use super::*;

struct Unavailable;
impl RasterSource for Unavailable {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        _: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        Err(ImageError::Invalid("Hidden source was evaluated"))
    }
}

/// Compile the same direct nearest reduction used by a large-image fallback.
fn reduction(graph: &mut Graph, source: NodeId) -> Result<NodeId, ImageError> {
    let target = Extent::new(1024, 512)?;
    graph.add(Operation::Resample {
        input: source,
        extent: target,
        sampling: Sampling::resize(graph.extent(source)?, target).with_filter(Filter::Nearest),
    })
}

#[test]
fn sparse_plan_crosses_document_clip_without_reading_zero_coverage_regions() -> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(4096, 2048)?)?;
        let sampled = reduction(&mut graph, source)?;
        let extent = graph.extent(sampled)?;
        let mask = graph.add(Operation::Rectangle {
            extent,
            shape: VectorRectangle::new(
                [64.25, 32.5, 959.75, 479.5],
                Pixel::linear(1.0, 1.0, 1.0, 1.0)?,
            )?,
            translation: [7, -3],
        })?;
        let clipped = graph.add(Operation::Mask { input: sampled, mask })?;
        let exposed = graph.add(Operation::Exposure { input: clipped, stops: 0.75 })?;
        let matte = Pixel::linear(0.125, 0.25, 0.375, 1.0)?;
        let background = graph.add(Operation::Solid { extent, color: matte })?;
        let output = graph.add(Operation::Over { background, foreground: exposed })?;
        let _unreachable = graph.add(Operation::Solid { extent, color: Pixel::TRANSPARENT })?;
        let plan = graph.sparse_source_plan(output, context)?;
        assert_eq!(plan.source(), Some(sampled));
        assert_eq!(plan.maximum_regions(64), 9);
        assert_eq!(plan.maximum_regions(1), 1);
        let outside = Region::new(0, 0, 64, 128)?;
        assert!(plan.samples(&graph, outside)?.is_none());
        let mixed = Region::new(64, 0, 128, 128)?;
        let (asset, actual) = plan
            .samples(&graph, mixed)?
            .ok_or(ImageError::Invalid("Visible clip lost sparse addressing"))?;
        let (expected_asset, expected) = graph
            .sampled_source(sampled, mixed)?
            .ok_or(ImageError::Invalid("Direct source addressing is unavailable"))?;
        assert_eq!(asset, expected_asset);
        assert_eq!(actual.region(), expected.region());
        assert_eq!(actual.columns(), expected.columns());
        assert_eq!(actual.rows(), expected.rows());
        let mut evaluator = Evaluator::new(graph, Unavailable, 16, context.memory())?;
        let pixels = evaluator.region(output, outside, context)?;
        assert!(pixels.pixels().iter().all(|pixel| *pixel == matte));
        Ok(())
    })
}

#[test]
fn sparse_plan_keeps_underflowed_vector_mask_sources_lazy() -> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(4096, 2048)?)?;
        let sampled = reduction(&mut graph, source)?;
        let mask = graph.add(Operation::Rectangle {
            extent: graph.extent(sampled)?,
            shape: VectorRectangle::new(
                [0.0, 0.0, 1.0e-16, 1.0e-16],
                Pixel::linear(1.0, 1.0, 1.0, 1.0e-16)?,
            )?,
            translation: [0; 2],
        })?;
        let output = graph.add(Operation::Mask { input: sampled, mask })?;
        let region = Region::new(0, 0, 128, 128)?;
        let plan = graph.sparse_source_plan(output, context)?;
        assert_eq!(plan.source(), Some(sampled));
        assert!(plan.samples(&graph, region)?.is_none());
        let mut evaluator = Evaluator::new(graph, Unavailable, 16, context.memory())?;
        assert!(
            evaluator
                .region(output, region, context)?
                .pixels()
                .iter()
                .all(|pixel| pixel.channels()[3] == 0.0)
        );
        Ok(())
    })
}

#[test]
fn sparse_plan_declines_unknown_masks_mixes_and_nested_sampling_and_rejects_foreign_nodes()
-> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(4096, 2048)?)?;
        let sampled = reduction(&mut graph, source)?;
        let extent = graph.extent(sampled)?;
        let solid =
            graph.add(Operation::Solid { extent, color: Pixel::linear(0.0, 0.0, 0.0, 0.5)? })?;
        let unknown = graph.add(Operation::Exposure { input: solid, stops: 0.0 })?;
        let masked = graph.add(Operation::Mask { input: sampled, mask: unknown })?;
        let mixed = graph.add(Operation::Mix {
            original: sampled,
            adjusted: masked,
            mask: unknown,
            opacity: 0.0,
        })?;
        let smaller = Extent::new(512, 256)?;
        let nested = graph.add(Operation::Resample {
            input: sampled,
            extent: smaller,
            sampling: Sampling::resize(extent, smaller).with_filter(Filter::Nearest),
        })?;
        for output in [masked, mixed, nested] {
            let plan = graph.sparse_source_plan(output, context)?;
            assert!(plan.source().is_none());
            assert_eq!(plan.maximum_regions(128), 1);
        }
        let plan = graph.sparse_source_plan(sampled, context)?;
        let (other, foreign) = super::graph(context, Extent::new(8, 8)?)?;
        assert!(graph.sparse_source_plan(foreign, context).is_err());
        assert!(plan.samples(&other, Region::new(0, 0, 1, 1)?).is_err());
        assert!(plan.samples(&graph, Region::new(0, 0, 129, 1)?).is_err());
        assert!(plan.samples(&graph, Region::new(1024, 0, 1, 1)?).is_err());
        Ok(())
    })
}
