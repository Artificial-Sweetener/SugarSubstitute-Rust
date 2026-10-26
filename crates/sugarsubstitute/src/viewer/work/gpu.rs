// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare immutable native GPU candidates and proof-only readbacks on execution-owned workers.
use super::*;
use std::time::Instant;
use sugarsubstitute_canvas::DamagePlan;

pub(super) fn prepare(
    input: &mut Prepare,
    engine: &mut Engine,
    started: Instant,
    context: &JobContext,
) -> Result<Packet, WorkError> {
    let device =
        engine.device.as_ref().ok_or_else(|| WorkError::invalid("GPU device is unavailable"))?;
    let mut surface = match input.gpu_surface.take() {
        Some(surface) if surface.extent() == input.view.surface() => surface,
        _ => GpuSurface::new(input.target.surface, input.view.surface(), device.clone())?,
    };
    let Some(Renderer::Gpu(renderer)) = engine.renderer.as_mut() else {
        return Err(WorkError::invalid("GPU renderer is unavailable"));
    };
    let update = renderer.prepare(
        RenderRequest {
            document: &input.document,
            view: input.view,
            reconstruction: input.reconstruction,
            matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
            target: input.target,
            base: input.base,
            damage: input.damage,
        },
        context,
    )?;
    let work = renderer.retain(&mut surface, &update, context)?;
    drop(update);
    let frame = surface.frame().ok_or_else(|| WorkError::invalid("GPU viewport is unavailable"))?;
    let snapshot = if input.resources.capture {
        let prepared = PreparedUpdate::prepare(
            input.target,
            None,
            DamagePlan::full(input.view.surface()),
            context.memory(),
            |region, pixels| {
                for tile in region.tiles() {
                    let source = frame.image().read_region(tile, context)?;
                    for row in 0..tile.height() as usize {
                        let from = row * tile.width() as usize;
                        let to = (tile.y() as usize - region.y() as usize + row)
                            * region.width() as usize
                            + (tile.x() - region.x()) as usize;
                        let width = tile.width() as usize;
                        pixels[to..to + width]
                            .copy_from_slice(&source.pixels()[from..from + width]);
                    }
                }
                Ok(())
            },
        )?;
        Some(CpuSurface::from_prepared(prepared)?)
    } else {
        None
    };
    Ok(Packet {
        tier: input.tier,
        reconstruction: input.reconstruction,
        display_buffers: None,
        pixels: Pixels::Gpu(GpuPacket { surface, snapshot, work }),
        view: input.view,
        document: (input.document.identity(), input.document.revision()),
        elapsed: started.elapsed(),
        stats: renderer.stats(),
        source_revision: input.source_revision,
    })
}
