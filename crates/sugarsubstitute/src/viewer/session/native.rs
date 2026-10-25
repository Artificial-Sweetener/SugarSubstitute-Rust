// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt native observations to document/camera intent without locking image workers.
use super::*;
use sugarsubstitute_desktop::canvas::{
    CpuFrame, GpuPresentation, Input, Lifetime, WheelDelta, WindowTarget,
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
        self.view.resize(extent)?;
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        let preceding = self.view;
        match input {
            Input::Pointer(position) => {
                if position.iter().any(|value| !value.is_finite()) {
                    return Err("Invalid native canvas pointer coordinate".into());
                }
                if self.dragging {
                    self.view
                        .pan_by([position[0] - self.pointer[0], position[1] - self.pointer[1]])?;
                }
                self.pointer = position;
            }
            Input::Pan(active) => self.dragging = active,
            Input::Wheel(delta) => {
                let amount = match delta {
                    WheelDelta::Lines(delta) => delta[1],
                    WheelDelta::Pixels(delta) => delta[1] / 120.0,
                };
                if !amount.is_finite() {
                    return Err("Invalid native canvas wheel delta".into());
                }
                self.view.zoom_at((amount * 0.25).clamp(-32.0, 32.0).exp2(), self.pointer)?;
            }
            Input::Fit => {
                if let Some(scene) = &self.scene {
                    self.view.fit(scene.document.snapshot().extent())?;
                }
            }
            Input::ActualSize => self.view.actual_size(self.pointer)?,
            Input::Open => self.picker = true,
            Input::Unfocused => self.dragging = false,
        }
        let changed = self.view != preceding;
        if self.view.zoom() != preceding.zoom()
            || (changed
                && self.presented.as_ref().is_some_and(|shown| shown.filter == Filter::Nearest))
        {
            self.navigation_until = Some(self.clock.now().checked_add(Duration::from_millis(120))?);
        }
        if changed
            && self.filter() == Filter::Nearest
            && let Some(rendering) = &mut self.rendering
            && rendering.filter == Filter::Lanczos3
        {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        Ok(())
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
        if self.options.gpu.is_some() && self.expose_pending && !self.closing {
            return std::time::Instant::now().checked_add(Duration::from_millis(16));
        }
        let cutoff = self.navigation_until?;
        if self.clock.now() >= cutoff {
            return None;
        }
        std::time::Instant::now().checked_add(Duration::from_nanos(
            cutoff.as_nanos().saturating_sub(self.clock.now().as_nanos()),
        ))
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.closing = true;
        self.initialization_pending = false;
        self.device_pending = false;
        self.native_target = None;
        self.picker = false;
        self.pending_open = None;
        self.dragging = false;
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
