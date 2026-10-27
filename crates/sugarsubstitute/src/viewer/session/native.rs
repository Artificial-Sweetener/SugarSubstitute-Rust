// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt native observations to document/camera intent without locking image workers.
use super::*;
use sugarsubstitute_canvas::ToolCancelReason;
use sugarsubstitute_desktop::canvas::{
    CpuFrame, CpuFrameKind, Feedback, GpuPresentation, Input, Lifetime, WindowTarget,
};
use sugarsubstitute_image_ops::Extent;

impl Lifetime for Session {
    fn attach_window(&mut self, target: WindowTarget) -> Result<(), Box<dyn Error>> {
        if self.native_target.is_some() {
            return Err("Canvas native peer was already attached".into());
        }
        self.native_target = Some(target);
        Ok(())
    }
    fn take_gpu_presentation(&mut self) -> Option<GpuPresentation> {
        let presentation = self.gpu_presentation.take()?;
        self.awaiting_gpu = Some(presentation.target);
        Some(presentation)
    }
    fn did_present_gpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        if self.awaiting_gpu != Some(target) {
            return Err("Canvas received a foreign GPU presentation acknowledgement".into());
        }
        let evidence = self
            .pending_gpu_evidence
            .take()
            .filter(|evidence| evidence.target == target)
            .ok_or("Canvas GPU presentation evidence is unavailable")?;
        self.pacing.native_accepted(target, self.navigation_time())?;
        self.awaiting_gpu = None;
        self.accept_display(evidence, true)?;
        self.native_presented = Some(target);
        Ok(())
    }
    fn did_present_cpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        if self.cpu_pending != Some(target) {
            return Err("Canvas received a foreign CPU presentation acknowledgement".into());
        }
        let frame = self.cpu_projection.as_ref().ok_or("CPU projection custody is unavailable")?;
        let (source, view) = (frame.source(), frame.view().target());
        let evidence = self
            .pending_cpu_evidence
            .take()
            .filter(|evidence| evidence.target == target)
            .ok_or("Canvas CPU presentation evidence is unavailable")?;
        if evidence.source != source || evidence.view != view {
            return Err("Canvas CPU presentation evidence differs from projection custody".into());
        }
        self.pacing.native_accepted(target, self.navigation_time())?;
        self.cpu_pending = None;
        self.accept_display(evidence, true)?;
        self.native_presented = Some(target);
        Ok(())
    }
    fn expose(&mut self) {
        if !self.closing
            && self.drawable
            && self.options.gpu.is_some()
            && self.native_presented.is_some()
        {
            self.expose_pending = true;
            self.input_observed = self.navigation_time();
        }
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() {
            return Err("Canvas execution is already started".into());
        }
        let capacity = if self.options.gpu.is_some() { 512 } else { 384 };
        let owner = ExecutionOwner::with_memory_capacity(
            self.clock.clone(),
            wake,
            std::num::NonZeroU64::new(capacity * 1024 * 1024)
                .ok_or("Invalid canvas execution capacity")?,
        )?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        self.runtime = Some(Runtime { owner, client, scope });
        self.initialization_pending = true;
        self.advance()
    }
    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>> {
        self.tools.resize(
            extent,
            self.scene.as_ref().map(|scene| &scene.document),
            self.navigation_time(),
        )?;
        self.input_observed = self.navigation_time();
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        let now = self.navigation_time();
        let before =
            (self.tools.navigation().viewport(), self.tools.navigation().target_viewport());
        match input {
            Input::Drawable(available) => {
                self.drawable = available;
                if available {
                    self.expose();
                }
            }
            Input::Tool(input) => self.tools.input(
                input,
                self.scene.as_mut().map(|scene| &mut scene.document),
                now,
            )?,
            Input::TemporaryNavigation(held) => self.tools.temporary_navigation(
                held,
                self.scene.as_ref().map(|scene| &scene.document),
                now,
            )?,
            Input::DisplayRefresh(rate) => self.tools.set_refresh_rate(rate)?,
            Input::Open => self.picker = true,
            Input::Unfocused => self.tools.cancel(
                ToolCancelReason::FocusLost,
                self.scene.as_ref().map(|scene| &scene.document),
                now,
            )?,
        }
        if before != (self.tools.navigation().viewport(), self.tools.navigation().target_viewport())
        {
            self.input_observed = now;
        }
        if self.pacing.period() != self.tools.navigation().frame_period() {
            self.pacing.set_frame_period(self.tools.navigation().frame_period(), now)?;
        }
        self.cancel_obsolete_refinement()?;
        Ok(())
    }
    fn feedback(&self) -> Feedback {
        Feedback {
            cursor: self.tools.cursor(self.scene.as_ref().map(|scene| &scene.document)),
            capture: self.tools.captured(),
        }
    }
    fn take_picker_request(&mut self) -> bool {
        if self.closing || self.resources.is_none() {
            return false;
        }
        std::mem::take(&mut self.picker)
    }
    fn selected(
        &mut self,
        selection: Result<Option<PathBuf>, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        match selection {
            Ok(Some(path)) => self.open(path, CacheReuse::Compatible, None)?,
            Ok(None) => {}
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    fn poll(&mut self, _: Duration) -> Result<(), Box<dyn Error>> {
        if let Some(diagnostics) = &self.gpu_diagnostics {
            diagnostics.report();
        }
        self.advance()?;
        if !self.closing {
            self.health.check()?;
        }
        Ok(())
    }
    fn frame(&self) -> Option<CpuFrame<'_>> {
        if self.options.gpu.is_some() {
            return None;
        }
        let frame = self.cpu_projection.as_ref()?;
        Some(CpuFrame {
            pending: self.cpu_pending == Some(frame.target()),
            kind: CpuFrameKind::Projected,
            target: frame.target(),
            base: self.projection_base,
            damage: self.projection_damage.as_ref()?,
            pixels: frame.pixels(),
        })
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn next_deadline(&self) -> Option<std::time::Instant> {
        if self.closing {
            return None;
        }
        let now = self.navigation_time();
        let native_now = std::time::Instant::now();
        let navigation = self.tools.navigation();
        let motion = navigation
            .next_deadline(now)
            .and_then(|cutoff| native_now.checked_add(cutoff.saturating_sub(now)));
        let presentation = if !self.drawable {
            None
        } else {
            match self.display_deadline(now) {
                Ok(Some(deadline)) => native_now.checked_add(deadline.saturating_sub(now)),
                Ok(None)
                    if self.options.gpu.is_some()
                        && self.expose_pending
                        && self.displaying.is_none()
                        && !self.gpu_presentation_pending() =>
                {
                    native_now.checked_add(navigation.frame_period())
                }
                Ok(None) => None,
                Err(cause) => {
                    tracing::error!(operation="canvas_display_deadline", cause=%cause, "Canvas display selection is invalid");
                    None
                }
            }
        };
        match (motion, presentation) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        if let PresentationReadiness::Busy {
            target,
            stage: sugarsubstitute_canvas_render::PresentationStage::AwaitingNative,
        } = self.pacing.readiness(now)?
        {
            self.pacing.abandoned(target, now)?;
        }
        self.cpu_pending = None;
        self.awaiting_gpu = None;
        self.pending_cpu_evidence = None;
        self.pending_gpu_evidence = None;
        self.projection_base = None;
        self.closing = true;
        self.initialization_pending = false;
        self.device_pending = false;
        self.native_target = None;
        self.picker = false;
        self.pending_open = None;
        self.tools.cancel(
            ToolCancelReason::Shutdown,
            self.scene.as_ref().map(|scene| &scene.document),
            self.navigation_time(),
        )?;
        if let Some(job) = &self.initializing {
            job.cancel();
        }
        if let Some(job) = &self.device_job {
            job.cancel();
        }
        if let Some(job) = &self.loading {
            job.job.cancel();
        }
        if let Some(rendering) = &self.rendering {
            rendering.job.cancel();
        }
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        Ok(())
    }
    fn is_closing(&self) -> bool {
        self.closing
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        self.advance()?;
        if !self.shutdown_started {
            return Ok(false);
        }
        match &self.runtime {
            Some(runtime) => Ok(runtime.owner.poll_shutdown()?.settled),
            None => Ok(true),
        }
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.gpu_diagnostics = None;
        if let Some(runtime) = &mut self.runtime {
            runtime.owner.finish_shutdown()?;
        }
        self.runtime = None;
        self.failed.take().map_or(Ok(()), Err)
    }
}

impl Session {
    /// Commit immutable source and coverage evidence only at the matching native acceptance.
    pub(super) fn accept_display(
        &mut self,
        evidence: DisplayEvidence,
        native_acknowledged: bool,
    ) -> Result<(), Box<dyn Error>> {
        let scene = self.scene.as_ref().ok_or("Canvas display evidence has no document")?;
        let snapshot = scene.document.snapshot();
        if evidence.document != (snapshot.identity(), snapshot.revision())
            || evidence.source_revision != scene.source_revision
            || !evidence.proves_complete(snapshot.extent())?
        {
            return Err("Canvas display evidence does not prove complete current coverage".into());
        }
        self.display_source = Some(evidence.source);
        self.display_view = Some(evidence.view);
        self.display_evidence = Some(evidence);
        if native_acknowledged
            && self.first_image_queue_latency.is_none()
            && let Some((source_revision, intent)) = self.pending_first_image
            && evidence.source_revision == source_revision
        {
            self.first_image_queue_latency = Some(
                self.navigation_time()
                    .checked_sub(intent)
                    .ok_or("Canvas native acknowledgement preceded source intent")?,
            );
            self.pending_first_image = None;
        }
        self.coverage_acceptances = self
            .coverage_acceptances
            .checked_add(1)
            .ok_or("Canvas coverage acceptance count exhausted")?;
        if evidence.tier == BackingTier::Fallback || evidence.underlay.is_some() {
            self.fallback_projection_acceptances = self
                .fallback_projection_acceptances
                .checked_add(1)
                .ok_or("Canvas fallback acceptance count exhausted")?;
        }
        if evidence.tier == BackingTier::Navigation
            || evidence.underlay.is_some_and(|underlay| underlay.tier == BackingTier::Navigation)
        {
            self.navigation_projection_acceptances = self
                .navigation_projection_acceptances
                .checked_add(1)
                .ok_or("Canvas navigation acceptance count exhausted")?;
        }
        if evidence.tier == BackingTier::Fallback
            || evidence.underlay.is_some_and(|underlay| underlay.tier == BackingTier::Fallback)
        {
            self.bootstrap_projection_acceptances = self
                .bootstrap_projection_acceptances
                .checked_add(1)
                .ok_or("Canvas bootstrap acceptance count exhausted")?;
        }
        Ok(())
    }

    /// Retain one observed terminal outcome while newer tool intent replaces expensive refinement.
    pub(super) fn cancel_obsolete_refinement(&mut self) -> Result<(), Box<dyn Error>> {
        let obsolete_coarser = self.rendering.as_ref().is_some_and(|rendering| {
            let prepared = rendering.coverage.viewport();
            rendering.tier == BackingTier::Detail
                && prepared.zoom() < self.tools.navigation().viewport().zoom()
                && prepared.zoom() < self.tools.navigation().target_viewport().zoom()
                && self.has_finer_ready_detail(prepared)
        });
        let desired = if self
            .rendering
            .as_ref()
            .is_some_and(|rendering| rendering.tier == BackingTier::Navigation)
        {
            Reconstruction::navigation_for_display_zoom(self.navigation_view().zoom())?
        } else {
            self.reconstruction()?
        };
        let Some(rendering) = &mut self.rendering else {
            return Ok(());
        };
        if rendering.tier == BackingTier::Fallback {
            return Ok(());
        }
        let compatible = matches!(
            (rendering.reconstruction, desired),
            (Reconstruction::Finished(rendering), Reconstruction::Finished(desired))
                if rendering == desired
        );
        let current = if let Some(scene) = &self.scene {
            let snapshot = scene.document.snapshot();
            let target = self.tools.navigation().viewport();
            rendering.display_extent == target.surface()
                && rendering.coverage.overlaps(target, snapshot.extent())?
        } else {
            false
        };
        if !current || !compatible || obsolete_coarser {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        Ok(())
    }
}
