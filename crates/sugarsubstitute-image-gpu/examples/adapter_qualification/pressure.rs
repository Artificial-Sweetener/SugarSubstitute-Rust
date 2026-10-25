// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reproduce batch admission at a bounded halo copy while an unrelated cached tile is evictable.
use super::image::{ASSET, Pattern};
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_gpu::{GpuDevice, GpuEvaluator};
use sugarsubstitute_image_ops::{Evaluator, Extent, Graph, NodeId, Operation, Region, Sampling};

type Result<T = ()> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
// Reproduce the 4K shortfall: the halo fits, but its first 512-byte batch buffer does not.
const REMAINING_BYTES: u64 = 344;

fn image(context: &JobContext) -> Result<(Graph, NodeId, NodeId)> {
    let mut graph = Graph::new(2, context.memory())?;
    let extent = Extent::new(256, 128)?;
    let source = graph.add(Operation::Raster { asset: ASSET, extent })?;
    let output = graph.add(Operation::Resample {
        input: source,
        extent,
        sampling: Sampling::new([1.0, 1.0], [0.25, 0.25])?,
    })?;
    Ok((graph, source, output))
}

pub(super) fn qualify(
    device: Arc<GpuDevice>,
    memory: &MemoryLease,
    context: &JobContext,
) -> Result {
    let region = Region::new(16, 16, 32, 32)?;
    let (graph, source, output) = image(context)?;
    let dependency =
        graph.required_inputs(output, region)?[0].ok_or("Missing halo qualification dependency")?.1;
    let mut gpu = GpuEvaluator::new(graph, Pattern, 16, device, context.memory(), context)?;
    drop(gpu.tile(source, 0, 0, context)?);
    drop(gpu.tile(source, 1, 0, context)?);
    let before = gpu.work().batches;
    let available = match memory.claim(memory.bytes()) {
        Err(cause) => cause.available_bytes,
        Ok(_) => return Err("Native device lost its original allocation custody".into()),
    };
    let gather_bytes = dependency.pixels() * 16 + 256;
    let pressure_bytes = available
        .checked_sub(gather_bytes + REMAINING_BYTES)
        .ok_or("Insufficient qualification space for one pinned halo")?;
    let _pressure = memory.claim(pressure_bytes)?;
    let actual = gpu.region(output, region, context)?.read(context)?;
    if gpu.stats().source_pixels != 2 * 128 * 128 || gpu.work().batches <= before {
        return Err("Halo pressure lost its retained source or skipped native submission".into());
    }
    let (graph, _, output) = image(context)?;
    let mut cpu = Evaluator::new(graph, Pattern, 16, context.memory())?;
    let expected = cpu.region(output, region, context)?;
    for (actual, expected) in actual.pixels().iter().zip(expected.pixels()) {
        for (actual, expected) in actual.channels().into_iter().zip(expected.channels()) {
            if (actual - expected).abs() > 2e-5 * expected.abs().max(1.0) {
                return Err("Halo pressure changed GPU pixels or its pinned source".into());
            }
        }
    }
    Ok(())
}
