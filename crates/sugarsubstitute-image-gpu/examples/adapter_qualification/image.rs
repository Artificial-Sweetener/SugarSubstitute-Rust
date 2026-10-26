// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare actual hardware evaluation with CPU DAG semantics, including hidden failing branches.
use std::{error::Error, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_gpu::{GpuDevice, GpuEvaluator};
use sugarsubstitute_image_ops::*;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub(super) const ASSET: AssetId = AssetId([17; 32]);
pub(super) struct Pattern;
impl RasterSource for Pattern {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        if asset != ASSET {
            return Err(ImageError::Invalid("Hidden unavailable qualification source"));
        }
        for y in 0..region.height() {
            for x in 0..region.width() {
                let gx = region.x() + x;
                let gy = region.y() + y;
                let alpha = ((gx % 7 + gy % 5) as f32) / 10.0;
                output[(y * region.width() + x) as usize] = Pixel::linear(
                    (gx % 19) as f32 * 0.25 - 1.25,
                    (gy % 23) as f32 / 22.0,
                    0.25,
                    alpha,
                )?;
            }
        }
        Ok(())
    }
}
struct Case {
    name: &'static str,
    node: NodeId,
    region: Region,
}
fn graph(context: &JobContext) -> Result<(Graph, Vec<Case>, [NodeId; 3])> {
    let mut graph = Graph::new(128, context.memory())?;
    let extent = Extent::new(257, 131)?;
    let source = graph.add(Operation::Raster { asset: ASSET, extent })?;
    let color = Pixel::linear(-0.5, 4.0, 0.25, 0.75)?;
    let solid = graph.add(Operation::Solid { extent, color })?;
    let vector = graph.add(Operation::Rectangle {
        extent,
        shape: VectorRectangle::new([126.375, 1.25, 256.75, 129.125], color)?,
        translation: [0; 2],
    })?;
    let exposure = graph.add(Operation::Exposure { input: source, stops: 1.25 })?;
    let over = graph.add(Operation::Over { background: source, foreground: vector })?;
    let masked = graph.add(Operation::Mask { input: over, mask: vector })?;
    let mix = graph.add(Operation::Mix {
        original: source,
        adjusted: exposure,
        mask: vector,
        opacity: 0.375,
    })?;
    let hidden = graph.add(Operation::Raster { asset: AssetId([99; 32]), extent })?;
    let zero = graph.add(Operation::Solid { extent, color: Pixel::TRANSPARENT })?;
    let zero_mask = graph.add(Operation::Mask { input: hidden, mask: zero })?;
    let zero_mix = graph.add(Operation::Mix {
        original: source,
        adjusted: hidden,
        mask: zero,
        opacity: 1.0,
    })?;
    let zero_opacity = graph.add(Operation::Mix {
        original: source,
        adjusted: hidden,
        mask: vector,
        opacity: 0.0,
    })?;
    let target = Extent::new(260, 129)?;
    let sampling = Sampling::new([0.9, 1.1], [-0.3, 2.2])?;
    let lanczos = graph.add(Operation::Resample { input: mix, extent: target, sampling })?;
    let nearest = graph.add(Operation::Resample {
        input: mix,
        extent: target,
        sampling: sampling.with_filter(Filter::Nearest),
    })?;
    let pyramid = graph.pyramid(masked)?.levels()[3];
    let sparse = graph.add(Operation::Resample {
        input: source,
        extent: target,
        sampling: Sampling::new([15.5, 9.25], [-100.5, -20.25])?.with_filter(Filter::Nearest),
    })?;
    let near = Region::new(125, 2, 128, 127)?;
    let mut cases = Vec::new();
    cases.try_reserve_exact(24)?;
    for (name, node) in [
        ("source", source),
        ("solid_hdr", solid),
        ("vector_coverage", vector),
        ("exposure", exposure),
        ("over", over),
        ("mask", masked),
        ("adjustment_mix", mix),
        ("hidden_zero_mask", zero_mask),
        ("hidden_zero_mix", zero_mix),
        ("hidden_zero_opacity", zero_opacity),
    ] {
        cases.push(Case { name, node, region: near });
    }
    cases.push(Case {
        name: "nearest_sparse_source",
        node: sparse,
        region: Region::new(0, 0, 128, 127)?,
    });
    let translated_lanczos = graph.add(Operation::Resample {
        input: mix,
        extent: target,
        sampling: sampling.with_output_translation([13, -7]),
    })?;
    let translated_sparse = graph.add(Operation::Resample {
        input: source,
        extent: target,
        sampling: Sampling::new([15.5, 9.25], [-100.5, -20.25])?
            .with_filter(Filter::Nearest)
            .with_output_translation([-13, 7]),
    })?;
    let translated_vector = graph.add(Operation::Rectangle {
        extent,
        shape: VectorRectangle::new([126.375, 1.25, 256.75, 129.125], color)?,
        translation: [13, -7],
    })?;
    for (name, node, region) in [
        ("translated_lanczos", translated_lanczos, near),
        ("translated_sparse_nearest", translated_sparse, Region::new(1, 1, 128, 127)?),
        ("translated_vector", translated_vector, near),
    ] {
        cases.push(Case { name, node, region });
    }
    cases.push(Case {
        name: "pyramid_odd_edges",
        node: pyramid,
        region: Extent::new(33, 17)?.region(),
    });
    let reduced_extent = graph.extent(pyramid)?;
    let nearest_extent = Extent::new(20, 10)?;
    let nearest_reconstruction = graph.add(Operation::Resample {
        input: pyramid,
        extent: nearest_extent,
        sampling: Sampling::resize(reduced_extent, nearest_extent).with_filter(Filter::Nearest),
    })?;
    cases.push(Case {
        name: "lanczos_pyramid_nearest_reconstruction",
        node: nearest_reconstruction,
        region: nearest_extent.region(),
    });
    for (name, node) in [("lanczos_global_phase", lanczos), ("nearest_global_phase", nearest)] {
        cases.push(Case { name, node, region: Region::new(128, 1, 128, 127)? });
        cases.push(Case { name, node, region: Region::new(0, 0, 128, 1)? });
    }
    let distant_extent = Extent::new(100_000_001, 101)?;
    let distant = graph.add(Operation::Rectangle {
        extent: distant_extent,
        shape: VectorRectangle::new([99_999_872.375, -1.25, 100_000_000.75, 99.125], color)?,
        translation: [0; 2],
    })?;
    cases.push(Case {
        name: "distant_vector_precision",
        node: distant,
        region: Region::new(99_999_873, 1, 128, 99)?,
    });
    let boundary = graph.add(Operation::Rectangle {
        extent: Extent::new(i32::MAX as u32, 128)?,
        shape: VectorRectangle::new(
            [f64::from(u32::MAX) - 1.5, 0.25, f64::from(u32::MAX) + 0.75, 127.5],
            color,
        )?,
        translation: [i32::MIN, 0],
    })?;
    cases.push(Case {
        name: "last_column_vector_coverage",
        node: boundary,
        region: Region::new(i32::MAX as u32 - 1, 0, 1, 128)?,
    });
    let singleton = graph.add(Operation::Raster { asset: ASSET, extent: Extent::new(1, 128)? })?;
    for (name, filter) in [
        ("singleton_extreme_lanczos", Filter::Lanczos3),
        ("singleton_extreme_nearest", Filter::Nearest),
    ] {
        let extent = Extent::new(128, 128)?;
        let node = graph.add(Operation::Resample {
            input: singleton,
            extent,
            sampling: Sampling::new([1e100, 1.0], [1e100, 0.0])?.with_filter(filter),
        })?;
        cases.push(Case { name, node, region: extent.region() });
    }
    let bright =
        graph.add(Operation::Solid { extent, color: Pixel::linear(f32::MAX, 0.0, 0.0, 1.0)? })?;
    let overflow = graph.add(Operation::Exposure { input: bright, stops: 32.0 })?;
    let tiny = Pixel::premultiplied([0.25, 0.0, 0.0, f32::from_bits(1)])?;
    let one = Extent::new(1, 1)?;
    let input = graph.add(Operation::Solid { extent: one, color: tiny })?;
    let mask =
        graph.add(Operation::Solid { extent: one, color: Pixel::linear(0.0, 0.0, 0.0, 0.25)? })?;
    let underflow_mask = graph.add(Operation::Mask { input, mask })?;
    let underflow_vector = graph.add(Operation::Rectangle {
        extent: one,
        shape: VectorRectangle::new([0.75, 0.0, 1.0, 1.0], tiny)?,
        translation: [0; 2],
    })?;
    Ok((graph, cases, [overflow, underflow_mask, underflow_vector]))
}
pub(super) fn qualify(device: Arc<GpuDevice>, context: &JobContext) -> Result<usize> {
    let _inventory = context.memory().claim(256 * 1024)?;
    let (cpu_graph, cpu_cases, cpu_invalid) = graph(context)?;
    let (gpu_graph, gpu_cases, gpu_invalid) = graph(context)?;
    let mut cpu = Evaluator::new(cpu_graph, Pattern, 32, context.memory())?;
    let mut gpu = GpuEvaluator::new(gpu_graph, Pattern, 32, device, context.memory(), context)?;
    for (case, native) in cpu_cases.iter().zip(&gpu_cases) {
        let started = Instant::now();
        let expected = cpu.region(case.node, case.region, context)?;
        let cpu_us = started.elapsed().as_micros();
        let started = Instant::now();
        let tile = gpu.region(native.node, native.region, context)?;
        let gpu_us = started.elapsed().as_micros();
        let started = Instant::now();
        let actual = tile.read(context)?;
        let readback_us = started.elapsed().as_micros();
        if actual.region() != expected.region() {
            return Err("GPU changed the requested output region".into());
        }
        let mut worst = 0.0_f32;
        for (index, (actual, expected)) in actual.pixels().iter().zip(expected.pixels()).enumerate()
        {
            for (channel, (actual, expected)) in
                actual.channels().into_iter().zip(expected.channels()).enumerate()
            {
                let difference = (actual - expected).abs();
                worst = worst.max(difference);
                if difference > 2e-5 * expected.abs().max(1.0) {
                    return Err(format!("GPU {} differs at pixel {index}, channel {channel}: actual={actual}, expected={expected}", case.name).into());
                }
            }
        }
        let before = gpu.stats();
        let started = Instant::now();
        drop(gpu.region(native.node, native.region, context)?);
        let reuse_us = started.elapsed().as_micros();
        println!(
            "DAG: case={} pixels={} maximum_absolute_error={worst:e} cpu_us={cpu_us} gpu_us={gpu_us} readback_us={readback_us} reuse_us={reuse_us}",
            case.name,
            case.region.pixels()
        );
        if gpu.stats().computed_tiles != before.computed_tiles
            || gpu.stats().reused_tiles <= before.reused_tiles
        {
            return Err("GPU did not reuse an identical completed region".into());
        }
    }
    for (cpu_node, gpu_node) in cpu_invalid.into_iter().zip(gpu_invalid) {
        if cpu.region(cpu_node, Region::new(0, 0, 1, 1)?, context).is_ok()
            || gpu.region(gpu_node, Region::new(0, 0, 1, 1)?, context).is_ok()
        {
            return Err("Image evaluation accepted invalid canonical output".into());
        }
    }
    println!(
        "DAG: invalid_canonical_outputs_rejected=3 stats={:?} work={:?}",
        gpu.stats(),
        gpu.work()
    );
    let retained = gpu.region(gpu_cases[0].node, gpu_cases[0].region, context)?;
    let expected = cpu.region(cpu_cases[0].node, cpu_cases[0].region, context)?;
    let before = gpu.stats();
    let (replacement, replacement_cases, _) = graph(context)?;
    gpu.replace_graph(replacement);
    drop(gpu.region(replacement_cases[0].node, replacement_cases[0].region, context)?);
    if gpu.stats().computed_tiles != before.computed_tiles {
        return Err("GPU discarded compatible content when graph identities changed".into());
    }
    gpu.clear_tiles();
    let alias_case = replacement_cases
        .iter()
        .find(|case| case.name == "hidden_zero_mix")
        .ok_or("Missing alias retention case")?;
    let original_case = cpu_cases
        .iter()
        .find(|case| case.name == "hidden_zero_mix")
        .ok_or("Missing CPU alias retention case")?;
    let alias = gpu.region(alias_case.node, alias_case.region, context)?;
    let alias_expected = cpu.region(original_case.node, original_case.region, context)?;
    while gpu.evict_one() {}
    if gpu.stats().resident_tiles != 0 {
        return Err("Cache-only GPU aliases prevented reconstructible tile eviction".into());
    }
    drop(gpu);
    if retained.read(context)?.pixels() != expected.pixels() {
        return Err("Retained GPU pixels changed after evaluator retirement".into());
    }
    if alias.read(context)?.pixels() != alias_expected.pixels() {
        return Err("GPU cache eviction changed outside alias pixels".into());
    }
    println!("DAG: aliased_cache_references_evicted=true outside_allocation_preserved=true");
    println!("DAG: compatible_graph_reuse=true retained_result_survives_evaluator=true");
    Ok(cpu_cases.len())
}
