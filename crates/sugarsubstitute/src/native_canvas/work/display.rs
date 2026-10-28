// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare disposable display from immutable canonical pixels independently of DAG evaluation.
use super::*;
mod gpu;
use std::error::Error;
use std::sync::TryLockError;
use sugarsubstitute_canvas::{BackingCoverage, ViewProjection};
use sugarsubstitute_canvas_render::{CpuProjection, GpuFrame, ProjectionBuffer, ProjectionSamples};
use sugarsubstitute_image_gpu::{DisplayReadbackWorkspace, GpuError};

enum ReadbackOutcome<T> {
    Complete(T),
    Deferred,
}

enum CacheReclaim {
    Progress,
    Busy,
    Exhausted,
}

fn retry_after_cache_reclaim<T, E>(
    mut operation: impl FnMut() -> Result<T, E>,
    mut reclaim: impl FnMut() -> Result<CacheReclaim, WorkError>,
    retryable: impl Fn(&E) -> bool,
) -> Result<ReadbackOutcome<T>, WorkError>
where
    E: Error + Send + Sync + 'static,
    WorkError: From<E>,
{
    loop {
        match operation() {
            Ok(value) => return Ok(ReadbackOutcome::Complete(value)),
            Err(cause) if retryable(&cause) => match reclaim()? {
                CacheReclaim::Progress => {}
                CacheReclaim::Busy => return Ok(ReadbackOutcome::Deferred),
                CacheReclaim::Exhausted => return Err(cause.into()),
            },
            Err(cause) => return Err(cause.into()),
        }
    }
}

fn retry_gpu_readback<T>(
    operation: impl FnMut() -> Result<T, GpuError>,
    reclaim: impl FnMut() -> Result<CacheReclaim, WorkError>,
) -> Result<ReadbackOutcome<T>, WorkError> {
    retry_after_cache_reclaim(operation, reclaim, GpuError::is_memory_pressure)
}

/// Immutable source pixels and exclusive software storage or a selected-device image.
pub(in crate::native_canvas) enum DrawSource {
    Cpu {
        surface: Arc<CpuSurface>,
        storage: Option<ProjectionBuffer>,
        underlay: Option<CpuUnderlay>,
    },
    Gpu {
        frame: GpuFrame,
        reference: Option<Arc<CpuSurface>>,
        underlay: Option<GpuUnderlay>,
    },
}
/// Canonical fallback pixels retained only for uncovered primary display samples.
pub(in crate::native_canvas) struct CpuUnderlay {
    pub(in crate::native_canvas) surface: Arc<CpuSurface>,
    pub(in crate::native_canvas) view: ViewProjection,
}
/// Canonical device fallback and optional proof-only CPU reference with the same camera.
pub(in crate::native_canvas) struct GpuUnderlay {
    pub(in crate::native_canvas) frame: GpuFrame,
    pub(in crate::native_canvas) reference: Option<Arc<CpuSurface>>,
    pub(in crate::native_canvas) view: ViewProjection,
}
/// Immutable identity and mapping of the complete backing filling missing primary samples.
#[derive(Clone, Copy)]
pub(in crate::native_canvas) struct UnderlayEvidence {
    pub(in crate::native_canvas) tier: BackingTier,
    pub(in crate::native_canvas) source: FrameStamp,
    pub(in crate::native_canvas) coverage: BackingCoverage,
    pub(in crate::native_canvas) projection: ViewProjection,
}
/// One immutable display request, independent of mutable image-operation evaluation.
pub(in crate::native_canvas) struct DrawInput {
    pub(in crate::native_canvas) underlay: Option<UnderlayEvidence>,
    pub(in crate::native_canvas) tier: BackingTier,
    pub(in crate::native_canvas) resources: Resources,
    pub(in crate::native_canvas) source: DrawSource,
    pub(in crate::native_canvas) target: FrameStamp,
    pub(in crate::native_canvas) view: ViewProjection,
    pub(in crate::native_canvas) coverage: sugarsubstitute_canvas::BackingCoverage,
    pub(in crate::native_canvas) document: (u64, u64),
    pub(in crate::native_canvas) source_revision: u64,
}
/// Completed disposable pixels; these never replace canonical image-operation products.
pub(in crate::native_canvas) enum DrawPixels {
    Cpu(Box<CpuProjection>),
    Gpu(Option<WindowFrame>),
}
/// Actual source generation, output camera and authored identity represented by one display.
pub(in crate::native_canvas) struct Draw {
    pub(in crate::native_canvas) underlay: Option<UnderlayEvidence>,
    pub(in crate::native_canvas) tier: BackingTier,
    pub(in crate::native_canvas) target: FrameStamp,
    pub(in crate::native_canvas) source: FrameStamp,
    pub(in crate::native_canvas) view: Viewport,
    pub(in crate::native_canvas) coverage: sugarsubstitute_canvas::BackingCoverage,
    pub(in crate::native_canvas) projection: ViewProjection,
    pub(in crate::native_canvas) pixels: DrawPixels,
    pub(in crate::native_canvas) document: (u64, u64),
    pub(in crate::native_canvas) source_revision: u64,
    pub(in crate::native_canvas) verified: bool,
}
impl DrawInput {
    /// Prepare native GPU presentation on an admitted blocking worker, independently of the
    /// cooperative CPU lane occupied by image reconstruction.
    pub(in crate::native_canvas) fn prepare_gpu(
        mut self,
        context: JobContext,
    ) -> Result<Draw, WorkError> {
        if !matches!(self.source, DrawSource::Gpu { .. }) {
            return Err(WorkError::invalid("Native display preparation requires a GPU source"));
        }
        self.prepare(&context)
    }
    /// Validate retained identities before consuming exclusive software storage or a GPU image.
    fn prepare(&mut self, context: &JobContext) -> Result<Draw, WorkError> {
        let underlay = match &self.source {
            DrawSource::Cpu { underlay, .. } => underlay.as_ref().and_then(|underlay| {
                underlay.surface.current().map(|source| (source, underlay.view))
            }),
            DrawSource::Gpu { underlay, .. } => {
                underlay.as_ref().map(|underlay| (underlay.frame.target(), underlay.view))
            }
        };
        if underlay != self.underlay.map(|underlay| (underlay.source, underlay.projection)) {
            return Err(WorkError::invalid("Display underlay differs from its retained evidence"));
        }
        let (source, pixels, verified) = match &mut self.source {
            DrawSource::Cpu { surface, storage, underlay } => {
                let buffer = storage
                    .take()
                    .ok_or_else(|| WorkError::invalid("CPU projection storage is unavailable"))?;
                let frame = buffer.project_over(
                    self.target,
                    surface,
                    self.view,
                    underlay.as_ref().map(|underlay| (underlay.surface.as_ref(), underlay.view)),
                    Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                    context,
                )?;
                (frame.source(), DrawPixels::Cpu(Box::new(frame)), true)
            }
            DrawSource::Gpu { frame, reference, underlay } => {
                let native = gpu::prepare(
                    &self.resources,
                    frame,
                    reference.as_deref(),
                    underlay.as_ref(),
                    self.view,
                    context,
                )?;
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
        Ok(Draw {
            underlay: self.underlay,
            tier: self.tier,
            target: self.target,
            source,
            view: self.view.target(),
            coverage: self.coverage,
            projection: self.view,
            pixels,
            document: self.document,
            source_revision: self.source_revision,
            verified,
        })
    }
}

impl CpuTask<Draw, WorkError> for DrawInput {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<Draw>, WorkError> {
        if !matches!(self.source, DrawSource::Cpu { .. }) {
            return Err(WorkError::invalid(
                "GPU presentation cannot occupy the cooperative CPU lane",
            ));
        }
        self.prepare(context).map(CpuStep::Complete)
    }
}

#[cfg(test)]
mod tests;
