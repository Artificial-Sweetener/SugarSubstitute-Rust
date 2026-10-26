// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce immutable display requests through one portable cadence and owned native handoff.
use super::*;
use sugarsubstitute_canvas::ViewProjection;

/// Exact retained mapping and authored identity committed only by native acknowledgement.
#[derive(Clone, Copy)]
pub(super) struct DisplayEvidence {
    pub(super) underlay: Option<work::UnderlayEvidence>,
    pub(super) tier: BackingTier,
    pub(super) target: FrameStamp,
    pub(super) source: FrameStamp,
    pub(super) view: Viewport,
    pub(super) coverage: BackingCoverage,
    pub(super) projection: ViewProjection,
    pub(super) document: (u64, u64),
    pub(super) source_revision: u64,
}

impl Session {
    /// A pair generation is usable only for its exact native output extent.
    pub(super) fn needs_display_buffers(&self, extent: sugarsubstitute_image_ops::Extent) -> bool {
        self.options.gpu.is_none() && self.display_buffer_extent != Some(extent)
    }

    /// Publish a complete pair generation without withdrawing the acknowledged old projection.
    pub(super) fn install_display_buffers(
        &mut self,
        buffers: [Option<ProjectionBuffer>; 2],
        extent: sugarsubstitute_image_ops::Extent,
    ) -> [Option<ProjectionBuffer>; 2] {
        self.display_buffer_extent = Some(extent);
        std::mem::replace(&mut self.display_buffers, buffers)
    }

    /// Admit only the latest camera when the sole display slot and its cadence are available.
    pub(super) fn submit_display(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing
            || self.displaying.is_some()
            || self.gpu_presentation_pending()
            || self.cpu_pending.is_some()
        {
            return Ok(());
        }
        let now = self.navigation_time();
        if self.pacing.readiness(now)? != PresentationReadiness::Ready {
            return Ok(());
        }
        let view = self.tools.navigation().viewport();
        let Some(selection) = self.select_backing()? else {
            return Ok(());
        };
        let shown = selection.primary.shown;
        let source_stamp = selection.primary.source;
        if !self.display_selection_pending(selection) {
            return Ok(());
        }
        if self.options.gpu.is_none()
            && !self
                .display_buffers
                .iter()
                .flatten()
                .any(|buffer| buffer.extent() == view.surface())
        {
            return Ok(());
        }
        let next = self.frame_revision.checked_add(1).ok_or("Canvas display revision exhausted")?;
        let target =
            FrameStamp { surface: source_stamp.surface, generation: Generation::new(next)? };
        let mode = PresentationMode::Projected;
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
        let underlay_evidence = selection.underlay.map(|layer| work::UnderlayEvidence {
            source: layer.source,
            coverage: layer.shown.coverage,
            projection: layer.projection,
        });
        let source = if self.options.gpu.is_some() {
            let surface = match shown.tier {
                BackingTier::Fallback => self.fallback_gpu_front.as_ref(),
                BackingTier::Detail => self.gpu_front.as_ref(),
            };
            let reference = match shown.tier {
                BackingTier::Fallback => self.fallback_front.clone(),
                BackingTier::Detail => self.front.clone(),
            };
            let underlay = selection
                .underlay
                .map(|layer| {
                    let surface = match layer.shown.tier {
                        BackingTier::Fallback => self.fallback_gpu_front.as_ref(),
                        BackingTier::Detail => self.gpu_front.as_ref(),
                    };
                    let reference = match layer.shown.tier {
                        BackingTier::Fallback => self.fallback_front.clone(),
                        BackingTier::Detail => self.front.clone(),
                    };
                    Ok::<_, Box<dyn Error>>(work::GpuUnderlay {
                        frame: surface
                            .and_then(GpuSurface::frame)
                            .filter(|frame| frame.target() == layer.source)
                            .ok_or("GPU display underlay is unavailable")?
                            .clone(),
                        reference: if self.options.capture { reference } else { None },
                        view: layer.projection,
                    })
                })
                .transpose()?;
            work::DrawSource::Gpu {
                frame: surface
                    .and_then(GpuSurface::frame)
                    .filter(|frame| frame.target() == source_stamp)
                    .ok_or("GPU display source is unavailable")?
                    .clone(),
                reference: if self.options.capture { reference } else { None },
                underlay,
            }
        } else {
            let surface = match shown.tier {
                BackingTier::Fallback => self.fallback_front.as_ref(),
                BackingTier::Detail => self.front.as_ref(),
            }
            .ok_or("CPU display source is unavailable")?
            .clone();
            if surface.current() != Some(source_stamp) {
                return Err("CPU display source generation changed during selection".into());
            }
            let underlay = selection
                .underlay
                .map(|layer| {
                    let surface = match layer.shown.tier {
                        BackingTier::Fallback => self.fallback_front.as_ref(),
                        BackingTier::Detail => self.front.as_ref(),
                    }
                    .filter(|surface| surface.current() == Some(layer.source))
                    .ok_or("CPU display underlay is unavailable")?
                    .clone();
                    Ok::<_, Box<dyn Error>>(work::CpuUnderlay { surface, view: layer.projection })
                })
                .transpose()?;
            let storage = self
                .display_buffers
                .iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|buffer| buffer.extent() == view.surface()))
                .and_then(Option::take);
            work::DrawSource::Cpu { surface, storage, underlay }
        };
        let mut input = Some(work::DrawInput {
            underlay: underlay_evidence,
            tier: shown.tier,
            resources,
            source,
            target,
            view: selection.primary.projection,
            coverage: shown.coverage,
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
                let evidence = DisplayEvidence {
                    underlay: drawing.underlay,
                    tier: drawing.tier,
                    target,
                    source: drawing.source,
                    view: drawing.view,
                    coverage: drawing.coverage,
                    projection: drawing.projection,
                    document: drawing.document,
                    source_revision: drawing.source_revision,
                };
                match drawing.pixels {
                    work::DrawPixels::Cpu(frame) => {
                        if frame.underlay() != drawing.underlay.map(|underlay| underlay.source) {
                            self.pacing.abandoned(target, now)?;
                            self.retire(Retired {
                                _projection: Some(frame),
                                ..Default::default()
                            })?;
                            return Err(
                                "Canvas CPU projection underlay differs from display evidence"
                                    .into(),
                            );
                        }
                        let same_sources = self.display_source == Some(drawing.source)
                            && self
                                .display_evidence
                                .and_then(|evidence| evidence.underlay)
                                .map(|underlay| underlay.source)
                                == drawing.underlay.map(|underlay| underlay.source);
                        let (damage, base) = if same_sources {
                            match (self.display_view, self.native_presented) {
                                (Some(previous), Some(base)) => {
                                    let damage = drawing.view.repair_from(previous)?;
                                    let base = (damage.blit().is_some()
                                        || damage.dirty().is_empty())
                                    .then_some(base);
                                    (damage, base)
                                }
                                _ => (DamagePlan::full(drawing.view.surface()), None),
                            }
                        } else {
                            (DamagePlan::full(drawing.view.surface()), None)
                        };
                        self.projection_damage = Some(damage);
                        self.projection_base = base;
                        if let Some(previous) = self.cpu_projection.replace(frame) {
                            self.recycle_buffer((*previous).into_buffer())?;
                        }
                        self.pacing.prepared(target, now)?;
                        if self.native_target.is_some() {
                            self.cpu_pending = Some(target);
                            self.pending_cpu_evidence = Some(evidence);
                        } else {
                            self.pacing.abandoned(target, now)?;
                            self.accept_display(evidence, false)?;
                        }
                    }
                    work::DrawPixels::Gpu(Some(frame)) => {
                        self.pacing.prepared(target, now)?;
                        self.pending_gpu_evidence = Some(evidence);
                        self.pending_gpu_presentation = Some(presentation::PendingPresentation {
                            target,
                            frame,
                            deadline: self.clock.now().checked_add(Duration::from_secs(30))?,
                            frame_period: self.pacing.period(),
                        });
                    }
                    work::DrawPixels::Gpu(None) => {
                        self.pacing.abandoned(target, now)?;
                        self.expose_pending = !drawing.verified;
                        if drawing.verified {
                            self.accept_display(evidence, false)?;
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
    /// Restore exclusive storage after either publication or rejected admission, without freeing it.
    fn recycle_buffer(&mut self, buffer: ProjectionBuffer) -> Result<(), Box<dyn Error>> {
        if self.display_buffer_extent != Some(buffer.extent()) {
            return self.retire(Retired { _buffers: [Some(buffer), None], ..Default::default() });
        }
        let slot = self
            .display_buffers
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("CPU display storage inventory is full")?;
        *slot = Some(buffer);
        Ok(())
    }
    /// Only idle pending work needs a cadence timer; running work supplies its completion wake.
    pub(super) fn display_deadline(
        &self,
        now: Duration,
    ) -> Result<Option<Duration>, Box<dyn Error>> {
        let Some(selection) = self.select_backing()? else {
            return Ok(None);
        };
        let pending = self.display_selection_pending(selection) || self.rendering.is_some();
        if !pending {
            return Ok(None);
        }
        Ok(self.pacing.next_deadline(now)?)
    }
}
