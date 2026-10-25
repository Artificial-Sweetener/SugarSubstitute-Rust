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
    if input.cpu.is_none() {
        input.cpu = Some(renderer.begin(
            RenderRequest {
                document: &input.document,
                view: input.view,
                filter: input.filter,
                matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                target: input.target,
                base: input.base,
                damage: input.damage,
            },
            context,
        )?);
        return Ok(CpuStep::Yield);
    }
    let preparation = input
        .cpu
        .as_mut()
        .ok_or_else(|| WorkError::invalid("CPU preparation state is unavailable"))?;
    let Some(update) = renderer.step(preparation, context)? else {
        return Ok(CpuStep::Yield);
    };
    input.finished = true;
    drop(input.cpu.take());
    if started.elapsed() >= std::time::Duration::from_secs(1) {
        let stats = renderer.stats();
        tracing::info!(operation="canvas_frame_preparation", document=input.document.identity(), revision=input.document.revision(), generation=?input.target.generation, elapsed_milliseconds=started.elapsed().as_millis(), computed_pixels=stats.computed_pixels, source_pixels=stats.source_pixels, "Canvas DAG preparation completed");
    }
    let pixels = if input.replace_surface {
        CpuPacket::Replace(CpuSurface::from_prepared(update)?)
    } else {
        CpuPacket::Repair(Box::new(update))
    };
    Ok(CpuStep::Complete(Packet {
        pixels: Pixels::Cpu(pixels),
        damage: input.damage,
        base: input.base,
        view: input.view,
        filter: input.filter,
        document: (input.document.identity(), input.document.revision()),
        elapsed: started.elapsed(),
        stats: renderer.stats(),
        source_revision: input.source_revision,
    }))
}
