// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt native observations to document/camera intent without locking image workers.
use super::*;
use sugarsubstitute_canvas::ToolCancelReason;
use sugarsubstitute_desktop::canvas::{
    CpuFrame, Feedback, GpuPresentation, Input, Lifetime, WindowTarget,
};
use sugarsubstitute_image_ops::Extent;

/// Standalone viewer storage beside its canvas reservation: source imports, graphics device
/// preparation and retirement work.
const STANDALONE_WORKING_BYTES: u64 = 224 * 1024 * 1024;

impl Session {
    /// Borrow pixels only when they represent the latest selected source and viewport.
    /// An older acknowledged projection remains available to the standalone viewer while a
    /// replacement loads, but it cannot acknowledge the replacement on an embedded surface.
    #[cfg(feature = "native-composition")]
    pub(crate) fn selected_cpu_frame(
        &self,
    ) -> Option<(CpuFrame<'_>, &sugarsubstitute_contracts::execution::MemoryLease)> {
        if let Some(frame) = self.preview_cpu_frame() {
            return Some(frame);
        }
        if self.closing
            || self.failed.is_some()
            || self.pending_open.is_some()
            || self.loading.is_some()
        {
            return None;
        }
        let scene = self.document.scene.as_ref()?;
        if scene.source.is_none() || scene.source_revision != self.selection {
            return None;
        }
        let frame = self.frame()?;
        let evidence = if frame.pending {
            self.document.pending_cpu_evidence?
        } else {
            self.document.display_evidence?
        };
        let snapshot = scene.document.snapshot();
        if evidence.target != frame.target
            || evidence.source_revision != self.selection
            || evidence.document != (snapshot.identity(), snapshot.revision())
            || evidence.view.surface() != self.document.tools.navigation().viewport().surface()
        {
            return None;
        }
        Some((frame, &self.resources.as_ref()?.presentation))
    }

    /// Report a retained failure without transferring its original cause or source custody.
    #[cfg(feature = "native-composition")]
    pub(crate) fn source_failed(&self) -> bool {
        #[cfg(windows)]
        let graphics_failed = self.host_graphics_failure.is_some();
        #[cfg(not(windows))]
        let graphics_failed = false;
        self.failed.is_some() || graphics_failed
    }
}

impl Lifetime for Session {
    fn attach_window(&mut self, target: WindowTarget) -> Result<(), Box<dyn Error>> {
        if self.config.gpu.is_some() {
            self.attach_presentation(attachment::Presentation::GpuWindow(target))
        } else {
            self.attach_cpu_surface()
        }
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
            .document
            .pending_gpu_evidence
            .take()
            .filter(|evidence| evidence.target == target)
            .ok_or("Canvas GPU presentation evidence is unavailable")?;
        self.pacing.native_accepted(target, self.navigation_time())?;
        self.awaiting_gpu = None;
        self.accept_display(evidence, true)?;
        self.document.native_presented = Some(target);
        Ok(())
    }
    fn did_present_cpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        if self.accept_preview_cpu(target)? {
            // CPU pixels replaced any shown shared texture; its ring slot may be reused.
            #[cfg(windows)]
            {
                self.composition_shown = None;
            }
            return Ok(());
        }
        if self.document.cpu_pending != Some(target) {
            return Err("Canvas received a foreign CPU presentation acknowledgement".into());
        }
        let frame =
            self.document.cpu_projection.as_ref().ok_or("CPU projection custody is unavailable")?;
        let (source, view) = (frame.source(), frame.view().target());
        let evidence = self
            .document
            .pending_cpu_evidence
            .take()
            .filter(|evidence| evidence.target == target)
            .ok_or("Canvas CPU presentation evidence is unavailable")?;
        if evidence.source != source || evidence.view != view {
            return Err("Canvas CPU presentation evidence differs from projection custody".into());
        }
        self.pacing.native_accepted(target, self.navigation_time())?;
        self.document.cpu_pending = None;
        self.accept_display(evidence, true)?;
        self.document.native_presented = Some(target);
        #[cfg(all(windows, feature = "native-composition"))]
        {
            self.composition_shown = None;
        }
        Ok(())
    }
    fn expose(&mut self) {
        if !self.closing
            && self.drawable
            && self.config.gpu.is_some()
            && self.document.native_presented.is_some()
        {
            self.expose_pending = true;
            self.input_observed = self.navigation_time();
        }
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() || self.starting_owner.is_some() || self.closing {
            return Err("Canvas execution cannot be started".into());
        }
        let capacity = budget::reservation_bytes(
            self.config.maximum_extent,
            self.config.gpu.is_some(),
            self.config.retained_document_bytes,
        )
        .saturating_add(STANDALONE_WORKING_BYTES);
        self.starting_owner = Some(ExecutionOwner::with_memory_capacity(
            self.clock.clone(),
            wake,
            std::num::NonZeroU64::new(capacity).ok_or("Invalid canvas execution capacity")?,
        )?);
        let spec = ScopeSpec { id: ScopeId::new(1)?, deadline: None };
        self.start_owned(spec)
    }
    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        self.resize_preview_candidate(extent)?;
        let previous = self.document.tools.navigation().viewport().surface();
        let now = self.navigation_time();
        if previous != extent {
            self.pending_cpu_withdrawal(now)?;
            #[cfg(all(windows, feature = "native-composition"))]
            self.abandon_composition_frame(now)?;
        }
        self.document.tools.resize(
            extent,
            self.document.scene.as_ref().map(|scene| &scene.document),
            now,
        )?;
        if previous != extent {
            self.abandon_cpu_frame(now)?;
        }
        self.input_observed = now;
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        let now = self.navigation_time();
        let before = (
            self.document.tools.navigation().viewport(),
            self.document.tools.navigation().target_viewport(),
        );
        match input {
            Input::Drawable(available) => {
                self.drawable = available;
                if available {
                    self.expose();
                }
            }
            Input::Tool(input) => self.document.tools.input(
                input,
                self.document.scene.as_mut().map(|scene| &mut scene.document),
                now,
            )?,
            Input::TemporaryNavigation(held) => self.document.tools.temporary_navigation(
                held,
                self.document.scene.as_mut().map(|scene| &mut scene.document),
                now,
            )?,
            Input::DisplayRefresh(rate) => {
                self.document.tools.set_refresh_rate(rate)?;
                self.display_refresh = rate;
            }
            Input::Open => self.picker = true,
            Input::Unfocused => self.document.tools.cancel(
                ToolCancelReason::FocusLost,
                self.document.scene.as_ref().map(|scene| &scene.document),
                now,
            )?,
        }
        if before
            != (
                self.document.tools.navigation().viewport(),
                self.document.tools.navigation().target_viewport(),
            )
        {
            self.input_observed = now;
        }
        if self.pacing.period() != self.document.tools.navigation().frame_period() {
            self.pacing.set_frame_period(self.document.tools.navigation().frame_period(), now)?;
        }
        self.cancel_obsolete_refinement()?;
        Ok(())
    }
    fn feedback(&self) -> Feedback {
        Feedback {
            cursor: self
                .document
                .tools
                .cursor(self.document.scene.as_ref().map(|scene| &scene.document)),
            capture: self.document.tools.captured(),
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
        if self.gpu_document() {
            return None;
        }
        #[cfg(feature = "native-composition")]
        if let Some((frame, _)) = self.preview_cpu_frame() {
            return Some(frame);
        }
        self.document.cpu_frame()
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
        let navigation = self.document.tools.navigation();
        let motion = navigation
            .next_deadline(now)
            .and_then(|cutoff| native_now.checked_add(cutoff.saturating_sub(now)));
        let presentation = if !self.drawable {
            None
        } else {
            match self.display_deadline(now) {
                Ok(Some(deadline)) => native_now.checked_add(deadline.saturating_sub(now)),
                Ok(None)
                    if self.config.gpu.is_some()
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
        #[cfg(feature = "native-composition")]
        let presentation = {
            let preview = match self.preview_deadline(now) {
                Ok(deadline) => deadline
                    .and_then(|deadline| native_now.checked_add(deadline.saturating_sub(now))),
                Err(cause) => {
                    tracing::error!(operation="canvas_preview_deadline", cause=%cause, "Canvas preview pacing is invalid");
                    None
                }
            };
            let autosave = self.input_autosave_deadline().and_then(|deadline| {
                let wait = deadline.checked_duration_since(self.clock.now()).unwrap_or_default();
                native_now.checked_add(wait)
            });
            [presentation, preview, autosave].into_iter().flatten().min()
        };
        match (motion, presentation) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        let deadline = match self.shutdown_deadline {
            Some(deadline) => deadline,
            None => {
                sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(self.clock.now())?
                    .settle_until
            }
        };
        self.close_until(deadline)
    }
    fn is_closing(&self) -> bool {
        self.closing
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        self.advance()?;
        let settled = if !self.shutdown_started {
            false
        } else if let Some(runtime) = &mut self.runtime {
            runtime.settled()?
        } else if let Some(owner) = &mut self.starting_owner {
            owner.poll_shutdown()?.settled
        } else {
            true
        };
        if !settled
            && !self.shutdown_expiry_reported
            && self.shutdown_deadline.is_some_and(|deadline| self.clock.now() >= deadline)
        {
            self.shutdown_expiry_reported = true;
            return Err("Canvas shutdown exceeded its original deadline".into());
        }
        Ok(settled)
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.gpu_diagnostics = None;
        if let Some(runtime) = &mut self.runtime {
            runtime.finish()?;
        }
        self.runtime = None;
        if let Some(owner) = &mut self.starting_owner {
            if !self.shutdown_started || !owner.poll_shutdown()?.settled {
                return Err("Canvas execution has not settled".into());
            }
            owner.finish_shutdown()?;
        }
        self.starting_owner = None;
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
        let scene =
            self.document.scene.as_ref().ok_or("Canvas display evidence has no document")?;
        let snapshot = scene.document.snapshot();
        if evidence.document != (snapshot.identity(), snapshot.revision())
            || evidence.source_revision != scene.source_revision
            || !evidence.proves_complete(snapshot.extent())?
        {
            return Err("Canvas display evidence does not prove complete current coverage".into());
        }
        self.document.display_source = Some(evidence.source);
        self.document.display_view = Some(evidence.view);
        self.document.display_evidence = Some(evidence);
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
                && prepared.zoom() < self.document.tools.navigation().viewport().zoom()
                && prepared.zoom() < self.document.tools.navigation().target_viewport().zoom()
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
        let current = if let Some(scene) = &self.document.scene {
            let snapshot = scene.document.snapshot();
            let target = self.document.tools.navigation().viewport();
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
