// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt native observations to document/camera intent without locking image workers.
use super::*;
use sugarsubstitute_canvas::ToolCancelReason;
use sugarsubstitute_desktop::canvas::{
    CpuFrame, Feedback, GpuPresentation, Input, Lifetime, WindowTarget,
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
        let target = self.gpu_front.as_ref()?.current()?;
        let frame = self.gpu_presentation.take()?;
        self.awaiting_gpu = Some(target);
        Some(GpuPresentation { target, frame })
    }
    fn did_present_gpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        if self.awaiting_gpu != Some(target) || self.current_frame() != Some(target) {
            return Err("Canvas received a foreign GPU presentation acknowledgement".into());
        }
        self.awaiting_gpu = None;
        self.native_presented = Some(target);
        self.expose_pending = false;
        Ok(())
    }
    fn expose(&mut self) {
        if self.options.gpu.is_some() {
            self.expose_pending = true;
        }
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() {
            return Err("Canvas execution is already started".into());
        }
        let owner = if self.options.gpu.is_some() {
            ExecutionOwner::with_memory_capacity(
                self.clock.clone(),
                wake,
                std::num::NonZeroU64::new(512 * 1024 * 1024)
                    .ok_or("Invalid GPU execution capacity")?,
            )?
        } else {
            ExecutionOwner::new(self.clock.clone(), wake)?
        };
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
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        let now = self.navigation_time();
        match input {
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
        self.cancel_obsolete_refinement();
        Ok(())
    }
    fn feedback(&self) -> Feedback {
        Feedback {
            cursor: self.tools.cursor(self.scene.as_ref().map(|scene| &scene.document)),
            capture: self.tools.captured(),
        }
    }
    fn take_picker_request(&mut self) -> bool {
        if self.closing
            || self.resources.is_none()
            || (self.front.is_none() && self.gpu_front.is_none())
        {
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
        let surface = self.front.as_ref()?;
        let presented = self.presented.as_ref()?;
        Some(CpuFrame {
            target: surface.current()?,
            base: presented.base,
            damage: &presented.damage,
            pixels: surface.pixels(),
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
        let presentation = if self.options.gpu.is_some() && self.expose_pending {
            native_now.checked_add(navigation.frame_period())
        } else {
            None
        };
        match (motion, presentation) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
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
        if let Some(runtime) = &mut self.runtime {
            runtime.owner.finish_shutdown()?;
        }
        self.runtime = None;
        self.failed.take().map_or(Ok(()), Err)
    }
}

impl Session {
    /// Retain one observed terminal outcome while newer tool intent replaces expensive refinement.
    pub(super) fn cancel_obsolete_refinement(&mut self) {
        if self.filter() == Filter::Nearest
            && let Some(rendering) = &mut self.rendering
            && rendering.filter == Filter::Lanczos3
        {
            rendering.superseded = true;
            rendering.job.cancel();
        }
    }
}
