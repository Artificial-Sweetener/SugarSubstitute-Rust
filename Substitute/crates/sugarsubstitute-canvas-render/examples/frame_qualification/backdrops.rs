// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure what acrylic regions add to one presented frame, by region count, on the selected GPU.
use std::{error::Error, sync::Arc, time::Instant};
use sugarsubstitute_canvas::{DamagePlan, FrameStamp, ViewProjection, Viewport};
use sugarsubstitute_canvas_render::backdrop::{
    BackdropPlan, BackdropRegion, GpuBackdrop, GpuBackdropPatch, MAX_BACKDROPS,
};
use sugarsubstitute_canvas_render::{CpuSurface, PreparedUpdate, ProjectionBuffer};
use sugarsubstitute_contracts::execution::{Generation, JobContext, SurfaceId};
use sugarsubstitute_image_gpu::{
    DisplayImage, DisplayMapping, DisplayProjection, GpuDevice, boundary_fixtures::OffscreenDisplay,
};
use sugarsubstitute_image_ops::{DisplaySample, Extent, Pixel};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Frames drawn per region count; the first compiles pipelines and is reported on its own.
const FRAMES: usize = 24;
/// Software frames painted per region count.
const SOFTWARE_FRAMES: usize = 6;
/// Blur radius the page asks for, in CSS pixels.
const SIGMA: f64 = 10.0;
const MATTE: [u8; 4] = [17, 29, 41, 255];
/// Region counts measured: none, the navigation bars, bars with the editing toolbars, the bound.
const COUNTS: [usize; 4] = [0, 2, 5, MAX_BACKDROPS];

/// The surfaces a canvas shows, laid out in CSS pixels of a 1920 by 1080 viewport: the two
/// navigation bars, then the input canvas's tool rail, contextual toolbar and brush options, then
/// toolbar-sized filler up to the count.
fn surfaces(count: usize, scale: f64) -> Result<Vec<BackdropRegion>> {
    const NAMED: [([f64; 4], f64); 5] = [
        ([660.0, 18.0, 600.0, 40.0], 12.0),
        ([660.0, 1022.0, 600.0, 40.0], 12.0),
        ([8.0, 52.0, 40.0, 260.0], 8.0),
        ([860.0, 700.0, 200.0, 36.0], 8.0),
        ([760.0, 70.0, 400.0, 44.0], 8.0),
    ];
    (0..count)
        .map(|index| {
            let (bounds, radius) = NAMED.get(index).copied().unwrap_or_else(|| {
                let cell = index - NAMED.len();
                let (column, row) = ((cell % 8) as f64, (cell / 8) as f64);
                ([150.0 + column * 205.0, 140.0 + row * 100.0, 120.0, 40.0], 8.0)
            });
            Ok(BackdropRegion::new(
                bounds.map(|value| value * scale),
                radius * scale,
                (SIGMA * scale) as f32,
            )?)
        })
        .collect()
}

/// Paint the same region counts over a software projection on this one worker, as a canvas
/// without a usable GPU does, and report the time the acrylic painting takes.
fn software(viewport: Extent, scale: f64, context: &JobContext) -> Result<usize> {
    let stamp = |generation| -> Result<FrameStamp> {
        Ok(FrameStamp { surface: SurfaceId::new(3)?, generation: Generation::new(generation)? })
    };
    let view = Viewport::new(viewport)?;
    let base = CpuSurface::from_prepared(PreparedUpdate::prepare(
        stamp(1)?,
        None,
        DamagePlan::full(viewport),
        context.memory(),
        |region, pixels| {
            for y in 0..region.height() {
                for x in 0..region.width() {
                    pixels[(y * region.width() + x) as usize] =
                        0xff00_0000 | ((x % 251) << 16) | ((y % 239) << 8) | ((x + y) % 233);
                }
            }
            Ok(())
        },
    )?)?;
    let matte = Pixel::linear(0.018, 0.018, 0.018, 1.0)?;
    let mut scratch = None;
    let mut generation = 1;
    let mut frames = 0;
    for count in COUNTS.into_iter().filter(|count| *count != 0 && *count != MAX_BACKDROPS) {
        let plan = BackdropPlan::new(viewport, &surfaces(count, scale)?)?;
        let mut paints = Vec::new();
        for frame in 0..SOFTWARE_FRAMES {
            let mut target = view;
            target.pan_by([3.0 + frame as f64, 0.0])?;
            let storage = match scratch.take() {
                Some(storage) => storage,
                None => ProjectionBuffer::new(viewport, context.memory(), context)?,
            };
            generation += 1;
            let mut shown = storage.project(
                stamp(generation)?,
                &base,
                ViewProjection::new(view, target)?,
                matte,
                context,
            )?;
            let started = Instant::now();
            shown.paint_backdrops(&plan, context)?;
            paints.push(started.elapsed().as_micros());
            scratch = Some(shown.into_buffer());
        }
        println!(
            "BACKDROPS_CPU: viewport={}x{} regions={} sigma={} paint_median_us={} paint_max_us={}",
            viewport.width(),
            viewport.height(),
            plan.regions().len(),
            SIGMA * scale,
            median(&mut paints),
            paints.last().copied().unwrap_or(0)
        );
        frames += SOFTWARE_FRAMES;
    }
    Ok(frames)
}

fn median(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples.get(samples.len() / 2).copied().unwrap_or(0)
}

/// Draw panning frames of one image through the production projection and patch pipelines at each
/// region count, and report the time acrylic preparation and the draw take.
pub(super) fn qualify(device: Arc<GpuDevice>, context: &JobContext) -> Result<usize> {
    let image_extent = Extent::new(2048, 1536)?;
    let _pixels = context.memory().claim(image_extent.pixels() * 4)?;
    let pixels: Vec<u32> = (0..image_extent.height())
        .flat_map(|y| {
            (0..image_extent.width()).map(move |x| {
                u32::from_be_bytes([255, (x % 251) as u8, (y % 239) as u8, ((x ^ y) % 256) as u8])
            })
        })
        .collect();
    let image = DisplayImage::from_pixels(device.clone(), image_extent, &pixels, context)?;
    let mut display = OffscreenDisplay::new(device.clone(), context)?;
    let mut frames = 0;
    for (viewport, scale) in [(Extent::new(1920, 1080)?, 1.0), (Extent::new(3840, 2160)?, 2.0)] {
        frames += software(viewport, scale, context)?;
        let step = f64::from(image_extent.width()) / f64::from(viewport.width());
        let _maps = context.memory().claim(
            (u64::from(viewport.width()) + u64::from(viewport.height()))
                * size_of::<DisplaySample>() as u64
                + 4096,
        )?;
        let mut unblurred = 0;
        for count in COUNTS {
            let plan = BackdropPlan::new(viewport, &surfaces(count, scale)?)?;
            let started = Instant::now();
            let mut backdrop = GpuBackdrop::new(device.clone(), context.memory().clone(), context)?;
            let pipeline_us = started.elapsed().as_micros();
            let (mut prepares, mut draws, mut totals) = (Vec::new(), Vec::new(), Vec::new());
            let mut first_us = 0;
            for frame in 0..FRAMES {
                let pan = frame as f64 * 3.375;
                let columns: Vec<_> = (0..viewport.width())
                    .map(|x| {
                        DisplaySample::from_corner(
                            f64::from(x) * step + pan,
                            image_extent.width(),
                            true,
                        )
                    })
                    .collect();
                let rows: Vec<_> = (0..viewport.height())
                    .map(|y| {
                        DisplaySample::from_corner(f64::from(y) * step, image_extent.height(), true)
                    })
                    .collect();
                let source = DisplayProjection {
                    image: &image,
                    mapping: Some(DisplayMapping::new(
                        image_extent,
                        viewport,
                        &columns,
                        &rows,
                        MATTE,
                    )?),
                    underlay: None,
                    overlay: &[],
                };
                let started = Instant::now();
                let patches = if plan.regions().is_empty() {
                    None
                } else {
                    Some(backdrop.prepare(&plan, source, context)?)
                };
                let prepare_us = started.elapsed().as_micros();
                let layers: Vec<_> = patches
                    .iter()
                    .flat_map(|patches| patches.patches())
                    .map(GpuBackdropPatch::layer)
                    .collect();
                let started = Instant::now();
                drop(display.render(source, &layers, context)?);
                let draw_us = started.elapsed().as_micros();
                if frame == 0 {
                    first_us = prepare_us + draw_us;
                } else {
                    prepares.push(prepare_us);
                    draws.push(draw_us);
                    totals.push(prepare_us + draw_us);
                }
            }
            let frame_us = median(&mut totals);
            if count == 0 {
                unblurred = frame_us;
            }
            println!(
                "BACKDROPS: viewport={}x{} regions={} sigma={} pipeline_us={pipeline_us} first_frame_us={first_us} prepare_median_us={} draw_median_us={} frame_median_us={frame_us} frame_max_us={} added_median_us={}",
                viewport.width(),
                viewport.height(),
                plan.regions().len(),
                SIGMA * scale,
                median(&mut prepares),
                median(&mut draws),
                totals.last().copied().unwrap_or(0),
                frame_us.saturating_sub(unblurred)
            );
            frames += FRAMES;
        }
    }
    Ok(frames)
}
