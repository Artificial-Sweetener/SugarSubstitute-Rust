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
pub(in crate::viewer) enum DrawSource {
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
pub(in crate::viewer) struct CpuUnderlay {
    pub(in crate::viewer) surface: Arc<CpuSurface>,
    pub(in crate::viewer) view: ViewProjection,
}
/// Canonical device fallback and optional proof-only CPU reference with the same camera.
pub(in crate::viewer) struct GpuUnderlay {
    pub(in crate::viewer) frame: GpuFrame,
    pub(in crate::viewer) reference: Option<Arc<CpuSurface>>,
    pub(in crate::viewer) view: ViewProjection,
}
/// Immutable identity and mapping of the complete backing filling missing primary samples.
#[derive(Clone, Copy)]
pub(in crate::viewer) struct UnderlayEvidence {
    pub(in crate::viewer) source: FrameStamp,
    pub(in crate::viewer) coverage: BackingCoverage,
    pub(in crate::viewer) projection: ViewProjection,
}
/// One immutable display request, independent of mutable image-operation evaluation.
pub(in crate::viewer) struct DrawInput {
    pub(in crate::viewer) underlay: Option<UnderlayEvidence>,
    pub(in crate::viewer) tier: BackingTier,
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
    pub(in crate::viewer) underlay: Option<UnderlayEvidence>,
    pub(in crate::viewer) tier: BackingTier,
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
        Ok(CpuStep::Complete(Draw {
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
        }))
    }
}

#[cfg(test)]
mod tests;
