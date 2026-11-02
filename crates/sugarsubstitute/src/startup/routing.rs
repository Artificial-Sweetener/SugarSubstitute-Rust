// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route one real completion batch before driving current application effects.
use super::consume_diagnostic::observe as observed_consume;
use super::*;
use policy::StartupRoute;
#[cfg(all(windows, feature = "native-composition"))]
use sugarsubstitute_contracts::execution::Clock;
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
        #[cfg(all(windows, feature = "native-composition"))]
        let composition = self
            .composition
            .as_mut()
            .map_or(Ok(()), |composition| composition.advance(self.clock.now()));
        #[cfg(not(all(windows, feature = "native-composition")))]
        let composition = Ok(());
        #[cfg(all(windows, feature = "native-composition"))]
        self.govern_memory();
        #[cfg(windows)]
        let canvas = self.canvas.as_mut().map_or(Ok(()), canvas::EmbeddedCanvas::poll);
        #[cfg(not(windows))]
        let canvas = Ok(());
        let render = self.preparation.as_mut().map(|render| render.consume(&batch)).transpose();
        let cleanup_failed = self.child.cleanup_failed();
        let child = self.child.consume(&batch, self.startup.as_ref());
        self.invalidate(cleanup_failed != self.child.cleanup_failed());
        let canvas = super::delivery::resolve(canvas, composition);
        let frame = super::consume_diagnostic::delivery::<B, _>(render, child, canvas)?;
        if let Some(prepared) = &mut self.startup {
            self.child.advance_reload(prepared)?;
        }
        self.deliver_startup_events()?;
        #[cfg(all(windows, feature = "native-composition"))]
        self.refresh_canvas_backend()?;
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
                #[cfg(all(windows, feature = "native-composition"))]
                if let DesktopStartupObservation::MainRetired(identity)
                | DesktopStartupObservation::MainAbsent(identity) = &observation
                {
                    if let Some(composition) = &mut self.composition {
                        composition.main_retired(*identity)?;
                    }
                    if let Some(canvas) = &mut self.canvas {
                        canvas.main_retired(*identity)?;
                    }
                }
                #[cfg(all(windows, feature = "native-composition"))]
                let observation = {
                    let accepts = !self.policy.closing()
                        && self.canvas.is_some()
                        && self.startup.as_ref().is_some_and(|startup| {
                            startup.session().phase()
                                == sugarsubstitute_application::startup::StartupPhase::Running
                        });
                    match observation {
                        DesktopStartupObservation::SurfaceMessage(observed)
                            if grants_native_canvas(accepts, observed.hosting()) =>
                        {
                            let allocation = observed.allocation();
                            let (identity, message, lease, claim) = observed.into_parts();
                            if self.child.admits_canvas(identity, &lease) {
                                let backend = self.child.canvas_backend(identity);
                                let canvas =
                                    self.canvas.as_mut().ok_or("Application canvas disappeared")?;
                                canvas.backend(backend)?;
                                canvas.surface(identity, message, allocation, claim)?;
                            } else {
                                self.canvas
                                    .as_mut()
                                    .ok_or("Application canvas disappeared")?
                                    .refuse_surface(identity, message, claim)?;
                            }
                            return Ok(());
                        }
                        DesktopStartupObservation::CanvasInput(input) if accepts => {
                            let canvas =
                                self.canvas.as_mut().ok_or("Application canvas disappeared")?;
                            if self
                                .child
                                .canvas_backend(input.identity)
                                .is_some_and(|backend| canvas.matches_backend(backend))
                                && canvas.input(input)? == canvas::RoutedInput::Refused
                            {
                                tracing::debug!(operation = "canvas_input_refused", identity = ?input.identity,
                                    "Routed canvas gesture arrived after its region stopped qualifying");
                            }
                            return Ok(());
                        }
                        // Pacing grants nothing, and the display is reported once per display
                        // change, usually before startup runs or any backend grant exists, so
                        // only the canvas's existence is required.
                        DesktopStartupObservation::CanvasDisplay(display)
                            if self.canvas.is_some() =>
                        {
                            self.canvas
                                .as_mut()
                                .ok_or("Application canvas disappeared")?
                                .display(display.hertz)?;
                            return Ok(());
                        }
                        DesktopStartupObservation::CanvasContextMessage(observed)
                            if grants_native_canvas(accepts, observed.hosting()) =>
                        {
                            let (identity, request, lease, claim) = observed.into_parts();
                            if self.child.admits_canvas(identity, &lease) {
                                let backend = self.child.canvas_backend(identity);
                                let canvas =
                                    self.canvas.as_mut().ok_or("Application canvas disappeared")?;
                                canvas.backend(backend)?;
                                canvas.context(identity, request, claim)?;
                            } else {
                                self.canvas
                                    .as_mut()
                                    .ok_or("Application canvas disappeared")?
                                    .refuse_context(identity, &request, claim)?;
                                #[cfg(feature = "k1-native-qualification")]
                                sugarsubstitute_desktop::qualification_trace::progress(
                                    sugarsubstitute_desktop::qualification_trace::Progress::CanvasContextRejected,
                                    identity,
                                );
                            }
                            return Ok(());
                        }
                        other => other,
                    }
                };
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

    /// Fence output work whenever the parent revokes or changes the mounted backend.
    #[cfg(all(windows, feature = "native-composition"))]
    fn refresh_canvas_backend(&mut self) -> Result<(), Box<dyn Error>> {
        let backend = self
            .startup
            .as_ref()
            .and_then(|startup| self.child.canvas_backend(self.policy.identity(startup.session())));
        if let Some(canvas) = &mut self.canvas {
            canvas.backend(backend)?;
        }
        Ok(())
    }
    /// The canvas's refinement deadline and the memory budget's next sample, whichever is first.
    pub(super) fn owned_deadlines(&self) -> Option<Instant> {
        #[cfg(windows)]
        {
            let canvas = self.canvas.as_ref().and_then(canvas::EmbeddedCanvas::next_deadline);
            #[cfg(feature = "native-composition")]
            let memory = self
                .memory
                .as_ref()
                .filter(|_| !self.closing)
                .map(super::memory::MemoryGovernor::next_deadline);
            #[cfg(not(feature = "native-composition"))]
            let memory = None;
            canvas.into_iter().chain(memory).min()
        }
        #[cfg(not(windows))]
        None
    }

    /// Sample memory when due and narrow or widen what retained canvas documents may keep.
    #[cfg(all(windows, feature = "native-composition"))]
    fn govern_memory(&mut self) {
        let (Some(memory), Some(canvas)) = (&mut self.memory, &mut self.canvas) else {
            return;
        };
        if self.closing {
            return;
        }
        let charge = sugarsubstitute_execution::memory_budget::CacheCharge {
            native_bytes: canvas.retained_bytes(),
            ..Default::default()
        };
        match memory.poll(std::time::Instant::now(), self.clock.now(), charge) {
            Ok(Some(decision)) => canvas.limit_retained(decision.native_bytes),
            Ok(None) => {}
            // A failed sample keeps the previous allowance; the canvas itself stays healthy.
            Err(error) => tracing::warn!(operation = "memory_budget_sample", %error,
                "System memory could not be sampled; the previous cache allowance remains"),
        }
    }

    /// Map the actual owner epoch; failed arithmetic is returned rather than hidden.
    pub(super) fn refresh_deadline(&mut self) -> Result<(), Box<dyn Error>> {
        let child = self.child.next_deadline();
        #[cfg(all(windows, feature = "native-composition"))]
        let composition =
            self.composition.as_ref().and_then(composition::CompositionWork::deadline);
        #[cfg(not(all(windows, feature = "native-composition")))]
        let composition = None;
        self.next_deadline = child
            .into_iter()
            .chain(composition)
            .min()
            .map(|at| self.clock.instant_for(at))
            .transpose()?;
        Ok(())
    }
}

/// Grant native canvas operations only while the application runs and the sending main lifetime
/// is composition-hosted. Other canvas messages reach the child, whose refusal owner reports the
/// native surface as unsupported so the page presents its canvas itself.
#[cfg(all(windows, feature = "native-composition"))]
fn grants_native_canvas(running: bool, hosting: sugarsubstitute_desktop::MainHosting) -> bool {
    running && hosting.presents_native_canvas()
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
