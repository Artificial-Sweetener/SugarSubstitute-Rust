// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare disposable display from immutable canonical pixels independently of DAG evaluation.
use super::*;
#[cfg(all(windows, feature = "native-composition"))]
mod composition;
mod gpu;
#[cfg(all(windows, feature = "native-composition"))]
pub(in crate::native_canvas) use composition::{CompositionFrame, CompositionTarget};
use std::error::Error;
use std::sync::TryLockError;
use sugarsubstitute_canvas::{BackingCoverage, PresentationOverlay, ViewProjection};
use sugarsubstitute_canvas_render::{
    CpuProjection, GpuFrame, OverlayPixel, OverlayPixels, ProjectionBuffer, ProjectionSamples,
    backdrop::BackdropPlan,
};
use sugarsubstitute_image_gpu::WindowFrame;
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
/// Exact session material revision and viewport-local effect plan for one display.
#[derive(Clone, Copy)]
pub(in crate::native_canvas) struct DisplayBackdrops {
    pub(in crate::native_canvas) plan: BackdropPlan,
    pub(in crate::native_canvas) generation: u64,
}
#[cfg(all(test, feature = "native-composition"))]
impl DisplayBackdrops {
    /// Construct inert presentation material for native preview fixtures.
    pub(in crate::native_canvas) fn empty(extent: Extent) -> Result<Self, WorkError> {
        Ok(Self { plan: BackdropPlan::new(extent, &[])?, generation: 0 })
    }
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
    pub(in crate::native_canvas) document: DocumentKey,
    pub(in crate::native_canvas) source_revision: u64,
    /// Presentation marks drawn over the projection, and the generation identifying them.
    pub(in crate::native_canvas) overlay: Option<PresentationOverlay>,
    pub(in crate::native_canvas) overlay_generation: u64,
    pub(in crate::native_canvas) backdrops: DisplayBackdrops,
}
/// Native consumer of retained GPU display images, fixed when the device is installed.
pub(in crate::native_canvas) enum DisplayTarget {
    /// The standalone viewer's swapchain.
    Window(WindowSurface),
    /// Shared textures copied by the main-window composition.
    #[cfg(all(windows, feature = "native-composition"))]
    Composition(Box<CompositionTarget>),
}
/// A completed GPU display for exactly one native consumer.
pub(in crate::native_canvas) enum NativeFrame {
    /// A drawn swapchain image awaiting presentation on a worker.
    Window(WindowFrame),
    /// A drawn shared texture awaiting the composition copy on the UI thread.
    #[cfg(all(windows, feature = "native-composition"))]
    Composition(CompositionFrame),
}
/// Completed disposable pixels; these never replace canonical image-operation products.
pub(in crate::native_canvas) enum DrawPixels {
    Cpu(Box<CpuProjection>),
    Gpu(Option<NativeFrame>),
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
    pub(in crate::native_canvas) document: DocumentKey,
    pub(in crate::native_canvas) source_revision: u64,
    /// Generation of the overlay drawn into these pixels; zero when none was drawn.
    pub(in crate::native_canvas) overlay_generation: u64,
    pub(in crate::native_canvas) backdrop_generation: u64,
    pub(in crate::native_canvas) backdrop_active: bool,
    pub(in crate::native_canvas) verified: bool,
}
impl DrawInput {
    /// Reserve bounded effect/overlay scratch without increasing the canonical renderer allowance.
    pub(in crate::native_canvas) fn backdrop_scratch_bytes(&self) -> Result<u64, WorkError> {
        if self.backdrops.plan.regions().is_empty() {
            return Ok(0);
        }
        let overlays = self
            .overlay
            .as_ref()
            .map(|overlay| OverlayPixels::maximum_pixels(overlay, self.view.target()))
            .transpose()?
            .unwrap_or(0);
        if matches!(self.source, DrawSource::Gpu { .. }) {
            return Ok(sugarsubstitute_canvas_render::backdrop::GpuBackdrop::scratch_bytes(
                &self.backdrops.plan,
                overlays,
            )?);
        }
        self.backdrops
            .plan
            .cpu_scratch_bytes()
            .checked_add(
                (overlays as u64)
                    .checked_mul(size_of::<OverlayPixel>() as u64)
                    .ok_or_else(|| WorkError::invalid("Backdrop overlay admission overflow"))?,
            )
            .ok_or_else(|| WorkError::invalid("Backdrop CPU scratch admission overflow"))
    }
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
        if self.backdrops.plan.viewport() != self.view.target().surface() {
            return Err(WorkError::invalid("Backdrop plan belongs to another display extent"));
        }
        let overlay = self
            .overlay
            .as_ref()
            .map(|overlay| OverlayPixels::rasterize(overlay, self.view.target(), context))
            .transpose()?;
        let (source, pixels, verified) = match &mut self.source {
            DrawSource::Cpu { surface, storage, underlay } => {
                let buffer = storage
                    .take()
                    .ok_or_else(|| WorkError::invalid("CPU projection storage is unavailable"))?;
                let mut frame = buffer.project_over(
                    self.target,
                    surface,
                    self.view,
                    underlay.as_ref().map(|underlay| (underlay.surface.as_ref(), underlay.view)),
                    self.resources.matte.pixel()?,
                    context,
                )?;
                if let Some(overlay) = &overlay {
                    frame.paint_overlay(overlay)?;
                }
                frame.paint_backdrops(&self.backdrops.plan, context)?;
                (frame.source(), DrawPixels::Cpu(Box::new(frame)), true)
            }
            DrawSource::Gpu { frame, reference, underlay } => {
                let native = gpu::prepare(
                    &self.resources,
                    gpu::Input {
                        frame,
                        reference: reference.as_deref(),
                        underlay: underlay.as_ref(),
                        view: self.view,
                        overlay: overlay.as_ref().map_or(&[], OverlayPixels::pixels),
                        backdrops: self.backdrops,
                    },
                    context,
                )?;
                let verified = native.is_some()
                    || self
                        .resources
                        .display
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
            overlay_generation: overlay.map_or(0, |_| self.overlay_generation),
            backdrop_generation: self.backdrops.generation,
            backdrop_active: !self.backdrops.plan.regions().is_empty(),
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
