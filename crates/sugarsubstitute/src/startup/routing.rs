// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route one real completion batch before driving current application effects.
use super::consume_diagnostic::observe as observed_consume;
use super::*;
use policy::StartupRoute;
use sugarsubstitute_contracts::startup::StartupEffect;
use sugarsubstitute_desktop::StartupConsumeStage as ConsumeStage;

impl<B: StartupBackend> StartupHost<B> {
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
        if self.view.as_mut().is_some_and(|view| view.take_retry()) {
            let retried = self.retry_owned();
            observed_consume::<B, _>(ConsumeStage::Retry, retried)?;
        }
        let batch =
            observed_consume::<B, _>(ConsumeStage::ExecutionDrain, self.execution.drain(maximum))?;
        let render = self.preparation.as_mut().map(|render| render.consume(&batch)).transpose();
        let cleanup_failed = self.child.cleanup_failed();
        let child = self.child.consume(&batch, self.startup.as_ref());
        self.invalidate(cleanup_failed != self.child.cleanup_failed());
        let frame = super::consume_diagnostic::delivery::<B, _>(render, child)?;
        if let Some(prepared) = &mut self.startup {
            self.child.advance_reload(prepared)?;
        }
        self.deliver_startup_events()?;
        if let Some(view) = &mut self.view {
            if let Some(startup) = &self.startup {
                view.phase(startup.session().phase());
            }
            let changed = view
                .output(self.child.transcript().as_ref().map(crate::OutputSnapshot::transcript));
            self.invalidate(changed);
        }
        let deadline = self.refresh_deadline();
        observed_consume::<B, _>(ConsumeStage::DeadlineRefresh, deadline)?;
        Ok(frame.flatten().filter(|_| !self.closing))
    }
    /// Deliver the original bounded child events before accepting later native evidence.
    pub(super) fn deliver_startup_events(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..32 {
            let Some(startup) = &mut self.startup else {
                break;
            };
            let Some(event) = observed_consume::<B, _>(
                ConsumeStage::StartupEvent,
                self.child.take_startup_event(),
            )?
            else {
                break;
            };
            let phase = startup.session().phase();
            let route = observed_consume::<B, _>(
                ConsumeStage::PolicyObservation,
                self.policy.observe(event, startup.session_mut()),
            )?;
            let dispatched = self.route(route);
            observed_consume::<B, _>(ConsumeStage::EffectRouting, dispatched)?;
            self.invalidate(
                self.startup.as_ref().is_some_and(|startup| startup.session().phase() != phase),
            );
        }
        Ok(())
    }
    /// Advance a user-requested retry only after the sole policy observed settlement.
    fn retry_owned(&mut self) -> Result<(), Box<dyn Error>> {
        let startup = self.startup.as_mut().ok_or("Actual startup owner absent")?;
        let identity = retry_identity(self.policy.identity(startup.session()))?;
        Self::prepare_retry_policy(&self.child, startup.session_mut(), identity)?;
        self.child.retry_attempt(identity, startup)?;
        if let Some(view) = &mut self.view {
            view.phase(sugarsubstitute_application::startup::StartupPhase::SplashFirstFrame);
        }
        self.invalidate(true);
        Ok(())
    }
    /// Preflight actual child custody before replacing the application's retained failed attempt.
    /// The transfer owner revalidates afterward; neither copied identity nor elapsed time settles it.
    pub(crate) fn prepare_retry_policy(
        child: &B,
        session: &mut sugarsubstitute_application::startup::StartupSession,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(), Box<dyn Error>> {
        child.ensure_retry_ready(identity)?;
        Ok(session.retry_configured(identity.launch, identity.navigation)?)
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
                if let Some(prepared) = &mut self.startup {
                    self.child.advance_reload(prepared)?;
                }
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

/// Advance both authorization generations before mutating a settled attempt.
/// Exhaustion retains the current identity; it never wraps or reuses an old navigation.
fn retry_identity(
    current: sugarsubstitute_contracts::startup::MainIdentity,
) -> Result<sugarsubstitute_contracts::startup::MainIdentity, Box<dyn Error>> {
    Ok(sugarsubstitute_contracts::startup::MainIdentity {
        run: current.run,
        launch: LaunchId::new(current.launch.get().checked_add(1).ok_or("Launch exhausted")?)?,
        navigation: sugarsubstitute_contracts::execution::Generation::new(
            current.navigation.get().checked_add(1).ok_or("Navigation exhausted")?,
        )?,
    })
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
