// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt native observations to document/camera intent without locking image workers.
use super::*;
use sugarsubstitute_desktop::canvas::{CpuFrame, Input, Lifetime, WheelDelta};
use sugarsubstitute_image_ops::Extent;

impl Lifetime for Session {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.runtime.is_some() {
            return Err("Canvas execution is already started".into());
        }
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
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
        if self.view.zoom() != preceding.zoom() {
            self.navigation_until = Some(self.clock.now().checked_add(Duration::from_millis(120))?);
        }
        Ok(())
    }
    fn take_picker_request(&mut self) -> bool {
        if self.closing || self.resources.is_none() || self.front.is_none() {
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
        self.picker = false;
        self.pending_open = None;
        self.dragging = false;
        if let Some(job) = &self.initializing {
            job.cancel();
        }
        if let Some(job) = &self.loading {
            job.job.cancel();
        }
        if let Some(job) = &self.rendering {
            job.cancel();
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
