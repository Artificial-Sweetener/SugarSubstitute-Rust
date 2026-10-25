// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Behavioral image contracts exercised through production execution admission.
use super::*;
use harness::{TestResult, run};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
mod harness;
mod reference;
mod retention;
mod sampling;

const MEMORY: u64 = 64 * 1024 * 1024;
const ASSET: AssetId = AssetId([7; 32]);

struct Grid {
    extent: Extent,
}
impl RasterSource for Grid {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != ASSET
            || region.intersection(self.extent.region()) != Some(region)
            || output.len() as u64 != region.pixels()
        {
            return Err(ImageError::Invalid("Invalid test source request"));
        }
        assert!(region.width() <= TILE_EDGE && region.height() <= TILE_EDGE);
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                output[(y - region.y()) as usize * region.width() as usize
                    + (x - region.x()) as usize] = grid_pixel(x, y);
            }
        }
        Ok(())
    }
}
fn grid_pixel(x: u32, y: u32) -> Pixel {
    let alpha = ((x % 7 + y % 5 + 1) as f32) / 12.0;
    Pixel([((x % 19) as f32 / 18.0) * alpha, ((y % 23) as f32 / 22.0) * alpha, 0.25 * alpha, alpha])
}
fn graph(context: &JobContext, extent: Extent) -> Result<(Graph, NodeId), ImageError> {
    let mut graph = Graph::new(64, context.memory())?;
    let source = graph.add(Operation::Raster { asset: ASSET, extent })?;
    Ok((graph, source))
}

#[test]
fn coarse_pyramid_results_survive_source_tile_churn_while_preparing_a_neighbor() -> TestResult {
    harness::run(MEMORY, |context| {
        let extent = Extent::new(512, 512)?;
        let (mut graph, source) = graph(context, extent)?;
        let mip = graph.resize(source, Extent::new(256, 256)?)?;
        assert_eq!(graph.pyramid_level(source)?, Some(0));
        assert_eq!(graph.pyramid_level(mip)?, Some(1));
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        drop(evaluator.tile(mip, 0, 0, context)?);
        drop(evaluator.tile(mip, 1, 0, context)?);
        let prepared = evaluator.stats();
        drop(evaluator.tile(mip, 0, 0, context)?);
        assert_eq!(evaluator.stats().source_pixels, prepared.source_pixels);
        assert_eq!(evaluator.stats().computed_tiles, prepared.computed_tiles);
        assert!(evaluator.stats().reused_tiles > prepared.reused_tiles);
        Ok(())
    })
}

#[test]
fn returning_to_source_resolution_reclaims_obsolete_coarse_tiles() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1024, 512)?;
        let (mut graph, source) = graph(context, extent)?;
        let mip = graph.resize(source, Extent::new(512, 256)?)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        for y in 0..2 {
            for x in 0..4 {
                drop(evaluator.tile(mip, x, y, context)?);
            }
        }
        for x in 0..8 {
            drop(evaluator.tile(source, x, 0, context)?);
        }
        let warmed = evaluator.stats();
        for x in 0..8 {
            drop(evaluator.tile(source, x, 0, context)?);
        }
        assert_eq!(evaluator.stats().source_pixels, warmed.source_pixels);
        assert_eq!(evaluator.stats().computed_tiles, warmed.computed_tiles);
        Ok(())
    })
}

#[test]
fn deep_reductions_keep_a_working_fine_neighborhood_under_a_small_tile_budget() -> TestResult {
    struct White;
    impl RasterSource for White {
        fn read(
            &mut self,
            _: AssetId,
            _: Region,
            output: &mut [Pixel],
            _: &JobContext,
        ) -> Result<(), ImageError> {
            output.fill(Pixel::linear(1.0, 1.0, 1.0, 1.0)?);
            Ok(())
        }
    }
    run(MEMORY, |context| {
        let extent = Extent::new(1024, 1024)?;
        let (mut graph, source) = graph(context, extent)?;
        let pyramid = graph.pyramid(source)?;
        let reduced = pyramid.levels()[3];
        let mut evaluator = Evaluator::new(graph, White, 24, context.memory())?;
        let tile = evaluator.tile(reduced, 0, 0, context)?;
        assert!(
            tile.pixels()
                .iter()
                .all(|pixel| pixel.channels().iter().all(|channel| (*channel - 1.0).abs() < 1e-4))
        );
        let stats = evaluator.stats();
        assert!(
            stats.source_pixels <= extent.pixels() * 3,
            "Repeated source work exceeded the bounded neighborhood: {} pixels",
            stats.source_pixels
        );
        assert_eq!(stats.resident_tiles, 24);
        Ok(())
    })
}

#[test]
fn image_geometry_rejects_empty_overflowing_and_unaddressable_extents() -> TestResult {
    assert!(Extent::new(0, 10).is_err());
    assert!(Extent::new(u32::MAX, 1).is_err());
    assert!(Region::new(u32::MAX, 0, 1, 1).is_err());
    let region = Region::new(4, 8, 2, 3)?;
    assert!(region.contains(5, 10));
    assert!(!region.contains(6, 10));
    assert_eq!(region.intersection(Region::new(6, 8, 2, 3)?), None);
    assert_eq!(Extent::new(1_000_000_000, 1_000_000_000)?.pixels(), 1_000_000_000_000_000_000);
    Ok(())
}

#[test]
fn canonical_pixels_preserve_linear_hdr_and_reject_invalid_coverage() -> TestResult {
    let pixel = Pixel::linear(4.0, -0.5, 1.0, 0.5)?;
    assert_eq!(pixel.channels(), [2.0, -0.25, 0.5, 0.5]);
    assert!(Pixel::linear(0.0, 0.0, 0.0, 2.0).is_err());
    assert!(Pixel::linear(f32::NAN, 0.0, 0.0, 0.0).is_err());
    assert!(Pixel::premultiplied([1.0, 0.0, 0.0, 0.0]).is_err());
    assert_eq!(
        Pixel::linear(1.0, 0.0, 0.0, 0.5)?.over(Pixel::linear(0.0, 0.0, 1.0, 1.0)?)?.channels(),
        [0.5, 0.0, 0.5, 1.0]
    );
    let foreground = Pixel::premultiplied([f32::MAX, 0.0, 0.0, 0.5])?;
    let background = Pixel::linear(f32::MAX, 0.0, 0.0, 1.0)?;
    assert!(foreground.over(background).is_err());
    Ok(())
}

#[test]
fn graph_rejects_foreign_inputs_unbounded_reductions_and_size_mismatches() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(256, 256)?;
        let (mut graph, source) = graph(context, extent)?;
        let mut other = Graph::new(8, context.memory())?;
        let foreign = other.add(Operation::Raster { asset: ASSET, extent })?;
        assert!(graph.add(Operation::Exposure { input: foreign, stops: 1.0 }).is_err());
        assert!(graph.resize(source, Extent::new(1, 1)?).is_err());
        assert!(graph.add(Operation::Exposure { input: source, stops: f32::INFINITY }).is_err());
        let small = graph
            .add(Operation::Solid { extent: Extent::new(1, 1)?, color: Pixel::TRANSPARENT })?;
        assert!(graph.add(Operation::Over { background: source, foreground: small }).is_err());
        assert!(graph.add(Operation::Mask { input: source, mask: small }).is_err());
        Ok(())
    })
}

#[test]
fn graph_keys_are_structural_and_include_semantics_and_source_content() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 64)?;
        let (mut first, a) = graph(context, extent)?;
        let (mut second, b) = graph(context, extent)?;
        assert_ne!(a, b);
        assert_eq!(first.fingerprint(a)?, second.fingerprint(b)?);
        let exposure_a = first.add(Operation::Exposure { input: a, stops: 1.0 })?;
        let exposure_b = second.add(Operation::Exposure { input: b, stops: 1.0 })?;
        assert_eq!(first.fingerprint(exposure_a)?, second.fingerprint(exposure_b)?);
        let changed = second.add(Operation::Exposure { input: b, stops: 2.0 })?;
        assert_ne!(first.fingerprint(exposure_a)?, second.fingerprint(changed)?);
        let different_asset = second.add(Operation::Raster { asset: AssetId([8; 32]), extent })?;
        assert_ne!(first.fingerprint(a)?, second.fingerprint(different_asset)?);
        Ok(())
    })
}

#[test]
fn pyramid_is_explicit_and_rounds_odd_dimensions_upward() -> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(513, 129)?)?;
        let pyramid = graph.pyramid(source)?;
        let expected = [
            (513, 129),
            (257, 65),
            (129, 33),
            (65, 17),
            (33, 9),
            (17, 5),
            (9, 3),
            (5, 2),
            (3, 1),
            (2, 1),
            (1, 1),
        ];
        assert_eq!(pyramid.levels().len(), expected.len());
        for (id, (width, height)) in pyramid.levels().iter().zip(expected) {
            assert_eq!(graph.extent(*id)?, Extent::new(width, height)?);
        }
        Ok(())
    })
}

#[test]
fn insufficient_pyramid_capacity_does_not_partially_append_nodes() -> TestResult {
    run(MEMORY, |context| {
        let mut graph = Graph::new(2, context.memory())?;
        let source = graph.add(Operation::Solid {
            extent: Extent::new(1024, 1024)?,
            color: Pixel::TRANSPARENT,
        })?;
        assert!(graph.pyramid(source).is_err());
        graph.add(Operation::Exposure { input: source, stops: 1.0 })?;
        Ok(())
    })
}

#[test]
fn filter_halos_and_dirty_propagation_cover_every_influenced_sample() -> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(513, 65)?)?;
        let target = Extent::new(257, 33)?;
        let reduced = graph.resize(source, target)?;
        let changed = Region::new(255, 22, 1, 1)?;
        let dirty = graph
            .invalidated_by(reduced, source, changed)?
            .ok_or(ImageError::Invalid("Missing dirty region"))?;
        assert!(dirty.width() < target.width());
        for y in 0..target.height() {
            for x in 0..target.width() {
                let dependency = graph.required_inputs(reduced, Region::new(x, y, 1, 1)?)?[0]
                    .ok_or(ImageError::Invalid("Missing dependency"))?
                    .1;
                if dependency.intersection(changed).is_some() {
                    assert!(dirty.contains(x, y), "Missed invalidation at {x},{y}");
                }
            }
        }
        assert!(graph.required_inputs(reduced, Region::new(256, 0, 2, 1)?).is_err());
        Ok(())
    })
}

#[test]
fn huge_sources_only_read_requested_tiles_and_reuse_immutable_results() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1_000_000_000, 1_000_000_000)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let tile = evaluator.tile(source, 7_000_000, 7_000_000, context)?;
        assert_eq!(tile.region().x(), 896_000_000);
        assert_eq!(evaluator.stats().source_pixels, u64::from(TILE_EDGE * TILE_EDGE));
        let repeated = evaluator.tile(source, 7_000_000, 7_000_000, context)?;
        assert!(Arc::ptr_eq(&tile, &repeated));
        assert_eq!(evaluator.stats().computed_tiles, 1);
        assert_eq!(evaluator.stats().reused_tiles, 1);
        assert!(evaluator.tile(source, u32::MAX, 0, context).is_err());
        Ok(())
    })
}

#[test]
fn cache_retention_is_bounded_and_external_tiles_keep_memory_charges() -> TestResult {
    run(4 * 1024 * 1024, |context| {
        let extent = Extent::new(1024, 128)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 2, context.memory())?;
        let tile = evaluator.tile(source, 0, 0, context)?;
        for x in 1..8 {
            evaluator.tile(source, x, 0, context)?;
        }
        assert_eq!(evaluator.stats().resident_tiles, 2);
        evaluator.clear_tiles();
        let available = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(ImageError::Invalid("Unbounded claim accepted"))?
            .available_bytes;
        let charge = tile.payload_bytes();
        drop(tile);
        let after = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(ImageError::Invalid("Unbounded claim accepted"))?
            .available_bytes;
        assert_eq!(after - available, charge);
        Ok(())
    })
}

#[test]
fn memory_pressure_evicts_unpinned_tiles_before_rejecting_work() -> TestResult {
    run(800 * 1024, |context| {
        let extent = Extent::new(1024, 128)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        for x in 0..8 {
            evaluator.tile(source, x, 0, context)?;
        }
        assert!(evaluator.stats().resident_tiles <= 3);
        let pinned_a = evaluator.tile(source, 0, 0, context)?;
        let pinned_b = evaluator.tile(source, 1, 0, context)?;
        let pinned_c = evaluator.tile(source, 2, 0, context)?;
        assert!(matches!(evaluator.tile(source, 3, 0, context), Err(ImageError::Memory(_))));
        drop((pinned_a, pinned_b, pinned_c));
        evaluator.tile(source, 3, 0, context)?;
        Ok(())
    })
}

#[test]
fn vector_coverage_masks_exposure_and_composition_use_one_graph() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(4, 4)?;
        let mut graph = Graph::new(8, context.memory())?;
        let background =
            graph.add(Operation::Solid { extent, color: Pixel::linear(0.0, 0.0, 1.0, 1.0)? })?;
        let image =
            graph.add(Operation::Solid { extent, color: Pixel::linear(0.5, 0.0, 0.0, 1.0)? })?;
        let adjusted = graph.add(Operation::Exposure { input: image, stops: 1.0 })?;
        let shape = VectorRectangle::new([0.5, 0.5, 2.0, 2.0], Pixel::linear(1.0, 1.0, 1.0, 1.0)?)?;
        let mask = graph.add(Operation::Rectangle { extent, shape })?;
        let masked = graph.add(Operation::Mask { input: adjusted, mask })?;
        let output = graph.add(Operation::Over { background, foreground: masked })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let result = evaluator.tile(output, 0, 0, context)?;
        assert_eq!(shape.bounds(), [0.5, 0.5, 2.0, 2.0]);
        assert_eq!(result.pixel(0, 0).map(Pixel::channels), Some([0.25, 0.0, 0.75, 1.0]));
        assert_eq!(result.pixel(1, 1).map(Pixel::channels), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(result.pixel(3, 3).map(Pixel::channels), Some([0.0, 0.0, 1.0, 1.0]));
        Ok(())
    })
}

#[test]
fn source_errors_preserve_the_original_cause() -> TestResult {
    struct Failing;
    impl RasterSource for Failing {
        fn read(
            &mut self,
            _: AssetId,
            _: Region,
            _: &mut [Pixel],
            _: &JobContext,
        ) -> Result<(), ImageError> {
            Err(ImageError::Source(Box::new(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Truncated image",
            ))))
        }
    }
    run(MEMORY, |context| {
        let (graph, source) = graph(context, Extent::new(1, 1)?)?;
        let mut evaluator = Evaluator::new(graph, Failing, 2, context.memory())?;
        let cause = evaluator
            .tile(source, 0, 0, context)
            .err()
            .ok_or(ImageError::Invalid("Invalid source succeeded"))?;
        assert!(std::error::Error::source(&cause).is_some());
        assert_eq!(evaluator.stats().computed_tiles, 0);
        Ok(())
    })
}

#[test]
fn expired_evaluation_deadlines_reject_work_before_source_access() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(256, 256)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 2, context.memory())?;
        let expired =
            context.bounded_until(sugarsubstitute_contracts::execution::Moment::from_nanos(0));
        assert!(matches!(evaluator.tile(source, 0, 0, &expired), Err(ImageError::Deadline)));
        assert_eq!(evaluator.stats().source_pixels, 0);
        assert_eq!(evaluator.stats().computed_tiles, 0);
        evaluator.tile(source, 0, 0, context)?;
        assert_eq!(evaluator.stats().computed_tiles, 1);
        Ok(())
    })
}

#[test]
fn graph_capacity_must_be_admitted_before_allocation() -> TestResult {
    run(64, |context| {
        assert!(matches!(Graph::new(64, context.memory()), Err(ImageError::Memory(_))));
        assert!(Graph::new(0, context.memory()).is_err());
        assert!(Graph::new(usize::MAX, context.memory()).is_err());
        Ok(())
    })
}
