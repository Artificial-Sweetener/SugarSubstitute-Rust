// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Time the brush's input path on a 4096 pixel mask: each pointer sample composites its dabs,
//! publishes the touched tiles and stages the preview, exactly as a live stroke does.
//!
//! Run with `cargo run --release --locked --example brush_dab_timing --package
//! sugarsubstitute-canvas`. It reports timings only and asserts nothing about them.
use std::time::{Duration, Instant};
use sugarsubstitute_canvas::{
    BrushMode, BrushOptions, BrushTool, CanvasError, Document, DocumentEdit, Layer, LayerContent,
    LayerId, MaskAppearance, Navigation, PointerSample, ToolButton, ToolId, ToolInput, ToolManager,
    ToolModifiers, Viewport,
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use sugarsubstitute_image_ops::{Coverage, Extent};

const EDGE: u32 = 4096;
const SAMPLES: usize = 24;

/// The example owns its whole process, so admission only needs to account, not to bound.
struct Unbounded;
impl MemoryRelease for Unbounded {
    fn release(&self, _: u64) {}
}

struct Timing {
    first: Duration,
    mean: Duration,
    worst: Duration,
    commit: Duration,
}

/// Paint one stroke of `SAMPLES` pointer samples, each `step` document pixels apart.
fn stroke(
    memory: &MemoryLease,
    size: u32,
    hardness: f64,
    step: f64,
) -> Result<Timing, CanvasError> {
    let extent = Extent::new(EDGE, EDGE)?;
    let mut navigation = Navigation::new(Viewport::new(extent)?);
    navigation.set_content(Some(extent), Duration::ZERO)?;
    let mut tools = ToolManager::new(navigation);
    let brush = ToolId::new(2)?;
    let options = BrushOptions::new(size, hardness, 1.0)?;
    tools.register(brush, BrushTool::new(BrushMode::Paint, options), memory)?;
    let mut document = Document::new(1, extent, memory)?;
    let content = LayerContent::Coverage {
        coverage: Coverage::new(extent, 0, memory)?,
        appearance: MaskAppearance::new([255, 0, 0], 0.5)?,
    };
    let layer = LayerId::new(1)?;
    document.apply(DocumentEdit::Insert { index: 0, layer: Layer::new(layer, content)? })?;
    tools.set_target(Some(layer));
    tools.select(brush, Some(&document), Duration::ZERO)?;
    let viewport = tools.navigation().viewport();
    let mut point = [1024.0, 1024.0];
    let mut send = |tools: &mut ToolManager, input: ToolInput| {
        tools.input(input, Some(&mut document), Duration::ZERO)
    };
    send(
        &mut tools,
        ToolInput::Pointer(PointerSample::mouse(viewport.device_at(point)?, ToolModifiers::NONE)),
    )?;
    let press = ToolInput::Button {
        button: ToolButton::Primary,
        pressed: true,
        modifiers: ToolModifiers::NONE,
    };
    let started = Instant::now();
    send(&mut tools, press)?;
    let first = started.elapsed();
    let mut total = Duration::ZERO;
    let mut worst = Duration::ZERO;
    for _ in 0..SAMPLES {
        point = [point[0] + step, point[1] + step * 0.5];
        let sample = PointerSample::mouse(viewport.device_at(point)?, ToolModifiers::NONE);
        let started = Instant::now();
        send(&mut tools, ToolInput::Pointer(sample))?;
        let elapsed = started.elapsed();
        total += elapsed;
        worst = worst.max(elapsed);
    }
    let release = ToolInput::Button {
        button: ToolButton::Primary,
        pressed: false,
        modifiers: ToolModifiers::NONE,
    };
    let started = Instant::now();
    send(&mut tools, release)?;
    let commit = started.elapsed();
    Ok(Timing { first, mean: total / SAMPLES as u32, worst, commit })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let memory = MemoryLease::new(16 << 30, Box::new(Unbounded));
    println!("{EDGE}x{EDGE} mask, {SAMPLES} samples per stroke, times per pointer sample");
    for size in [20, 200, 1000] {
        for hardness in [1.0, 0.5] {
            // One dab per sample at the brush's own spacing, then five dabs per sample.
            for dabs in [1.0, 5.0] {
                let step = f64::from(size) * 0.2 * dabs / 1.118_034 * 0.999;
                let timing = stroke(&memory, size, hardness, step)?;
                println!(
                    "size {size:>4} px  hardness {hardness:.1}  {dabs} dab/sample  first {:>8.3} ms  mean {:>8.3} ms  worst {:>8.3} ms  commit {:>6.3} ms",
                    timing.first.as_secs_f64() * 1e3,
                    timing.mean.as_secs_f64() * 1e3,
                    timing.worst.as_secs_f64() * 1e3,
                    timing.commit.as_secs_f64() * 1e3,
                );
            }
        }
    }
    Ok(())
}
