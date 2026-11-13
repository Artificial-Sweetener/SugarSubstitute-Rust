// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify bounded dynamic device materials and representative pan-refresh timings headlessly.
use super::Result;
use std::{sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_gpu::{
    DisplayImage, DisplayMapping, DisplayPatch, DisplayProjection, DisplayProjector, GpuDevice,
    GpuEvaluator, GpuRasterBinding, OverlayPixel, boundary_fixtures::OffscreenDisplay,
};
use sugarsubstitute_image_ops::*;
#[path = "backdrop/reference.rs"]
mod reference;
use reference::{Reference, Source};

pub(super) fn qualify(
    device: Arc<GpuDevice>,
    foreign: Arc<GpuDevice>,
    context: &JobContext,
) -> Result {
    let primary_extent = Extent::new(1024, 512)?;
    let fallback_extent = Extent::new(32, 24)?;
    let _pixels =
        context.memory().claim((primary_extent.pixels() + fallback_extent.pixels()) * 4)?;
    let primary_pixels = reference::pixels(primary_extent, 7);
    let fallback_pixels = reference::pixels(fallback_extent, 93);
    let primary =
        DisplayImage::from_pixels(device.clone(), primary_extent, &primary_pixels, context)?;
    let fallback =
        DisplayImage::from_pixels(device.clone(), fallback_extent, &fallback_pixels, context)?;
    let started = Instant::now();
    let mut projector = DisplayProjector::new(device.clone(), context)?;
    let mut display = OffscreenDisplay::new(device.clone(), context)?;
    println!("BACKDROP: pipeline_init_us={}", started.elapsed().as_micros());
    let mut evaluator: Option<GpuEvaluator<()>> = None;
    for (frame, viewport) in [
        (0, Extent::new(1920, 1080)?),
        (1, Extent::new(1920, 1080)?),
        (2, Extent::new(1921, 1081)?),
    ] {
        let pan = f64::from(frame) * 11.375;
        let _maps = context.memory().claim(
            (u64::from(viewport.width()) + u64::from(viewport.height()))
                * 4
                * size_of::<DisplaySample>() as u64
                + 4096,
        )?;
        let columns: Vec<_> = (0..viewport.width())
            .map(|x| {
                DisplaySample::from_corner(f64::from(x) - 45.25 + pan, primary_extent.width(), true)
            })
            .collect();
        let rows: Vec<_> = (0..viewport.height())
            .map(|y| {
                DisplaySample::from_corner(f64::from(y) - 600.125, primary_extent.height(), true)
            })
            .collect();
        let fallback_columns: Vec<_> = (0..viewport.width())
            .map(|x| {
                if x % 97 == 0 {
                    DisplaySample::UNCOVERED
                } else {
                    DisplaySample::from_corner(
                        f64::from(x) * 32.0 / f64::from(viewport.width()) + 0.25,
                        fallback_extent.width(),
                        true,
                    )
                }
            })
            .collect();
        let fallback_rows: Vec<_> = (0..viewport.height())
            .map(|y| {
                DisplaySample::from_corner(
                    f64::from(y) * 24.0 / f64::from(viewport.height()),
                    fallback_extent.height(),
                    true,
                )
            })
            .collect();
        let overlay = [
            OverlayPixel { x: 5, y: 5, color: [255, 3, 177] },
            OverlayPixel { x: 760, y: 1050, color: [3, 255, 177] },
        ];
        let matte = [17, 29, 41, 255];
        let source = DisplayProjection {
            image: &primary,
            mapping: Some(DisplayMapping::new(primary_extent, viewport, &columns, &rows, matte)?),
            underlay: Some((
                &fallback,
                DisplayMapping::new(
                    fallback_extent,
                    viewport,
                    &fallback_columns,
                    &fallback_rows,
                    matte,
                )?,
            )),
            overlay: &overlay,
        };
        let reference = Reference {
            primary: &primary_pixels,
            primary_extent,
            columns: &columns,
            rows: &rows,
            fallback: &fallback_pixels,
            fallback_extent,
            fallback_columns: &fallback_columns,
            fallback_rows: &fallback_rows,
            overlay: &overlay,
            matte: u32::from_be_bytes([255, 17, 29, 41]),
        };
        let crop = Region::new(588, 968, 744, viewport.height() - 968)?;
        let inner = Region::new(72, 72, 601, viewport.height().min(1081) - 1040)?;
        let destination = [crop.x() + inner.x(), crop.y() + inner.y()];
        let bounds = [660.25, 1040.25, 600.0, 40.0];
        let asset = AssetId([frame as u8 + 31; 32]);
        let started = Instant::now();
        let binding = projector.project_raster(asset, crop, source, context.memory(), context)?;
        let project_us = started.elapsed().as_micros();
        let (graph, node) = reference::graph(asset, binding.extent(), context.memory())?;
        let capacity = GpuEvaluator::raster_tile_capacity(&graph, node)?;
        let started = Instant::now();
        if let Some(evaluator) = &mut evaluator {
            evaluator.reserve_raster_tiles(capacity, context.memory())?;
            evaluator.replace_raster(graph, node, binding)?;
        } else {
            evaluator = Some(GpuEvaluator::from_raster(
                graph,
                node,
                binding,
                capacity,
                device.clone(),
                context.memory(),
                context,
            )?);
        }
        let gpu = evaluator.as_mut().ok_or("Device-only evaluator missing")?;
        let work_before = gpu.work();
        let _regions = context.memory().claim(inner.tile_count() * size_of::<Region>() as u64)?;
        let regions: Vec<_> = inner.tiles().collect();
        let prepared = gpu.display_regions(node, &regions, context)?;
        let filtered =
            DisplayImage::compose_region(device.clone(), inner, prepared.tiles(), context)?;
        if gpu.stats().source_pixels != 0 {
            return Err("Device mode unexpectedly read host source pixels".into());
        }
        let gaussian_us = started.elapsed().as_micros();
        let work = gpu.work();
        println!(
            "BACKDROP_DAG: frame={frame} capacity={capacity} batches={} dispatches={} copies={} uploaded_bytes={}",
            work.batches - work_before.batches,
            work.dispatches - work_before.dispatches,
            work.dependency_copies - work_before.dependency_copies,
            work.uploaded_bytes - work_before.uploaded_bytes
        );
        let started = Instant::now();
        let original = projector.project_image(
            Region::new(destination[0], destination[1], inner.width(), inner.height())?,
            source,
            context,
        )?;
        let _mask = context.memory().claim(inner.pixels() + 128)?;
        let coverage: Vec<_> = (0..inner.height())
            .flat_map(|y| {
                (0..inner.width()).map(move |x| {
                    reference::coverage(bounds, 9.25, destination[0] + x, destination[1] + y)
                })
            })
            .collect();
        let layer = DisplayPatch {
            original: &original,
            filtered: &filtered,
            destination,
            coverage: &coverage,
        };
        let actual = display.render(source, &[layer], context)?;
        let material_us = started.elapsed().as_micros();
        let started = Instant::now();
        compare(&actual, &reference, viewport, inner, destination, &coverage, context)?;
        println!(
            "BACKDROP: viewport={}x{} bar=600x40 sigma=10 frame={frame} pan={pan} crop={}x{} project_us={project_us} gaussian_us={gaussian_us} material_us={material_us} native_total_us={} parity_us={} full_viewport_cpu_f32=false device_only=true outside_unchanged=true",
            viewport.width(),
            viewport.height(),
            crop.width(),
            crop.height(),
            project_us + gaussian_us + material_us,
            started.elapsed().as_micros()
        );
        if frame == 2 {
            errors(&mut projector, source, crop, device.clone(), foreign.clone(), context)?;
        }
    }
    Ok(())
}

fn compare(
    actual: &DisplayImage,
    reference: &Reference<'_>,
    viewport: Extent,
    inner: Region,
    destination: [u32; 2],
    coverage: &[u8],
    context: &JobContext,
) -> Result {
    let asset = AssetId([91; 32]);
    let (graph, node) = reference::graph(asset, viewport, context.memory())?;
    let mut cpu = Evaluator::new(graph, Source { asset, reference }, 2, context.memory())?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let destination_region =
        Region::new(destination[0], destination[1], inner.width(), inner.height())?;
    for region in viewport.region().tiles() {
        let read = actual.read_region(region, context)?;
        let blur = if region.intersection(destination_region).is_some() {
            Some(cpu.region(node, region, context)?)
        } else {
            None
        };
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                let pixel =
                    read.pixels()[((y - region.y()) * region.width() + x - region.x()) as usize];
                let base = reference.encoded(x, y);
                let mut count = 0;
                let expected = if destination_region.contains(x, y) {
                    count = coverage
                        [((y - destination[1]) * inner.width() + x - destination[0]) as usize];
                    if count == 0 {
                        base
                    } else {
                        let image = blur.as_ref().ok_or("CPU reference tile missing")?;
                        let p = image.pixels()
                            [((y - region.y()) * region.width() + x - region.x()) as usize];
                        let [r, g, b, a] = encoder.encode(p)?;
                        reference::mixed(base, u32::from_be_bytes([a, r, g, b]), count)
                    }
                } else {
                    base
                };
                for (a, b) in pixel.to_be_bytes().into_iter().zip(expected.to_be_bytes()) {
                    if a.abs_diff(b) > u8::from(count != 0) {
                        return Err(format!("Backdrop frame differs at{x},{y}: actual={pixel:#010x} expected={expected:#010x} mask={count}").into());
                    }
                }
            }
        }
    }
    Ok(())
}
fn errors(
    projector: &mut DisplayProjector,
    source: DisplayProjection<'_>,
    crop: Region,
    device: Arc<GpuDevice>,
    foreign: Arc<GpuDevice>,
    context: &JobContext,
) -> Result {
    let asset = AssetId([111; 32]);
    let binding = projector.project_raster(asset, crop, source, context.memory(), context)?;
    let (graph, node) = reference::graph(asset, binding.extent(), context.memory())?;
    if GpuEvaluator::from_raster(graph, node, binding, 2, foreign, context.memory(), context)
        .is_ok()
    {
        return Err("Device-only raster accepted a foreign GPU".into());
    }
    let binding = projector.project_raster(asset, crop, source, context.memory(), context)?;
    let (graph, node) = reference::graph(asset, binding.extent(), context.memory())?;
    let mut tiny = GpuEvaluator::from_raster(
        graph,
        node,
        binding,
        2,
        device.clone(),
        context.memory(),
        context,
    )?;
    let sample = Region::new(72, 72, 64, 32)?;
    drop(tiny.display_regions(node, &[sample], context)?);
    while tiny.evict_one() {}
    drop(tiny.region(node, sample, context)?);
    if tiny.stats().source_pixels != 0 {
        return Err("Tiny device-source cache eviction entered host reads".into());
    }
    let bad =
        Region::new(0, 0, source.mapping.ok_or("Projection missing")?.output().width() + 1, 1)?;
    if projector.project_raster(asset, bad, source, context.memory(), context).is_ok() {
        return Err("Device crop accepted outside output".into());
    }
    let tile = Region::new(0, 0, 128, 1)?;
    let _pixels = context.memory().claim(tile.pixels() * 16)?;
    let pixels = vec![Pixel::TRANSPARENT; tile.pixels() as usize];
    let native = device.upload(tile, &pixels, context)?;
    if GpuRasterBinding::from_tiles(
        asset,
        Extent::new(129, 1)?,
        vec![native],
        context.memory(),
        context,
    )
    .is_ok()
    {
        return Err("Device binding accepted incomplete source".into());
    }
    println!(
        "BACKDROP: foreign_device_rejected=true incomplete_source_rejected=true outside_crop_rejected=true tiny_cache_eviction_device_only=true"
    );
    Ok(())
}
