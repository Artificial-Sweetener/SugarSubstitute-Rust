// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare disposable display from immutable canonical pixels independently of DAG evaluation.
use super::*;
use std::sync::TryLockError;
use sugarsubstitute_canvas::ViewProjection;
use sugarsubstitute_canvas_render::{CpuProjection, GpuFrame, ProjectionBuffer, ProjectionSamples};

/// Immutable source pixels and exclusive software storage or a selected-device image.
pub(in crate::viewer) enum DrawSource {
    Cpu { surface: Arc<CpuSurface>, storage: Option<ProjectionBuffer> },
    Gpu { frame: GpuFrame, reference: Option<Arc<CpuSurface>> },
}
/// One immutable display request, independent of mutable image-operation evaluation.
pub(in crate::viewer) struct DrawInput {
    pub(in crate::viewer) resources: Resources,
    pub(in crate::viewer) source: DrawSource,
    pub(in crate::viewer) target: FrameStamp,
    pub(in crate::viewer) view: ViewProjection,
    pub(in crate::viewer) coverage: sugarsubstitute_canvas::BackingCoverage,
    pub(in crate::viewer) document: (u64, u64),
    pub(in crate::viewer) source_revision: u64,
}
/// Completed disposable pixels; these never replace canonical image-operation products.
pub(in crate::viewer) enum DrawPixels {
    Cpu(Box<CpuProjection>),
    Gpu(Option<WindowFrame>),
}
/// Actual source generation, output camera and authored identity represented by one display.
pub(in crate::viewer) struct Draw {
    pub(in crate::viewer) target: FrameStamp,
    pub(in crate::viewer) source: FrameStamp,
    pub(in crate::viewer) view: Viewport,
    pub(in crate::viewer) coverage: sugarsubstitute_canvas::BackingCoverage,
    pub(in crate::viewer) projection: ViewProjection,
    pub(in crate::viewer) pixels: DrawPixels,
    pub(in crate::viewer) document: (u64, u64),
    pub(in crate::viewer) source_revision: u64,
    pub(in crate::viewer) verified: bool,
}
impl CpuTask<Draw, WorkError> for DrawInput {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<Draw>, WorkError> {
        let (source, pixels, verified) = match &mut self.source {
            DrawSource::Cpu { surface, storage } => {
                let buffer = storage
                    .take()
                    .ok_or_else(|| WorkError::invalid("CPU projection storage is unavailable"))?;
                let frame = buffer.project(
                    self.target,
                    surface,
                    self.view,
                    Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                    context,
                )?;
                (frame.source(), DrawPixels::Cpu(Box::new(frame)), true)
            }
            DrawSource::Gpu { frame, reference } => {
                let native =
                    prepare_gpu(&self.resources, frame, reference.as_deref(), self.view, context)?;
                let verified = native.is_some()
                    || self
                        .resources
                        .window
                        .lock()
                        .map_err(|_| WorkError::invalid("Canvas presentation state failed"))?
                        .is_none();
                (frame.target(), DrawPixels::Gpu(native), verified)
            }
        };
        Ok(CpuStep::Complete(Draw {
            target: self.target,
            source,
            view: self.view.target(),
            coverage: self.coverage,
            projection: self.view,
            pixels,
            document: self.document,
            source_revision: self.source_revision,
            verified,
        }))
    }
}

/// Never wait for the image-operation renderer lock while drawing a retained native image.
fn prepare_gpu(
    resources: &Resources,
    frame: &GpuFrame,
    reference: Option<&CpuSurface>,
    view: ViewProjection,
    context: &JobContext,
) -> Result<Option<WindowFrame>, WorkError> {
    let samples = if view.source() != view.target() {
        Some(ProjectionSamples::new(view, context)?)
    } else {
        None
    };
    let matte = Pixel::linear(0.018, 0.018, 0.018, 1.0)?;
    let mut window = resources
        .window
        .lock()
        .map_err(|_| WorkError::invalid("Canvas native presentation state failed"))?;
    let native = match window.as_mut() {
        Some(window) => loop {
            let prepared = match &samples {
                Some(samples) => {
                    window.prepare_projected(frame.image(), samples.gpu_mapping(matte)?, context)
                }
                None => window.prepare(frame.image(), context),
            };
            match prepared {
                Err(cause) if cause.is_memory_pressure() => {
                    let mut engine = match resources.engine.try_lock() {
                        Ok(engine) => engine,
                        Err(TryLockError::WouldBlock) => break None,
                        Err(TryLockError::Poisoned(_)) => {
                            return Err(WorkError::invalid("Canvas preparation state failed"));
                        }
                    };
                    let evicted = match engine.renderer.as_mut() {
                        Some(Renderer::Gpu(renderer)) => renderer.evict_one(),
                        _ => false,
                    };
                    if !evicted {
                        return Err(cause.into());
                    }
                }
                result => break result?,
            }
        },
        None => None,
    };
    if resources.verify_native
        && let Some(native) = &native
    {
        verify_gpu(frame, native, reference, samples.as_ref(), matte, view, context)?;
    }
    Ok(native)
}

/// Proof-only readback is independent of normal presentation timing and source evaluation.
fn verify_gpu(
    frame: &GpuFrame,
    native: &WindowFrame,
    reference: Option<&CpuSurface>,
    samples: Option<&ProjectionSamples>,
    matte: Pixel,
    view: ViewProjection,
    context: &JobContext,
) -> Result<(), WorkError> {
    if let Some(samples) = samples {
        let reference = reference.ok_or_else(|| {
            WorkError::invalid("GPU projection qualification lacks its canonical CPU reference")
        })?;
        if reference.current() != Some(frame.target()) {
            return Err(WorkError::invalid(
                "GPU projection reference represents another source generation",
            ));
        }
        let rgba = sugarsubstitute_image_ops::SrgbEncoder::encode_constant(matte)?;
        let background = u32::from_be_bytes([255, rgba[0], rgba[1], rgba[2]]);
        let stride = reference.extent().width() as usize;
        for region in view.target().surface().region().tiles() {
            let actual = native.read_region(region, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let row = samples.rows()[y as usize];
                    let column = samples.columns()[x as usize];
                    let expected = if row == ProjectionSamples::UNCOVERED
                        || column == ProjectionSamples::UNCOVERED
                    {
                        background
                    } else {
                        reference.pixels()[row as usize * stride + column as usize]
                    };
                    let index = ((y - region.y()) * region.width() + x - region.x()) as usize;
                    if actual.pixels()[index] != expected {
                        return Err(WorkError::invalid(
                            "Native GPU projection differs from canonical nearest sampling",
                        ));
                    }
                }
            }
        }
    } else {
        for region in frame.image().extent().region().tiles() {
            let source = frame.image().read_region(region, context)?;
            if native.read_region(region, context)?.pixels() != source.pixels() {
                return Err(WorkError::invalid(
                    "Native GPU presentation differs from its completed display image",
                ));
            }
        }
    }
    Ok(())
}
