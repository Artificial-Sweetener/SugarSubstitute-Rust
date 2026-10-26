// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Projection agrees with independent camera composition and preserves canonical pixels and claims.
use super::*;
use crate::{
    CpuRenderer, PreparedUpdate,
    tests::{harness, run_test, stamp},
};
use sugarsubstitute_canvas::{
    DamagePlan, Document, DocumentEdit, Layer, LayerContent, LayerId, RenderRequest, Viewport,
};
use sugarsubstitute_image_ops::{AssetId, Extent, Filter, RasterSource, Region};

struct TestSource {
    extent: Extent,
}
impl RasterSource for TestSource {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32]) || region.intersection(self.extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Invalid projection test source request"));
        }
        output.fill(Pixel::linear(0.25, 0.5, 0.75, 1.0)?);
        Ok(())
    }
}

fn available(context: &JobContext) -> Result<u64, RenderError> {
    context
        .memory()
        .claim(u64::MAX)
        .err()
        .map(|cause| cause.available_bytes)
        .ok_or(RenderError::Invalid("Unbounded projection test claim was accepted"))
}

fn image_document(extent: Extent, context: &JobContext) -> Result<Document, RenderError> {
    let mut document = Document::new(1, extent, context.memory())?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(document)
}

/// Distinguish every retained source pixel while keeping presentation opaque.
fn color(x: u32, y: u32) -> u32 {
    0xff000000 | (y << 8) | x
}
/// Adopt one complete canonical fixture through the real prepared-frame boundary.
fn surface(context: &JobContext) -> Result<CpuSurface, RenderError> {
    let extent = Extent::new(31, 23)?;
    CpuSurface::from_prepared(PreparedUpdate::prepare(
        stamp(1)?,
        None,
        DamagePlan::full(extent),
        context.memory(),
        |region, pixels| {
            for y in 0..region.height() {
                for x in 0..region.width() {
                    pixels[(y * region.width() + x) as usize] = color(x, y);
                }
            }
            Ok(())
        },
    )?)
}

#[test]
fn projected_pixels_match_independent_camera_sampling_with_safe_matte_coverage()
-> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let source = Viewport::new(base.extent())?;
        let matte = Pixel::linear(0.018, 0.018, 0.018, 1.0)?;
        let encoded = SrgbEncoder::encode_constant(matte)?;
        let background = u32::from_be_bytes([255, encoded[0], encoded[1], encoded[2]]);
        for factor in [0.5, 0.8, 1.0, 1.25, 4.0, 1e9] {
            for dx in [-120.25, -7.0, -0.5, 0.0, 0.5, 7.0, 120.25] {
                for dy in [-51.0, -3.25, 0.0, 3.0, 80.0] {
                    let mut target = source;
                    target.zoom_at(factor, [13.125, 11.375])?;
                    target.pan_by([dx, dy])?;
                    target.resize(Extent::new(37, 29)?)?;
                    let mapping = ViewProjection::new(source, target)?;
                    let projection = ProjectionBuffer::new(mapping, context.memory(), context)?
                        .project(stamp(2)?, &base, mapping, matte, context)?;
                    for y in 0..29 {
                        for x in 0..37 {
                            let point = source.device_at(
                                target.document_at([f64::from(x) + 0.5, f64::from(y) + 0.5])?,
                            )?;
                            let expected = if point[0] >= 0.0
                                && point[0] < 31.0
                                && point[1] >= 0.0
                                && point[1] < 23.0
                            {
                                color(point[0].floor() as u32, point[1].floor() as u32)
                            } else {
                                background
                            };
                            assert_eq!(
                                projection.pixels()[(y * 37 + x) as usize],
                                expected,
                                "factor={factor} pan={dx},{dy} pixel={x},{y}"
                            );
                        }
                    }
                    assert_eq!(projection.source(), stamp(1)?);
                    assert_eq!(projection.target(), stamp(2)?);
                    assert_eq!(projection.view().target(), target);
                }
            }
        }
        assert_eq!(base.current(), Some(stamp(1)?));
        for y in 0..23 {
            for x in 0..31 {
                assert_eq!(base.pixels()[(y * 31 + x) as usize], color(x, y));
            }
        }
        Ok(())
    })
}

#[test]
fn projection_rejects_foreign_or_stale_frames_and_incomplete_matte() -> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let view = Viewport::new(base.extent())?;
        let map = ViewProjection::new(view, view)?;
        let black = Pixel::linear(0.0, 0.0, 0.0, 1.0)?;
        assert!(matches!(
            ProjectionBuffer::new(map, context.memory(), context)?.project(
                stamp(1)?,
                &base,
                map,
                black,
                context
            ),
            Err(RenderError::StaleTarget)
        ));
        let foreign = FrameStamp {
            surface: sugarsubstitute_contracts::execution::SurfaceId::new(2)
                .map_err(|_| RenderError::Invalid("Invalid test surface"))?,
            ..stamp(2)?
        };
        assert!(
            ProjectionBuffer::new(map, context.memory(), context)?
                .project(foreign, &base, map, black, context)
                .is_err()
        );
        assert!(
            ProjectionBuffer::new(map, context.memory(), context)?
                .project(stamp(2)?, &base, map, Pixel::TRANSPARENT, context)
                .is_err()
        );
        let empty = CpuSurface::new(stamp(1)?.surface, base.extent(), context.memory())?;
        assert!(matches!(
            ProjectionBuffer::new(map, context.memory(), context)?.project(
                stamp(2)?,
                &empty,
                map,
                black,
                context
            ),
            Err(RenderError::StaleBase)
        ));
        assert_eq!(base.current(), Some(stamp(1)?));
        Ok(())
    })
}

#[test]
fn projection_memory_pressure_does_not_replace_or_reallocate_canonical_source()
-> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let original = base.pixels().as_ptr();
        let view = Viewport::new(base.extent())?;
        let large = Viewport::new(Extent::new(16384, 16384)?)?;
        assert!(matches!(
            ProjectionBuffer::new(ViewProjection::new(view, large)?, context.memory(), context),
            Err(RenderError::Image(ImageError::Memory(_)))
        ));
        assert_eq!(base.pixels().as_ptr(), original);
        assert_eq!(base.current(), Some(stamp(1)?));
        let map = ViewProjection::new(view, view)?;
        let small = ProjectionBuffer::new(map, context.memory(), context)?.project(
            stamp(3)?,
            &base,
            map,
            Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
            context,
        )?;
        assert_eq!(small.pixels(), base.pixels());
        Ok(())
    })
}

#[test]
fn recycled_storage_keeps_allocation_and_resamples_only_canonical_pixels() -> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let source = Viewport::new(base.extent())?;
        let matte = Pixel::linear(0.0, 0.0, 0.0, 1.0)?;
        let identity = ViewProjection::new(source, source)?;
        let mut buffer = ProjectionBuffer::new(identity, context.memory(), context)?;
        let mut allocation = None;
        for (index, factor) in [0.5, 4.0, 1.0, 0.8, 1.0].into_iter().enumerate() {
            let mut target = source;
            target.zoom_at(factor, [7.125, 11.375])?;
            let map = ViewProjection::new(source, target)?;
            let frame = buffer.project(stamp(index as u64 + 2)?, &base, map, matte, context)?;
            if let Some(address) = allocation {
                assert_eq!(frame.pixels().as_ptr(), address);
            } else {
                allocation = Some(frame.pixels().as_ptr());
            }
            if factor == 1.0 {
                assert_eq!(frame.pixels(), base.pixels());
            }
            assert_eq!(frame.source(), stamp(1)?);
            buffer = frame.into_buffer();
            assert_eq!(buffer.extent(), base.extent());
        }
        let resized = Viewport::new(Extent::new(32, 23)?)?;
        assert!(
            buffer
                .project(stamp(8)?, &base, ViewProjection::new(source, resized)?, matte, context)
                .is_err()
        );
        assert_eq!(base.current(), Some(stamp(1)?));
        Ok(())
    })
}

#[test]
fn paired_projection_storage_owns_two_exact_claims_and_releases_them_together()
-> harness::TestResult {
    run_test(|context| {
        let source = Viewport::new(Extent::new(17, 13)?)?;
        let view = ViewProjection::new(source, source)?;
        let mut renderer =
            CpuRenderer::new(TestSource { extent: source.surface() }, 1, context.memory())?;
        let before = available(context)?;
        let pair = renderer.projection_buffers(view, context)?;
        let claim = source.surface().pixels() * 4 + size_of::<CpuProjection>() as u64 + 128;
        assert_eq!(available(context)?, before - 2 * claim);
        let (first, second) = pair.into_parts();
        assert_eq!(first.extent(), source.surface());
        assert_eq!(second.extent(), source.surface());
        drop((first, second));
        assert_eq!(available(context)?, before);
        let blocker = context.memory().claim(before - claim).map_err(ImageError::from)?;
        assert!(matches!(
            renderer.projection_buffers(view, context),
            Err(RenderError::Image(ImageError::Memory(cause)))
                if cause.requested_bytes == claim && cause.available_bytes == 0
        ));
        assert_eq!(available(context)?, claim);
        drop(blocker);
        assert_eq!(available(context)?, before);
        Ok(())
    })
}

#[test]
fn paired_projection_pressure_evicts_only_cache_tiles_and_preserves_canonical_pixels()
-> harness::TestResult {
    run_test(|context| {
        let extent = Extent::new(128, 128)?;
        let document = image_document(extent, context)?;
        let snapshot = document.snapshot();
        let source_view = Viewport::new(extent)?;
        let mut renderer = CpuRenderer::new(TestSource { extent }, 16, context.memory())?;
        let prepared = renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view: source_view,
                filter: Filter::Nearest,
                matte: Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(extent),
            },
            context,
        )?;
        let surface = CpuSurface::from_prepared(prepared)?;
        let original = surface.pixels().as_ptr();
        let expected = surface.pixels()[17 * extent.width() as usize + 19];
        let resident = renderer.stats().resident_tiles;
        assert!(resident > 0);

        let free = available(context)?;
        let overhead = size_of::<CpuProjection>() as u64 + 128;
        let pixels = ((free / 2 + 1).saturating_sub(overhead).div_ceil(4)).max(1);
        let width = pixels.min(512);
        let target_extent = Extent::new(
            u32::try_from(width)
                .map_err(|_| RenderError::Invalid("Projection test target is too wide"))?,
            u32::try_from(pixels.div_ceil(width))
                .map_err(|_| RenderError::Invalid("Projection test target is too tall"))?,
        )?;
        let target_view = Viewport::new(target_extent)?;
        let pair =
            renderer.projection_buffers(ViewProjection::new(source_view, target_view)?, context)?;
        assert!(renderer.stats().resident_tiles < resident);
        assert_eq!(surface.pixels().as_ptr(), original);
        assert_eq!(surface.pixels()[17 * extent.width() as usize + 19], expected);
        assert_eq!(surface.current(), Some(stamp(1)?));
        let (first, second) = pair.into_parts();
        assert_eq!(first.extent(), target_extent);
        assert_eq!(second.extent(), target_extent);
        assert_ne!(first.pixels.as_ptr(), second.pixels.as_ptr());
        Ok(())
    })
}
