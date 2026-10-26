// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce immutable display requests through one portable cadence and owned native handoff.
use super::*;
use sugarsubstitute_canvas::ViewProjection;

impl Session {
    /// Admit only the latest camera when the sole display slot and its cadence are available.
    pub(super) fn submit_display(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing
            || self.displaying.is_some()
            || self.gpu_presentation.is_some()
            || self.awaiting_gpu.is_some()
            || self.cpu_pending.is_some()
        {
            return Ok(());
        }
        let now = self.navigation_time();
        if self.pacing.readiness(now)? != PresentationReadiness::Ready {
            return Ok(());
        }
        let Some(source_stamp) = self.current_frame() else {
            return Ok(());
        };
        let Some(shown) = &self.presented else {
            return Ok(());
        };
        let view = self.tools.navigation().viewport();
        if !self.expose_pending
            && self.display_source == Some(source_stamp)
            && self.display_view == Some(view)
        {
            return Ok(());
        }
        if self.options.gpu.is_none() && shown.view == view {
            return self.stage_cpu_canonical();
        }
        if self.options.gpu.is_none() && !self.display_buffers.iter().any(Option::is_some) {
            return Ok(());
        }
        let next = self.frame_revision.checked_add(1).ok_or("Canvas display revision exhausted")?;
        let target =
            FrameStamp { surface: source_stamp.surface, generation: Generation::new(next)? };
        let mapping = ViewProjection::new(shown.view, view)?;
        let mode = if shown.view == view {
            PresentationMode::Canonical
        } else {
            PresentationMode::Projected
        };
        let input_at = self.input_observed;
        let spec = self.spec(
            Operation::FramePrepare,
            JobClass::CpuInteractive,
            2 * 1024 * 1024,
            Duration::from_secs(30),
            Some(ReplacementKey { surface: SurfaceId::new(2)?, revision: target.generation }),
        )?;
        let resources = self.resources.as_ref().ok_or("Canvas resources are unavailable")?.clone();
        let document = shown.document;
        let source_revision = shown.source_revision;
        let source = if self.options.gpu.is_some() {
            work::DrawSource::Gpu {
                frame: self
                    .gpu_front
                    .as_ref()
                    .and_then(GpuSurface::frame)
                    .ok_or("GPU display source is unavailable")?
                    .clone(),
                reference: if self.options.capture { self.front.clone() } else { None },
            }
        } else {
            let surface = self.front.as_ref().ok_or("CPU display source is unavailable")?.clone();
            let storage = self.display_buffers.iter_mut().find_map(Option::take);
            work::DrawSource::Cpu { surface, storage }
        };
        let mut input = Some(work::DrawInput {
            resources,
            source,
            target,
            view: mapping,
            document,
            source_revision,
        });
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let submitted =
            runtime
                .client
                .submit_cpu_owned_input(&runtime.scope, spec, &mut input, |input, _| input);
        if let Some(input) = input
            && let work::DrawSource::Cpu { storage: Some(storage), .. } = input.source
        {
            self.recycle_buffer(storage)?;
        }
        match submitted {
            Ok(job) => {
                self.pacing.submitted(target, mode, input_at, now)?;
                self.displaying = Some(job);
                self.frame_revision = next;
            }
            Err(sugarsubstitute_execution::OwnedInputError::Admission(cause))
                if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Preserve the actual completed camera and source; newer navigation remains pending intent.
    pub(super) fn consume_display(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(completion) = self.displaying.as_mut().and_then(OwnedJob::try_take_completion)
        else {
            return Ok(());
        };
        let job = self.displaying.take().ok_or("Display execution identity was lost")?;
        Self::observe_terminal(&job, &completion.outcome);
        let target = FrameStamp {
            surface: SurfaceId::new(1)?,
            generation: job
                .spec()
                .replacement
                .ok_or("Display replacement identity is unavailable")?
                .revision,
        };
        let now = self.navigation_time();
        match Self::terminal(completion.outcome) {
            Ok(drawing) => {
                if drawing.target != target || drawing.source.surface != target.surface {
                    self.pacing.abandoned(target, now)?;
                    self.retire(Retired { _drawing: Some(drawing), ..Default::default() })?;
                    return Err("Canvas received a foreign completed display identity".into());
                }
                let current = !self.closing
                    && self.pending_open.is_none()
                    && self.loading.is_none()
                    && drawing.view.surface() == self.tools.navigation().viewport().surface()
                    && self.scene.as_ref().is_some_and(|scene| {
                        let doc = scene.document.snapshot();
                        drawing.document == (doc.identity(), doc.revision())
                            && drawing.source_revision == scene.source_revision
                    });
                if !current {
                    self.pacing.abandoned(target, now)?;
                    if !self.closing
                        && drawing.view.surface() == self.tools.navigation().viewport().surface()
                        && let work::DrawPixels::Cpu(frame) = drawing.pixels
                    {
                        return self.recycle_buffer((*frame).into_buffer());
                    }
                    return self.retire(Retired { _drawing: Some(drawing), ..Default::default() });
                }
                self.native_verified = drawing.verified;
                match drawing.pixels {
                    work::DrawPixels::Cpu(frame) => {
                        self.projection_damage = Some(DamagePlan::full(drawing.view.surface()));
                        if let Some(previous) = self.cpu_projection.replace(frame) {
                            self.recycle_buffer((*previous).into_buffer())?;
                        }
                        self.cpu_shown_projected = true;
                        self.pacing.prepared(target, now)?;
                        if self.native_target.is_some() {
                            self.cpu_pending = Some(target);
                        } else {
                            self.pacing.abandoned(target, now)?;
                            self.display_source = Some(drawing.source);
                            self.display_view = Some(drawing.view);
                        }
                    }
                    work::DrawPixels::Gpu(Some(frame)) => {
                        self.pacing.prepared(target, now)?;
                        self.display_source = Some(drawing.source);
                        self.display_view = Some(drawing.view);
                        self.gpu_presentation = Some(GpuPresentation { target, frame });
                    }
                    work::DrawPixels::Gpu(None) => {
                        self.pacing.abandoned(target, now)?;
                        self.expose_pending = !drawing.verified;
                        if drawing.verified {
                            self.display_source = Some(drawing.source);
                            self.display_view = Some(drawing.view);
                            self.native_presented = Some(target);
                        }
                    }
                }
            }
            Err(cause) => {
                self.pacing.abandoned(target, now)?;
                if self.closing || self.pending_open.is_some() {
                    tracing::debug!(operation="canvas_display_stopped", cause=%cause, "Canvas display reached a terminal outcome before replacement");
                } else {
                    self.fail(cause);
                }
            }
        }
        Ok(())
    }
    /// Stage canonical CPU pixels only after prior native custody has ended.
    pub(super) fn stage_cpu_canonical(&mut self) -> Result<(), Box<dyn Error>> {
        if self.options.gpu.is_some() || self.closing || self.cpu_pending.is_some() {
            return Ok(());
        }
        let Some(target) = self.front.as_ref().and_then(|front| front.current()) else {
            return Ok(());
        };
        let Some(shown) = &self.presented else {
            return Ok(());
        };
        let view = shown.view;
        let input_at = if shown.view == self.tools.navigation().viewport() {
            self.input_observed.max(shown.input_at)
        } else {
            shown.input_at
        };
        if self.native_presented == Some(target) {
            return Ok(());
        }
        let now = self.navigation_time();
        if self.pacing.readiness(now)? != PresentationReadiness::Ready {
            return Ok(());
        }
        if let Some(previous) = self.cpu_projection.take() {
            self.recycle_buffer((*previous).into_buffer())?;
            self.projection_damage = None;
        }
        self.cpu_shown_projected = false;
        if self.native_target.is_some() {
            self.pacing.submitted(target, PresentationMode::Canonical, input_at, now)?;
            self.pacing.prepared(target, now)?;
            self.cpu_pending = Some(target);
        } else {
            self.display_source = Some(target);
            self.display_view = Some(view);
            self.native_presented = Some(target);
        }
        Ok(())
    }
    /// Restore exclusive storage after either publication or rejected admission, without freeing it.
    fn recycle_buffer(&mut self, buffer: ProjectionBuffer) -> Result<(), Box<dyn Error>> {
        let slot = self
            .display_buffers
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("CPU display storage inventory is full")?;
        *slot = Some(buffer);
        Ok(())
    }
    /// Only idle pending work needs a cadence timer; running work supplies its completion wake.
    pub(super) fn display_deadline(&self, now: Duration) -> Option<Duration> {
        let source = self.current_frame()?;
        let pending = self.expose_pending
            || self.display_source != Some(source)
            || self.display_view != Some(self.tools.navigation().viewport())
            || self.rendering.is_some();
        if !pending {
            return None;
        }
        match self.pacing.next_deadline(now) {
            Ok(deadline) => deadline,
            Err(cause) => {
                tracing::error!(operation="canvas_display_deadline", cause=%cause, "Canvas presentation clock is invalid");
                None
            }
        }
    }
}
