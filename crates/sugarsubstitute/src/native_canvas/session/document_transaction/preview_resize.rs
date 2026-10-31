// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Refit preview display storage while retaining canonical pixels and the parked final.

use super::{preview::*, *};
use sugarsubstitute_canvas_render::ProjectionBuffers;
use sugarsubstitute_execution::OwnedInputError;
use sugarsubstitute_image_ops::Extent;

/// Allocate one replacement pair without reevaluating the retained preview backing.
struct BuffersTask {
    resources: Resources,
    extent: Extent,
}
impl CpuTask<ProjectionBuffers, WorkError> for BuffersTask {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<ProjectionBuffers>, WorkError> {
        work::preview::projection_buffers(&self.resources, self.extent, context)
            .map(CpuStep::Complete)
    }
}

impl Session {
    /// Return an offered candidate to private preparation before physical geometry changes.
    pub(in crate::native_canvas::session) fn resize_preview_candidate(
        &mut self,
        extent: Extent,
    ) -> Result<(), Box<dyn Error>> {
        if self.preview_shown() && extent != self.document.tools.navigation().viewport().surface() {
            self.preview.poll_requested = true;
            self.preview.resize_extent = Some(extent);
            self.preview.resize_deadline =
                Some(self.clock.now().checked_add(Duration::from_secs(30))?);
            if let Some(job) = &self.preview.resize_job {
                job.cancel();
            }
        }
        let Some(candidate) = &mut self.preview.candidate else {
            return Ok(());
        };
        let Stage::Offered(document, resources) = &mut candidate.stage else {
            return Ok(());
        };
        if document.tools.navigation().viewport().surface() == extent || candidate.cancelled {
            return Ok(());
        }
        let target = document.cpu_pending.ok_or("Offered preview lost its target")?;
        let now = Duration::from_nanos(self.clock.now().as_nanos());
        self.pacing.abandoned(target, now)?;
        let evidence =
            document.pending_cpu_evidence.take().ok_or("Offered preview lost its evidence")?;
        let shown =
            document.fallback_presented.ok_or("Offered preview lost its backing evidence")?;
        let ready = work::preview::ReadyPreview {
            scene: document.scene.take().ok_or("Offered preview lost its scene")?,
            resources: resources.take().ok_or("Offered preview lost its resources")?,
            backing: document.fallback_front.take().ok_or("Offered preview lost its backing")?,
            spare_buffer: document.display_buffers[0]
                .take()
                .ok_or("Offered preview lost its spare pixels")?,
            drawing: work::Draw {
                underlay: evidence.underlay,
                tier: evidence.tier,
                target,
                source: evidence.source,
                view: evidence.view,
                coverage: evidence.coverage,
                projection: evidence.projection,
                pixels: work::DrawPixels::Cpu(
                    document.cpu_projection.take().ok_or("Offered preview lost its pixels")?,
                ),
                document: evidence.document,
                source_revision: evidence.source_revision,
                verified: document.native_verified,
            },
            reconstruction: shown.reconstruction,
            sampling_scale: shown.sampling_scale,
            elapsed: self.last_prepare,
            stats: self.last_stats,
            source_io: None,
        };
        candidate.stage = Stage::Ready(Some(Box::new(ready)));
        self.preview.poll_requested = true;
        Ok(())
    }

    /// Allocate only projection storage through the existing CPU owner after accepted-preview resize.
    pub(in crate::native_canvas::session) fn advance_preview_resize(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(outcome) = Self::take(&mut self.preview.resize_job) {
            let completed = self.preview.resize_job_intent.take();
            let latest = self.preview.resize_extent.zip(self.preview.resize_deadline);
            match Self::terminal(outcome) {
                Ok(buffers) => {
                    let (first, second) = buffers.into_parts();
                    let extent = first.extent();
                    let retired = if completed == latest
                        && !self.closing
                        && !self.preview.restoring
                        && self.preview_shown()
                        && self.document.tools.navigation().viewport().surface() == extent
                    {
                        self.preview.resize_deadline = None;
                        self.preview.resize_extent = None;
                        self.install_display_buffers([Some(first), Some(second)], extent)
                    } else {
                        [Some(first), Some(second)]
                    };
                    self.retire(Retired { _buffers: retired, ..Default::default() })?;
                }
                Err(cause) => {
                    tracing::debug!(operation="canvas_preview_resize_stopped", cause=%cause, "Preview display storage reached a terminal outcome");
                    if completed == latest {
                        self.preview.resize_deadline = Some(self.clock.now());
                    }
                }
            }
        }
        if self.closing
            || self.preview.restoring
            || !self.preview_shown()
            || self.preview.resize_job.is_some()
            || self.preview.candidate.is_some()
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.pending_retirement.is_some()
            || self.deferred_retirement.is_some()
        {
            return Ok(());
        }
        let extent = self.document.tools.navigation().viewport().surface();
        if !self.needs_display_buffers(extent) {
            return Ok(());
        }
        let deadline = match self.preview.resize_deadline {
            Some(deadline) => deadline,
            None => {
                let deadline = self.clock.now().checked_add(Duration::from_secs(30))?;
                self.preview.resize_deadline = Some(deadline);
                self.preview.resize_extent = Some(extent);
                deadline
            }
        };
        if self.clock.now() >= deadline {
            return Ok(());
        }
        let next = self.frame_revision.checked_add(1).ok_or("Canvas display revision exhausted")?;
        let mut spec = self.spec(
            Operation::FramePrepare,
            JobClass::CpuInteractive,
            2 * 1024 * 1024,
            Duration::from_secs(30),
            Some(ReplacementKey {
                surface: SurfaceId::new(PREVIEW_RESIZE_SURFACE)?,
                revision: Generation::new(next)?,
            }),
        )?;
        spec.deadline = Some(deadline);
        let mut input = Some(BuffersTask {
            resources: self
                .preview
                .resources
                .as_ref()
                .ok_or("Preview resize resources are unavailable")?
                .clone(),
            extent,
        });
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime
            .client
            .submit_cpu_owned_input(&runtime.scope, spec, &mut input, |task, _| task)
        {
            Ok(job) => {
                self.frame_revision = next;
                self.preview.resize_job = Some(job);
                self.preview.resize_job_intent = Some((extent, deadline));
            }
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
