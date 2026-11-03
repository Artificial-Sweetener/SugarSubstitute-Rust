// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance one original CPU request through admitted, unpublished viewport-tile steps.
use super::*;
use std::time::Instant;

pub(super) fn step(
    input: &mut Prepare,
    renderer: &mut CpuRenderer<SourcePort>,
    started: Instant,
    context: &JobContext,
) -> Result<CpuStep<Packet>, WorkError> {
    if input.replace_buffers && input.display_buffers.is_none() {
        input.display_buffers = Some(renderer.projection_buffers(input.display_extent, context)?);
        return Ok(CpuStep::Yield);
    }
    if input.cpu.is_none() {
        let request = RenderRequest {
            document: &input.document,
            comparison: input.comparison,
            view: input.view,
            reconstruction: input.reconstruction,
            matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
            target: input.target,
            base: input.base,
            damage: input.damage,
        };
        input.cpu = Some(if input.tier == BackingTier::Navigation {
            renderer.begin_navigation(request, context)?
        } else {
            renderer.begin(request, context)?
        });
        return Ok(CpuStep::Yield);
    }
    let preparation = input
        .cpu
        .as_mut()
        .ok_or_else(|| WorkError::invalid("CPU preparation state is unavailable"))?;
    let Some(update) = renderer.step(preparation, context)? else {
        return Ok(CpuStep::Yield);
    };
    let sampling_scale = preparation.sampling_scale();
    input.finished = true;
    drop(input.cpu.take());
    if started.elapsed() >= std::time::Duration::from_secs(1) {
        let stats = renderer.stats();
        tracing::info!(operation="canvas_frame_preparation", document=input.document.identity(), revision=input.document.revision(), generation=?input.target.generation, elapsed_milliseconds=started.elapsed().as_millis(), computed_pixels=stats.computed_pixels, source_pixels=stats.source_pixels, "Canvas DAG preparation completed");
    }
    let pixels = if input.replace_surface {
        CpuPacket::Replace { surface: CpuSurface::from_prepared(update)? }
    } else {
        CpuPacket::Repair(Box::new(update))
    };
    Ok(CpuStep::Complete(Packet {
        sampling_scale,
        tier: input.tier,
        reconstruction: input.reconstruction,
        display_buffers: input.display_buffers.take(),
        pixels: Pixels::Cpu(pixels),
        view: input.view,
        document: input.key,
        elapsed: started.elapsed(),
        stats: renderer.stats(),
        source_revision: input.source_revision,
        source_io: input.source_io()?,
    }))
}
