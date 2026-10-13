// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route one real completion batch before driving current application effects.
use super::*;
use policy::StartupRoute;
use sugarsubstitute_contracts::startup::StartupEffect;

impl ProductionStartup {
    /// Every recipient sees the same immutable bounded terminal batch exactly once.
    /// Pending typed disposal remains with each actual owner even when another fails.
    /// # Errors
    /// Retain genuine recipient causes and reject stale or unconsumed application effects.
    pub(super) fn consume(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        if !self.started || !self.execution.has_owner() {
            return Ok(None);
        }
        let batch = self.execution.drain(maximum)?;
        let render = self.preparation.as_mut().map(|render| render.consume(&batch)).transpose();
        let child = self.child.consume(&batch, self.startup.as_ref());
        let frame = super::delivery::resolve(render, child)?;
        for _ in 0..32 {
            let Some(startup) = &mut self.startup else { break };
            let Some(event) = self.child.take_startup_event()? else {
                break;
            };
            let route = self.policy.observe(event, startup.session_mut())?;
            self.route(route)?;
            self.invalidate(true);
        }
        self.refresh_deadline()?;
        Ok(frame.flatten().filter(|_| !self.closing))
    }
    /// Dispatch the one application decision to native UI or the actual child authority owner.
    /// # Errors
    /// Return rejection before replacing an unconsumed UI command or manufacturing authority.
    pub(super) fn route(&mut self, route: StartupRoute) -> Result<(), Box<dyn Error>> {
        match route {
            StartupRoute::Native(observation) => {
                if matches!(&observation, DesktopStartupObservation::NativeRetired(run) if *run == self.policy.run())
                    && let Some(startup) = &mut self.startup
                {
                    startup.release_source();
                }
                self.child.observe(observation, self.startup.as_ref())?;
            }
            StartupRoute::Close => self.close_owned()?,
            StartupRoute::None => {}
            StartupRoute::Effect(effect) => match effect {
                StartupEffect::RetireSplash { launch } => self.commands.retire_splash(launch)?,
                StartupEffect::RevealMain { .. } => {
                    let startup = self
                        .startup
                        .as_ref()
                        .ok_or_else(|| std::io::Error::other("Actual startup policy absent"))?;
                    self.commands.reveal_main(self.policy.identity(startup.session()))?
                }
                effect => {
                    let prepared = self.startup.as_mut().ok_or_else(|| {
                        std::io::Error::other("Actual startup policy absent for application effect")
                    })?;
                    self.child.apply(effect, prepared)?;
                }
            },
        }
        Ok(())
    }
    /// Map the actual owner epoch; failed arithmetic is returned rather than hidden.
    pub(super) fn refresh_deadline(&mut self) -> Result<(), Box<dyn Error>> {
        self.next_deadline =
            self.child.next_deadline().map(|at| self.clock.instant_for(at)).transpose()?;
        Ok(())
    }
}
