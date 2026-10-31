// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce immutable display requests through one portable cadence and owned native handoff.
use super::*;
use sugarsubstitute_canvas::ViewProjection;
mod admission;

/// Preserve the submitted frame identity independently of execution-lane replacement metadata.
pub(super) struct Displaying {
    pub(super) target: FrameStamp,
    pub(super) document: (u64, u64),
    pub(super) source_revision: u64,
    pub(super) job: OwnedJob<work::Draw, WorkError>,
}

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
    /// Withdraw presentation authority when its native attachment is released.
    /// Pending CPU pixels remain owned, but cannot wait for an acknowledgement from a dead peer.
    /// Running display work remains observed and is rejected while the document is hidden.
    ///
    /// # Errors
    /// Preserves custody when pending frame evidence or its pacing identity is inconsistent.
    #[cfg(feature = "native-composition")]
    pub(crate) fn revoke_presentation(&mut self) -> Result<(), Box<dyn Error>> {
        self.abandon_cpu_frame(self.navigation_time())?;
        #[cfg(windows)]
        self.abandon_composition_frame(self.navigation_time())?;
        self.drawable = false;
        Ok(())
    }

    /// Check frame and pacing custody before another owner mutates its viewport or source.
    pub(super) fn pending_cpu_withdrawal(
        &self,
        now: Duration,
    ) -> Result<Option<FrameStamp>, Box<dyn Error>> {
        let Some(target) = self.document.cpu_pending else {
            return Ok(None);
        };
        if self.document.pending_cpu_evidence.is_none_or(|evidence| evidence.target != target)
            || self.document.cpu_projection.as_ref().is_none_or(|frame| frame.target() != target)
            || !matches!(
                self.pacing.readiness(now)?,
                PresentationReadiness::Busy { target: pending, .. } if pending == target
            )
        {
            return Err("Pending CPU frame custody does not match its presentation".into());
        }
        Ok(Some(target))
    }

    /// Withdraw an unaccepted CPU projection before changing its source or physical extent.
    /// The desktop only borrows CPU pixels for a synchronous upload, so no native worker
    /// owns this frame. Keep its buffer available for later worker-owned replacement.
    pub(super) fn abandon_cpu_frame(&mut self, now: Duration) -> Result<(), Box<dyn Error>> {
        let Some(target) = self.pending_cpu_withdrawal(now)? else {
            return Ok(());
        };
        self.pacing.abandoned(target, now)?;
        self.document.cpu_pending = None;
        self.document.pending_cpu_evidence = None;
        Ok(())
    }

    /// A pair generation is usable only for its exact native output extent.
    pub(super) fn needs_display_buffers(&self, extent: sugarsubstitute_image_ops::Extent) -> bool {
        !self.gpu_document() && self.document.display_buffer_extent != Some(extent)
    }

    /// Publish a complete pair generation without withdrawing the acknowledged old projection.
    pub(super) fn install_display_buffers(
        &mut self,
        buffers: [Option<ProjectionBuffer>; 2],
        extent: sugarsubstitute_image_ops::Extent,
    ) -> [Option<ProjectionBuffer>; 2] {
        self.document.display_buffer_extent = Some(extent);
        std::mem::replace(&mut self.document.display_buffers, buffers)
    }

    /// Admit only the latest camera when the sole display slot and its cadence are available.
    pub(super) fn submit_display(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing
            || !self.drawable
            || self.displaying.is_some()
            || self.gpu_presentation_pending()
            || self.document.cpu_pending.is_some()
        {
            return Ok(());
        }
        let now = self.navigation_time();
        if self.pacing.readiness(now)? != PresentationReadiness::Ready {
            return Ok(());
        }
        let view = self.document.tools.navigation().viewport();
        let Some(selection) = self.select_backing()? else {
            return Ok(());
        };
        let shown = selection.primary.shown;
        let source_stamp = selection.primary.source;
        if !self.display_selection_pending(selection) {
            return Ok(());
        }
        if !self.gpu_document()
            && !self
                .document
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
        let resources = self.resources.as_ref().ok_or("Canvas resources are unavailable")?.clone();
        let document = shown.document;
        let source_revision = shown.source_revision;
        let underlay_evidence = selection.underlay.map(|layer| work::UnderlayEvidence {
            tier: layer.shown.tier,
            source: layer.source,
            coverage: layer.shown.coverage,
            projection: layer.projection,
        });
        let source = if self.gpu_document() {
            let surface = match shown.tier {
                BackingTier::Fallback => self.document.fallback_gpu_front.as_ref(),
                BackingTier::Navigation => self.document.navigation_gpu_front.as_ref(),
                BackingTier::Detail => self.document.gpu_front.as_ref(),
            };
            let reference = match shown.tier {
                BackingTier::Fallback => self.document.fallback_front.clone(),
                BackingTier::Navigation => self.document.navigation_front.clone(),
                BackingTier::Detail => self.document.front.clone(),
            };
            let underlay = selection
                .underlay
                .map(|layer| {
                    let surface = match layer.shown.tier {
                        BackingTier::Fallback => self.document.fallback_gpu_front.as_ref(),
                        BackingTier::Navigation => self.document.navigation_gpu_front.as_ref(),
                        BackingTier::Detail => self.document.gpu_front.as_ref(),
                    };
                    let reference = match layer.shown.tier {
                        BackingTier::Fallback => self.document.fallback_front.clone(),
                        BackingTier::Navigation => self.document.navigation_front.clone(),
                        BackingTier::Detail => self.document.front.clone(),
                    };
                    Ok::<_, Box<dyn Error>>(work::GpuUnderlay {
                        frame: surface
                            .and_then(GpuSurface::frame)
                            .filter(|frame| frame.target() == layer.source)
                            .ok_or("GPU display underlay is unavailable")?
                            .clone(),
                        reference: if self.config.capture { reference } else { None },
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
                reference: if self.config.capture { reference } else { None },
                underlay,
            }
        } else {
            let surface = match shown.tier {
                BackingTier::Fallback => self.document.fallback_front.as_ref(),
                BackingTier::Navigation => self.document.navigation_front.as_ref(),
                BackingTier::Detail => self.document.front.as_ref(),
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
                        BackingTier::Fallback => self.document.fallback_front.as_ref(),
                        BackingTier::Navigation => self.document.navigation_front.as_ref(),
                        BackingTier::Detail => self.document.front.as_ref(),
                    }
                    .filter(|surface| surface.current() == Some(layer.source))
                    .ok_or("CPU display underlay is unavailable")?
                    .clone();
                    Ok::<_, Box<dyn Error>>(work::CpuUnderlay { surface, view: layer.projection })
                })
                .transpose()?;
            let storage = self
                .document
                .display_buffers
                .iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|buffer| buffer.extent() == view.surface()))
                .and_then(Option::take);
            work::DrawSource::Cpu { surface, storage, underlay }
        };
        let input = work::DrawInput {
            underlay: underlay_evidence,
            tier: shown.tier,
            resources,
            source,
            target,
            view: selection.primary.projection,
            coverage: shown.coverage,
            document,
            source_revision,
        };
        self.admit_display(input, mode, input_at, now)
    }
    /// Preserve the actual completed camera and source; newer navigation remains pending intent.
    pub(super) fn consume_display(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(completion) =
            self.displaying.as_mut().and_then(|display| display.job.try_take_completion())
        else {
            return Ok(());
        };
        let Displaying { target, document, source_revision, job } =
            self.displaying.take().ok_or("Display execution identity was lost")?;
        Self::observe_terminal(&job, &completion.outcome);
        let now = self.navigation_time();
        let current_source = self.admitted_source_is_current(document, source_revision);
        match Self::terminal(completion.outcome) {
            Ok(drawing) => {
                if !current_source {
                    self.pacing.abandoned(target, now)?;
                    return self.retire(Retired { _drawing: Some(drawing), ..Default::default() });
                }
                if drawing.target != target
                    || drawing.source.surface != target.surface
                    || drawing.document != document
                    || drawing.source_revision != source_revision
                {
                    self.pacing.abandoned(target, now)?;
                    self.retire(Retired { _drawing: Some(drawing), ..Default::default() })?;
                    return Err("Canvas received a foreign completed display identity".into());
                }
                let current = !self.closing
                    && self.drawable
                    && self.pending_open.is_none()
                    && self.loading.is_none()
                    && current_source
                    && drawing.view.surface()
                        == self.document.tools.navigation().viewport().surface();
                if !current {
                    self.pacing.abandoned(target, now)?;
                    if !self.closing
                        && current_source
                        && drawing.view.surface()
                            == self.document.tools.navigation().viewport().surface()
                        && let work::DrawPixels::Cpu(frame) = drawing.pixels
                    {
                        return self.recycle_buffer((*frame).into_buffer());
                    }
                    return self.retire(Retired { _drawing: Some(drawing), ..Default::default() });
                }
                self.document.native_verified = drawing.verified;
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
                        let same_sources = self.document.display_source == Some(drawing.source)
                            && self
                                .document
                                .display_evidence
                                .and_then(|evidence| evidence.underlay)
                                .map(|underlay| underlay.source)
                                == drawing.underlay.map(|underlay| underlay.source);
                        let (damage, base) = if same_sources {
                            match (self.document.display_view, self.document.native_presented) {
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
                        self.document.projection_damage = Some(damage);
                        self.document.projection_base = base;
                        if let Some(previous) = self.document.cpu_projection.replace(frame) {
                            self.recycle_buffer((*previous).into_buffer())?;
                        }
                        self.pacing.prepared(target, now)?;
                        if self.presentation.attached() {
                            self.document.cpu_pending = Some(target);
                            self.document.pending_cpu_evidence = Some(evidence);
                        } else {
                            self.pacing.abandoned(target, now)?;
                            self.accept_display(evidence, false)?;
                        }
                    }
                    #[cfg(all(windows, feature = "native-composition"))]
                    work::DrawPixels::Gpu(Some(work::NativeFrame::Composition(frame))) => {
                        self.offer_composition_frame(target, frame, evidence, now)?;
                    }
                    work::DrawPixels::Gpu(Some(work::NativeFrame::Window(frame))) => {
                        self.pacing.prepared(target, now)?;
                        self.document.pending_gpu_evidence = Some(evidence);
                        self.pending_gpu_presentation = Some(presentation::PendingPresentation {
                            target,
                            frame,
                            deadline: self.clock.now().checked_add(Duration::from_secs(30))?,
                            frame_period: self.pacing.period(),
                        });
                    }
                    work::DrawPixels::Gpu(None) => {
                        self.pacing.abandoned(target, now)?;
                        self.expose_pending |= !drawing.verified;
                        if drawing.verified {
                            self.accept_display(evidence, false)?;
                            self.document.native_presented = Some(target);
                        }
                    }
                }
            }
            Err(cause) => {
                self.pacing.abandoned(target, now)?;
                #[cfg(feature = "native-composition")]
                let restoring = self.restoring_document();
                #[cfg(not(feature = "native-composition"))]
                let restoring = false;
                if self.closing || self.pending_open.is_some() || restoring || !current_source {
                    tracing::debug!(operation="canvas_display_stopped", job_id=job.spec().id.get(), document_id=document.0, document_revision=document.1, source_revision, cause=%cause, "Canvas display reached a terminal outcome before replacement");
                } else {
                    self.fail(cause);
                }
            }
        }
        Ok(())
    }
    /// Restore exclusive storage after either publication or rejected admission, without freeing it.
    fn recycle_buffer(&mut self, buffer: ProjectionBuffer) -> Result<(), Box<dyn Error>> {
        if self.document.display_buffer_extent != Some(buffer.extent()) {
            return self.retire(Retired { _buffers: [Some(buffer), None], ..Default::default() });
        }
        let slot = self
            .document
            .display_buffers
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("CPU display storage inventory is full")?;
        *slot = Some(buffer);
        Ok(())
    }
    /// Only idle pending work needs a cadence timer; running work supplies its completion wake.
    /// A finished render that `consume_frame` withholds for pacing is idle work too, and may
    /// precede any selectable backing for a newly selected source.
    pub(super) fn display_deadline(
        &self,
        now: Duration,
    ) -> Result<Option<Duration>, Box<dyn Error>> {
        if !self.drawable {
            return Ok(None);
        }
        let paced_completion = !self.gpu_document()
            && self.presentation.attached()
            && self.rendering.as_ref().is_some_and(|rendering| {
                matches!(
                    rendering.job.status().phase,
                    sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
                )
            });
        if paced_completion {
            return Ok(self.pacing.next_deadline(now)?);
        }
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
