// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare an unpublished, centered Fit candidate through the existing CPU rendering owners.

use super::*;
use crate::native_canvas::coverage;
use sugarsubstitute_canvas::{BackingCoverage, DamagePlan};
use sugarsubstitute_canvas_render::ProjectionBuffer;
use sugarsubstitute_image_ops::{EvaluationStats, Filter};
mod reproject;

/// Observe the original execution controls before candidate allocation or projection work.
fn check_control(context: &JobContext) -> Result<(), WorkError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled.into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline.into());
    }
    Ok(())
}

/// Admit reusable physical display storage from the prepared candidate's private CPU owner.
/// Call only on an admitted worker after its candidate preparation has reached a terminal outcome.
/// This can evict reconstructible candidate graph tiles without changing its retained backing.
///
/// # Errors
/// Rejects unavailable CPU state and preserves original control, allocation and admission errors.
pub(in crate::native_canvas) fn projection_buffers(
    resources: &Resources,
    extent: Extent,
    context: &JobContext,
) -> Result<ProjectionBuffers, WorkError> {
    check_control(context)?;
    let mut engine = resources
        .engine
        .lock()
        .map_err(|_| WorkError::invalid("Preview preparation state failed"))?;
    let Some(Renderer::Cpu(renderer)) = engine.renderer.as_mut() else {
        return Err(WorkError::invalid("Preview CPU renderer is unavailable"));
    };
    Ok(renderer.projection_buffers(extent, context)?)
}

/// Complete candidate facts retained until the document transaction accepts or retires them.
/// This value grants no native presentation authority. Retire its final ownership on a worker.
pub(in crate::native_canvas) struct ReadyPreview {
    pub(in crate::native_canvas) scene: Scene,
    pub(in crate::native_canvas) resources: Resources,
    pub(in crate::native_canvas) backing: Arc<CpuSurface>,
    pub(in crate::native_canvas) spare_buffer: ProjectionBuffer,
    pub(in crate::native_canvas) drawing: Draw,
    pub(in crate::native_canvas) reconstruction: Reconstruction,
    pub(in crate::native_canvas) sampling_scale: f64,
    pub(in crate::native_canvas) elapsed: std::time::Duration,
    pub(in crate::native_canvas) stats: EvaluationStats,
    pub(in crate::native_canvas) source_io: Option<ArchiveStats>,
}

/// Own an immutable candidate and a private CPU engine while active final navigation continues.
/// Each step delegates to the admitted preparation or display task and yields between phases.
pub(in crate::native_canvas) struct PreparePreview {
    scene: Option<Scene>,
    resources: Resources,
    coverage: BackingCoverage,
    view: Viewport,
    display_target: FrameStamp,
    prepare: Option<Prepare>,
    drawing: Option<DrawInput>,
    backing: Option<Arc<CpuSurface>>,
    spare_buffer: Option<ProjectionBuffer>,
    facts: Option<PreparedFacts>,
}

/// Evaluation evidence independent of the backing and display allocations.
struct PreparedFacts {
    reconstruction: Reconstruction,
    sampling_scale: f64,
    elapsed: std::time::Duration,
    stats: EvaluationStats,
    source_io: Option<ArchiveStats>,
}

impl Resources {
    /// Share admitted storage custody while isolating candidate renderer state and native peers.
    /// Call only on an admitted worker; the separate engine metadata consumes retained capacity.
    fn preview_cpu(&self) -> Result<Self, WorkError> {
        let metadata = Arc::new(self.memory.claim(1024 * 1024)?);
        Ok(Self {
            capture: self.capture,
            verify_native: self.verify_native,
            memory: self.memory.clone(),
            presentation: self.presentation.clone(),
            cache: self.cache.clone(),
            engine: Arc::new(Mutex::new(Engine {
                renderer: None,
                source_revision: None,
                device: None,
            })),
            display: Arc::new(Mutex::new(None)),
            _metadata: metadata,
            budget_device: None,
        })
    }
}

impl PreparePreview {
    /// Construct a Fit-only candidate on an admitted CPU worker from an already imported scene.
    /// Application owners allocate unique source/backing identities and a later display stamp.
    /// Physical dimensions pass through the canvas surface bounds and existing fallback cap.
    ///
    /// # Errors
    /// Preserves geometry/admission failures and rejects invalid or unordered frame identities.
    pub(in crate::native_canvas) fn new(
        scene: Scene,
        active: &Resources,
        backing_target: FrameStamp,
        display_target: FrameStamp,
        display_extent: Extent,
    ) -> Result<Self, WorkError> {
        if scene.source_revision == 0
            || backing_target.surface != display_target.surface
            || backing_target.generation >= display_target.generation
        {
            return Err(WorkError::invalid("Preview source and frame identities are invalid"));
        }
        let document = scene.document.snapshot();
        let coverage = coverage::fallback(document.extent())?;
        let mut view = Viewport::new(display_extent)?;
        view.fit(document.extent())?;
        if coverage.crop(view, document.extent())?.is_none() {
            return Err(WorkError::invalid("Preview backing does not cover its Fit camera"));
        }
        let resources = active.preview_cpu()?;
        let prepare = Prepare {
            compared: None,
            placed: Vec::new(),
            comparison: None,
            tier: BackingTier::Fallback,
            reconstruction: Reconstruction::Finished(Filter::Lanczos3),
            resources: resources.clone(),
            document,
            source: scene.source.clone(),
            source_revision: scene.source_revision,
            view: coverage.viewport(),
            target: backing_target,
            base: None,
            damage: DamagePlan::full(coverage.viewport().surface()),
            replace_surface: true,
            replace_buffers: true,
            display_extent,
            gpu_surface: None,
            finished: false,
            cpu: None,
            display_buffers: None,
            started: None,
            source_stats_before: None,
        };
        Ok(Self {
            scene: Some(scene),
            resources,
            coverage,
            view,
            display_target,
            prepare: Some(prepare),
            drawing: None,
            backing: None,
            spare_buffer: None,
            facts: None,
        })
    }

    /// Transfer complete backing and exclusive buffers into the existing display task.
    fn begin_display(&mut self, mut packet: Packet) -> Result<(), WorkError> {
        let buffers = packet
            .display_buffers
            .take()
            .ok_or_else(|| WorkError::invalid("Preview display buffers are unavailable"))?;
        let (storage, spare) = buffers.into_parts();
        let Pixels::Cpu(CpuPacket::Replace { surface }) = packet.pixels else {
            return Err(WorkError::invalid(
                "Preview preparation returned a nonreplacement backing",
            ));
        };
        let backing = Arc::new(surface);
        let projection = self
            .coverage
            .crop(self.view, self.prepare_document_extent()?)?
            .ok_or_else(|| WorkError::invalid("Preview backing lost its Fit coverage"))?;
        self.drawing = Some(DrawInput {
            underlay: None,
            tier: BackingTier::Fallback,
            resources: self.resources.clone(),
            source: DrawSource::Cpu {
                surface: backing.clone(),
                storage: Some(storage),
                underlay: None,
            },
            target: self.display_target,
            view: projection,
            coverage: self.coverage,
            document: packet.document,
            source_revision: packet.source_revision,
        });
        self.backing = Some(backing);
        self.spare_buffer = Some(spare);
        self.facts = Some(PreparedFacts {
            reconstruction: packet.reconstruction,
            sampling_scale: packet.sampling_scale,
            elapsed: packet.elapsed,
            stats: packet.stats,
            source_io: packet.source_io,
        });
        Ok(())
    }

    /// Borrow the immutable scene's authored dimensions without consulting active navigation.
    fn prepare_document_extent(&self) -> Result<Extent, WorkError> {
        self.scene
            .as_ref()
            .map(|scene| scene.document.snapshot().extent())
            .ok_or_else(|| WorkError::invalid("Preview document was already transferred"))
    }
}

impl CpuTask<ReadyPreview, WorkError> for PreparePreview {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<ReadyPreview>, WorkError> {
        check_control(context)?;
        if let Some(prepare) = self.prepare.as_mut() {
            return match prepare.step(context)? {
                CpuStep::Yield => Ok(CpuStep::Yield),
                CpuStep::Complete(packet) => {
                    self.begin_display(packet)?;
                    self.prepare = None;
                    Ok(CpuStep::Yield)
                }
            };
        }
        let drawing = self
            .drawing
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Preview request was already prepared"))?;
        match drawing.step(context)? {
            CpuStep::Yield => Ok(CpuStep::Yield),
            CpuStep::Complete(drawing) => {
                self.drawing = None;
                let facts = self.facts.take().ok_or_else(|| {
                    WorkError::invalid("Preview preparation evidence is unavailable")
                })?;
                Ok(CpuStep::Complete(ReadyPreview {
                    scene: self.scene.take().ok_or_else(|| {
                        WorkError::invalid("Preview document was already transferred")
                    })?,
                    resources: self.resources.clone(),
                    backing: self
                        .backing
                        .take()
                        .ok_or_else(|| WorkError::invalid("Preview backing is unavailable"))?,
                    spare_buffer: self.spare_buffer.take().ok_or_else(|| {
                        WorkError::invalid("Preview spare display buffer is unavailable")
                    })?,
                    drawing,
                    reconstruction: facts.reconstruction,
                    sampling_scale: facts.sampling_scale,
                    elapsed: facts.elapsed,
                    stats: facts.stats,
                    source_io: facts.source_io,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests;
