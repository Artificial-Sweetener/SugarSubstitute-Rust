// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse value-identical composites without trapping shared payloads in cache-only references.
use super::*;
use crate::TileRetention;

#[test]
fn evaluator_pressure_eviction_preserves_outside_pixels_and_releases_only_cache_custody()
-> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 4, context.memory())?;
        let outside = evaluator.tile(source, 0, 0, context)?;
        let expected = outside.pixel(17, 19);
        assert!(!evaluator.evict_one());
        assert_eq!(outside.pixel(17, 19), expected);
        drop(outside);
        assert!(evaluator.evict_one());
        assert_eq!(evaluator.stats().resident_tiles, 0);
        assert!(!evaluator.evict_one());
        let reconstructed = evaluator.tile(source, 0, 0, context)?;
        assert_eq!(reconstructed.pixel(17, 19), expected);
        Ok(())
    })
}

#[test]
fn cache_only_aliases_release_payloads_but_external_results_keep_the_original_charge() -> TestResult
{
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        let (graph, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 4, context.memory())?;
        let outside = evaluator.tile(source, 0, 0, context)?;
        evaluator.clear_tiles();
        let expected = outside.pixel(17, 19);
        let payload = outside.payload_bytes();
        let mut cache = TileRetention::new(4, context.memory())?;
        cache.insert([1; 32], extent.region(), Some(0), outside.clone());
        cache.insert([2; 32], extent.region(), None, outside.clone());
        assert!(!cache.evict_unpinned(|_| true));
        let available = || -> Result<u64, ImageError> {
            Ok(context
                .memory()
                .claim(u64::MAX)
                .err()
                .ok_or(ImageError::Invalid("Unbounded claim accepted"))?
                .available_bytes)
        };
        let before = available()?;
        drop(outside);
        assert!(cache.evict_unpinned(|_| true));
        assert_eq!(cache.len(), 1);
        let surviving = cache
            .find([1; 32], extent.region(), Some(0))
            .ok_or(ImageError::Invalid("Preferred original tile was not retained"))?;
        assert_eq!(surviving.pixel(17, 19), expected);
        assert!(!cache.evict_unpinned(|_| true));
        drop(surviving);
        assert!(cache.evict_unpinned(|_| true));
        assert!(cache.is_empty());
        assert_eq!(available()? - before, payload);
        Ok(())
    })
}

struct OpaqueSource;
impl RasterSource for OpaqueSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        output.fill(Pixel::linear(0.5, 0.25, 0.125, 1.0)?);
        Ok(())
    }
}

#[test]
fn coverage_underflow_rejects_colored_zero_alpha_before_tiles_can_be_reused() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1, 1)?;
        let mut graph = Graph::new(4, context.memory())?;
        let color = Pixel::premultiplied([0.25, 0.0, 0.0, f32::from_bits(1)])?;
        let input = graph.add(Operation::Solid { extent, color })?;
        let mask =
            graph.add(Operation::Solid { extent, color: Pixel::linear(0.0, 0.0, 0.0, 0.25)? })?;
        let masked = graph.add(Operation::Mask { input, mask })?;
        let vector = graph.add(Operation::Rectangle {
            extent,
            shape: VectorRectangle::new([0.75, 0.0, 1.0, 1.0], color)?,
            translation: [0; 2],
        })?;
        let mut evaluator = Evaluator::new(graph, OpaqueSource, 8, context.memory())?;
        for output in [masked, vector] {
            assert!(matches!(
                evaluator.region(output, extent.region(), context),
                Err(ImageError::Invalid(_))
            ));
        }
        Ok(())
    })
}

fn composite(
    context: &JobContext,
    extent: Extent,
    shape: VectorRectangle,
) -> Result<(Graph, NodeId, NodeId), ImageError> {
    let (mut graph, source) = graph(context, extent)?;
    let transparent = graph.add(Operation::Solid { extent, color: Pixel::TRANSPARENT })?;
    let matte =
        graph.add(Operation::Solid { extent, color: Pixel::linear(0.125, 0.25, 0.5, 1.0)? })?;
    let mask = graph.add(Operation::Rectangle { extent, shape, translation: [0; 2] })?;
    let masked = graph.add(Operation::Mask { input: source, mask })?;
    let over = graph.add(Operation::Over { background: transparent, foreground: masked })?;
    let output = graph.add(Operation::Over { background: matte, foreground: over })?;
    Ok((graph, source, output))
}

#[test]
fn opaque_and_zero_coverage_composites_reuse_inputs_while_partial_masks_preserve_blending()
-> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        for (alpha, expected) in [
            (1.0, Pixel::linear(0.5, 0.25, 0.125, 1.0)?),
            (0.5, Pixel::linear(0.3125, 0.25, 0.3125, 1.0)?),
            (0.0, Pixel::linear(0.125, 0.25, 0.5, 1.0)?),
        ] {
            let shape = VectorRectangle::new(
                [-1.0, -1.0, 129.0, 129.0],
                Pixel::linear(0.0, 0.0, 0.0, alpha)?,
            )?;
            let (graph, source, output) = composite(context, extent, shape)?;
            let mut evaluator = Evaluator::new(graph, OpaqueSource, 16, context.memory())?;
            let result = evaluator.region(output, extent.region(), context)?;
            assert!(result.pixels().iter().all(|pixel| *pixel == expected));
            if alpha == 1.0 {
                assert_eq!(evaluator.stats().computed_pixels, extent.pixels());
                let original = evaluator.region(source, extent.region(), context)?;
                assert!(std::sync::Arc::ptr_eq(&result, &original));
            } else if alpha == 0.0 {
                assert_eq!(evaluator.stats().source_pixels, 0);
                assert_eq!(evaluator.stats().computed_pixels, extent.pixels());
            }
        }
        Ok(())
    })
}

#[test]
fn fractional_vector_boundaries_keep_coverage_blending_instead_of_aliasing_unmasked_pixels()
-> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        let shape =
            VectorRectangle::new([0.5, 0.25, 32.0, 127.75], Pixel::linear(0.0, 0.0, 0.0, 1.0)?)?;
        let (graph, _, output) = composite(context, extent, shape)?;
        let mut evaluator = Evaluator::new(graph, OpaqueSource, 16, context.memory())?;
        let result = evaluator.region(output, extent.region(), context)?;
        assert_eq!(result.pixel(0, 0), Some(Pixel::linear(0.265625, 0.25, 0.359375, 1.0)?));
        assert_eq!(result.pixel(1, 0), Some(Pixel::linear(0.40625, 0.25, 0.21875, 1.0)?));
        assert_eq!(result.pixel(1, 1), Some(Pixel::linear(0.5, 0.25, 0.125, 1.0)?));
        assert_eq!(result.pixel(32, 64), Some(Pixel::linear(0.125, 0.25, 0.5, 1.0)?));
        Ok(())
    })
}
