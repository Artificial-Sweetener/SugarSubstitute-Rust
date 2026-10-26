// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit coalesced viewer work and owned cleanup through the existing execution boundary.
use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl Session {
    /// Preview scale changes while continuing finished reconstruction during a pan.
    pub(super) fn reconstruction(&self) -> Result<Reconstruction, Box<dyn Error>> {
        if self.initial_preview || self.tools.navigation().resampling() {
            Ok(Reconstruction::Preview)
        } else {
            Ok(Reconstruction::finished_for_display_zoom(
                self.tools.navigation().viewport().zoom(),
            )?)
        }
    }
    /// Keep pan repairs on settled pixels; fractional motion remains a display offset until release.
    pub(super) fn detail_view(
        &self,
        target: Viewport,
        document: (u64, u64),
        source_revision: u64,
        reconstruction: Reconstruction,
    ) -> Result<Viewport, Box<dyn Error>> {
        if self.tools.navigation().captured()
            && !self.tools.navigation().resampling()
            && let Some(shown) = &self.presented
            && shown.tier == BackingTier::Detail
            && shown.document == document
            && shown.source_revision == source_revision
            && !shown.reconstruction.is_preview()
            && shown.reconstruction == reconstruction
            && shown.view.zoom().to_bits() == target.zoom().to_bits()
        {
            return Ok(target.on_sampling_grid(shown.view)?);
        }
        Ok(target)
    }
    /// Drain metadata and typed completions before submitting the latest application intent.
    pub(super) fn advance(&mut self) -> Result<(), Box<dyn Error>> {
        self.tools.advance(self.navigation_time())?;
        self.cancel_obsolete_refinement()?;
        if let Some(runtime) = &self.runtime {
            let batch =
                runtime.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if batch.notifications.iter().any(|notice| notice.scope_id != runtime.scope.id()) {
                return Err("Canvas received a foreign execution scope notice".into());
            }
        }
        self.consume_initialization()?;
        if self.pending_retirement.is_none() {
            self.consume_device()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_load()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_frame()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_display()?;
        }
        if let Some(outcome) = Self::take(&mut self.retirement) {
            if let Err(cause) = Self::terminal(outcome) {
                self.fail(cause);
            }
            if self.pending_retirement.is_none() {
                self.retirement_deadline = None;
            }
        }
        if self.closing {
            self.advance_close()?;
        }
        self.submit_retirement()?;
        if self.closing || self.pending_retirement.is_some() || self.retirement.is_some() {
            return Ok(());
        }
        if self.initialization_pending {
            self.submit_initialization()?;
        }
        if self.initializing.is_some() || self.resources.is_none() {
            return Ok(());
        }
        if self.picker {
            return Ok(());
        }
        if !self.gpu_ready {
            if self.failed.is_none() {
                self.initialize_gpu()?;
            }
            return Ok(());
        }
        if self.pending_open.is_some() {
            if self.loading.is_none() && self.rendering.is_none() {
                self.submit_load()?;
            }
            return Ok(());
        }
        if self.failed.is_none() {
            self.submit_display()?;
        }
        if self.loading.is_none() && self.rendering.is_none() && self.failed.is_none() {
            if self.gpu_presentation.is_some() || self.awaiting_gpu.is_some() {
                return Ok(());
            }
            self.submit_frame()?;
        }
        Ok(())
    }
    /// Commit a camera request only when the preceding request has actually completed.
    /// Navigation remains coalesced intent while a current document frame is being prepared.
    fn submit_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let scene = self.scene.as_ref().ok_or("Canvas document is unavailable")?;
        let document = scene.document.snapshot();
        let identity = (document.identity(), document.revision());
        let target_view = self.tools.navigation().viewport();
        let document_extent = document.extent();
        let display_extent = target_view.surface();
        let needs_display_buffers = self.needs_display_buffers(display_extent);
        let fallback_current = self.fallback_presented.as_ref().is_some_and(|shown| {
            shown.document == identity && shown.source_revision == scene.source_revision
        });
        let (tier, coverage, reconstruction) = if !fallback_current {
            (BackingTier::Fallback, coverage::fallback(document_extent)?, Reconstruction::Preview)
        } else if needs_display_buffers {
            let shown =
                self.fallback_presented.as_ref().ok_or("Canvas fallback identity was lost")?;
            (BackingTier::Fallback, shown.coverage, shown.reconstruction)
        } else {
            let reconstruction = self.reconstruction()?;
            if reconstruction.is_preview() && self.tools.navigation().interacting() {
                return Ok(());
            }
            let detail_view =
                self.detail_view(target_view, identity, scene.source_revision, reconstruction)?;
            let desired = coverage::detail(detail_view)?;
            let current = if let Some(shown) = self.presented.as_ref() {
                shown.document == identity
                    && shown.source_revision == scene.source_revision
                    && shown.reconstruction == reconstruction
                    && shown.view.zoom() == target_view.zoom()
                    && !coverage::detail_needs_refresh(
                        shown.coverage,
                        detail_view,
                        document_extent,
                    )?
                    && shown.coverage.crop(target_view, document_extent)?.is_some()
            } else {
                false
            };
            if current {
                return Ok(());
            }
            (BackingTier::Detail, desired, reconstruction)
        };
        let view = coverage.viewport();
        let shown = match tier {
            BackingTier::Fallback => self.fallback_presented.as_ref(),
            BackingTier::Detail => self.presented.as_ref(),
        };
        let compatible = shown.filter(|shown| {
            shown.document == identity
                && shown.source_revision == scene.source_revision
                && shown.reconstruction == reconstruction
                && shown.view.surface() == view.surface()
        });
        let damage = match compatible {
            Some(shown) => view.repair_from(shown.view)?,
            None => DamagePlan::full(view.surface()),
        };
        let replace_surface = if self.options.gpu.is_some() {
            let surface = match tier {
                BackingTier::Fallback => self.fallback_gpu_front.as_ref(),
                BackingTier::Detail => self.gpu_front.as_ref(),
            };
            surface.is_none_or(|surface| surface.extent() != view.surface())
        } else {
            let surface = match tier {
                BackingTier::Fallback => self.fallback_front.as_ref(),
                BackingTier::Detail => self.front.as_ref(),
            };
            surface.is_none_or(|surface| surface.extent() != view.surface())
        };
        let replace_buffers = needs_display_buffers;
        let base = if replace_surface {
            None
        } else {
            compatible.and_then(|_| match tier {
                BackingTier::Fallback => {
                    if self.options.gpu.is_some() {
                        self.fallback_gpu_front.as_ref().and_then(GpuSurface::current)
                    } else {
                        self.fallback_front.as_ref().and_then(|surface| surface.current())
                    }
                }
                BackingTier::Detail => self.current_frame(),
            })
        };
        let next = self.frame_revision.checked_add(1).ok_or("Canvas frame revision exhausted")?;
        let target = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(next)? };
        let bytes = if self.options.gpu.is_some() {
            if self.options.capture { work::GPU_PROOF_BYTES } else { work::GPU_PREPARE_BYTES }
        } else {
            damage
                .dirty()
                .pixels()
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(256 * 1024))
                .ok_or("Canvas patch reservation overflow")?
        };
        let spec = self.spec(
            Operation::ImageEvaluate,
            JobClass::CpuDecode,
            bytes,
            Duration::from_secs(300),
            None,
        )?;
        let resources =
            self.resources.as_ref().ok_or("Canvas preparation resources are unavailable")?.clone();
        let source = scene.source.clone();
        let source_revision = scene.source_revision;
        let gpu_surface = match tier {
            BackingTier::Fallback => self.fallback_gpu_front.clone(),
            BackingTier::Detail => self.gpu_front.clone(),
        };
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_cpu(&runtime.scope, spec, move |_| work::Prepare {
            tier,
            reconstruction,
            resources,
            document,
            source,
            source_revision,
            view,
            target,
            base,
            damage,
            replace_surface,
            replace_buffers,
            display_extent,
            gpu_surface,
            finished: false,
            cpu: None,
            display_buffers: None,
            started: None,
            source_stats_before: None,
        }) {
            Ok(job) => {
                self.rendering = Some(Rendering {
                    job,
                    reconstruction,
                    tier,
                    coverage,
                    display_extent,
                    superseded: false,
                });
                self.frame_revision = next;
            }
            Err(cause) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Fund and transfer disposal metadata only after actual admission; never drop source handles
    /// or large prepared payloads on the UI thread merely because a newer selection superseded them.
    fn submit_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.retirement.is_some() || self.pending_retirement.is_none() {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            64 * 1024,
            Duration::from_secs(5),
            None,
        )?;
        spec.deadline = self.retirement_deadline;
        match runtime.client.submit_blocking_owned_box(
            &runtime.scope,
            spec,
            &mut self.pending_retirement,
            |value, _| {
                drop(value);
                Ok::<(), WorkError>(())
            },
        ) {
            Ok(job) => self.retirement = Some(job),
            Err(OwnedInputError::Admission(cause)) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Dispose retained state while cleanup admission is still open, then ask execution to stop.
    fn advance_close(&mut self) -> Result<(), Box<dyn Error>> {
        if self.initializing.is_some()
            || self.device_job.is_some()
            || self.loading.is_some()
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.pending_retirement.is_some()
            || self.retirement.is_some()
        {
            return Ok(());
        }
        if self.resources.is_some()
            || self.scene.is_some()
            || self.front.is_some()
            || self.fallback_front.is_some()
            || self.gpu_front.is_some()
            || self.fallback_gpu_front.is_some()
            || self.gpu_presentation.is_some()
            || self.cpu_projection.is_some()
            || self.display_buffers.iter().any(Option::is_some)
        {
            let retired = Retired {
                _scene: self.scene.take(),
                _resources: self.resources.take(),
                _front: self.front.take(),
                _fallback_front: self.fallback_front.take(),
                _gpu_front: self.gpu_front.take(),
                _fallback_gpu_front: self.fallback_gpu_front.take(),
                _presentation: self.gpu_presentation.take().map(|presentation| presentation.frame),
                _projection: self.cpu_projection.take(),
                _buffers: std::mem::take(&mut self.display_buffers),
                ..Default::default()
            };
            self.presented = None;
            self.fallback_presented = None;
            self.display_evidence = None;
            self.display_buffer_extent = None;
            self.projection_base = None;
            self.retire(retired)?;
        } else if !self.shutdown_started {
            if let Some(runtime) = &mut self.runtime {
                runtime
                    .owner
                    .request_shutdown(self.clock.now().checked_add(Duration::from_secs(5))?);
            }
            self.shutdown_started = true;
        }
        Ok(())
    }
}
/// Actual releases notify this execution owner's completion wake when a feasible retry can run.
pub(super) fn retryable(cause: &AdmissionError) -> bool {
    matches!(
        cause,
        AdmissionError::QueueSaturated { .. }
            | AdmissionError::MemorySaturated { .. }
            | AdmissionError::CompletionSaturated
    )
}
