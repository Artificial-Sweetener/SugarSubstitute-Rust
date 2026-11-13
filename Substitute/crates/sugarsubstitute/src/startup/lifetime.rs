// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual child cleanup admission open until native/render ownership settles.
use super::*;
use sugarsubstitute_desktop::StartupCloseStage;

impl<B: StartupBackend> StartupHost<B> {
    /// Start the sole execution owner with the real wake, then transfer actual child scope custody.
    /// # Errors
    /// Retain partial setup through shutdown; a failed start cannot replace this lifetime.
    pub(super) fn start_owned(
        &mut self,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<(), Box<dyn Error>> {
        if self.started || self.closing {
            return Err(std::io::Error::other("Startup content cannot restart").into());
        }
        self.started = true;
        self.execution.start(wake)?;
        if self.scopes.render.id < self.scopes.child.id {
            self.render_scope = Some(self.execution.scope(self.scopes.render)?);
            self.child_scope = Some(self.execution.scope(self.scopes.child)?);
        } else {
            self.child_scope = Some(self.execution.scope(self.scopes.child)?);
            self.render_scope = Some(self.execution.scope(self.scopes.render)?);
        }
        let child_scope = self
            .child_scope
            .take()
            .ok_or_else(|| std::io::Error::other("Actual child scope absent"))?;
        self.child.start(&self.execution.client()?, child_scope, &self.jobs)?;
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(composition) = &mut self.composition {
            composition.start(self.execution.client()?)?;
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(workspace) = &mut self.workspace {
            workspace.start(self.execution.client()?)?;
        }
        self.refresh_deadline()
    }
    /// Fence UI/render admission and deliver one original shutdown transaction to the child.
    /// # Errors
    /// Preserve first measurement/cleanup failure while attempting independent cancellation.
    pub(super) fn close_owned(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return self.advance_shutdown_budget();
        }
        self.closing = true;
        self.visible = false;
        #[cfg(not(all(windows, feature = "native-composition")))]
        self.commands.close();
        let mut failure = None;
        retain(&mut failure, self.advance_shutdown_budget());
        let policy = self.policy.close(self.startup.as_mut().map(PreparedStartup::session_mut));
        match policy {
            Ok(Some(effect)) => {
                let applied = self
                    .startup
                    .as_mut()
                    .ok_or_else(|| {
                        std::io::Error::other("Actual startup policy absent for cleanup")
                    })
                    .map_err(|cause| Box::new(cause) as Box<dyn Error>)
                    .and_then(|prepared| self.child.apply(effect, prepared));
                retain(&mut failure, self.observed_close(StartupCloseStage::PolicyApply, applied));
            }
            Ok(None) => {}
            Err(cause) => {
                retain(&mut failure, Err(self.close_failure(StartupCloseStage::PolicyClose, cause)))
            }
        }
        if let Some(render) = &mut self.preparation {
            let retirement = render.request_retirement();
            retain(
                &mut failure,
                self.observed_close(StartupCloseStage::RenderRetirement, retirement),
            );
        }
        let deadline = self.refresh_deadline();
        retain(&mut failure, self.observed_close(StartupCloseStage::DeadlineRefresh, deadline));
        failure.map_or(Ok(()), Err)
    }
    /// Await the existing capture measurement before admitting authenticated exit intent.
    /// An absent capture permits exactly one local sample; pending capture never does.
    pub(super) fn advance_shutdown_budget(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(all(windows, feature = "native-composition"))]
        let flushing = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.close_ready(self.clock.now()));
        #[cfg(not(all(windows, feature = "native-composition")))]
        let flushing = false;
        let initiated =
            if flushing { self.child.begin_workspace_close() } else { self.child.begin_shutdown() };
        let initiated = self.observed_close(StartupCloseStage::CaptureObservation, initiated);
        let existing = self.child.shutdown_budget();
        let mut failure = None;
        let origin = match initiated {
            Ok(origin) => {
                Some(existing.map_or(origin, crate::startup::ShutdownInitiation::Captured))
            }
            Err(cause) if existing.is_some() => {
                retain(&mut failure, Err(cause));
                existing.map(crate::startup::ShutdownInitiation::Captured)
            }
            Err(cause) => {
                retain(&mut failure, Err(self.shutdown.reject(cause)));
                None
            }
        };
        if let Some(origin) = origin {
            match self.shutdown.request(origin, || {
                if self.execution.has_owner() { self.execution.now() } else { Ok(self.clock.now()) }
            }) {
                Ok(Some(_)) => {}
                Ok(None) => {}
                Err(cause) => retain(
                    &mut failure,
                    Err(self.close_failure(StartupCloseStage::ShutdownBudget, cause)),
                ),
            }
        }
        self.shutdown.tighten(self.child.shutdown_budget());
        if let Ok(budget) = self.shutdown.budget() {
            self.execution.retain_shutdown_cutoff(budget.settle_until);
            #[cfg(all(windows, feature = "native-composition"))]
            let close_save_cutoff = self.close_save_cutoff(budget);
            #[cfg(all(windows, feature = "native-composition"))]
            if let Some(workspace) = &mut self.workspace {
                workspace.retain_retirement_until(budget.settle_until);
                workspace.begin_close(self.clock.now(), close_save_cutoff)?;
            }
            // The close save gates the exit intent: the input canvas's final save is submitted
            // and both owners answer before the child is asked to shut down.
            let now = sugarsubstitute_contracts::execution::Clock::now(&*self.clock);
            if !self.advance_close_save(budget, now)? {
                return self.refresh_deadline();
            }
            if self.close_reason
                == sugarsubstitute_application::workspace_session::close_save::CloseReason::RestartToApply
                && self.close_save_outcome().is_some()
            {
                let requested = self.child.request_restart_to_apply();
                retain(&mut failure, self.observed_close(StartupCloseStage::ExitIntent, requested));
            }
            let requested = self.child.request_shutdown(budget);
            retain(&mut failure, self.observed_close(StartupCloseStage::ExitIntent, requested));
            self.commands.close();
            #[cfg(all(windows, feature = "native-composition"))]
            if let Some(composition) = &mut self.composition {
                composition.close();
            }
            #[cfg(all(windows, feature = "native-composition"))]
            if let Some(composition) = &mut self.composition {
                composition.tighten(budget.settle_until);
            }
            #[cfg(windows)]
            if let Some(canvas) = &mut self.canvas {
                let retirement = canvas.close_until(budget.settle_until);
                retain(
                    &mut failure,
                    self.observed_close(StartupCloseStage::CanvasRetirement, retirement),
                );
            }
        }
        let deadline = self.refresh_deadline();
        retain(&mut failure, self.observed_close(StartupCloseStage::DeadlineRefresh, deadline));
        failure.map_or(Ok(()), |cause| {
            Err(self.close_failure(StartupCloseStage::ShutdownBudget, cause))
        })
    }
    /// Observe actual child/render settlement before closing global execution admission.
    /// # Errors
    /// Retain failed or unmeasured work without granting native settlement or renewing time.
    pub(super) fn poll_owned_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        let mut failure = None;
        retain(&mut failure, self.consume(NonZeroUsize::MIN.saturating_add(31)).map(|_| ()));
        retain(&mut failure, self.advance_shutdown_budget());
        let render = self.preparation.as_mut().map_or(Ok(true), PreparationDriver::poll_retirement);
        let child = self.child.poll_shutdown();
        let render = observe_settlement(&mut failure, render);
        let child = observe_settlement(&mut failure, child);
        #[cfg(windows)]
        let canvas = self.canvas.as_mut().map_or(Ok(true), canvas::EmbeddedCanvas::settled);
        #[cfg(windows)]
        let canvas = observe_settlement(&mut failure, canvas);
        #[cfg(not(windows))]
        let canvas = true;
        #[cfg(all(windows, feature = "native-composition"))]
        let composition = {
            let now = if self.execution.has_owner() {
                self.execution.now()
            } else {
                Ok(self.clock.now())
            };
            let settled = now.and_then(|now| {
                self.composition.as_mut().map_or(Ok(true), |composition| composition.settled(now))
            });
            observe_settlement(&mut failure, settled)
        };
        #[cfg(not(all(windows, feature = "native-composition")))]
        let composition = true;
        self.shutdown.tighten(self.child.shutdown_budget());
        let measured = match self.shutdown.budget() {
            Ok(budget) => {
                self.execution.retain_shutdown_cutoff(budget.settle_until);
                true
            }
            Err(_) if self.shutdown.pending() => false,
            Err(cause) => {
                retain(&mut failure, Err(cause));
                false
            }
        };
        #[cfg(all(windows, feature = "native-composition"))]
        let workspace = self.workspace.as_ref().is_none_or(workspace::WorkspaceWork::settled);
        #[cfg(not(all(windows, feature = "native-composition")))]
        let workspace = true;
        let execution = self
            .execution
            .poll_shutdown(render && child && canvas && composition && measured && workspace);
        let execution = observe_settlement(&mut failure, execution);
        failure.map_or(Ok(render && child && canvas && composition && workspace && execution), Err)
    }
    /// Called by Desktop only after its native controls/loop and wake lifetime disconnect.
    /// # Errors
    /// Failed native/render finalization or worker joins retain the entire content lifetime.
    pub(super) fn finish_owned_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.finished {
            return Ok(());
        }
        if !self.poll_owned_shutdown()? {
            return Err(std::io::Error::other("Actual startup owners remain unsettled").into());
        }
        let mut failure = None;
        if let Some(render) = &mut self.preparation {
            retain(&mut failure, render.finish_retirement());
        }
        retain(&mut failure, self.child.finish_shutdown());
        #[cfg(windows)]
        if let Some(canvas) = &mut self.canvas {
            retain(&mut failure, canvas.finish());
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if let Some(composition) = &mut self.composition {
            let now = if self.execution.has_owner() {
                self.execution.now()
            } else {
                Ok(self.clock.now())
            };
            let finished = now.and_then(|now| composition.finish(now));
            retain(&mut failure, finished);
        }
        if failure.is_none() {
            retain(&mut failure, self.execution.finish());
        }
        match failure {
            Some(cause) => Err(cause),
            None => {
                self.preparation = None;
                self.finished = true;
                Ok(())
            }
        }
    }
}
fn observe_settlement(
    failure: &mut Option<Box<dyn Error>>,
    result: Result<bool, Box<dyn Error>>,
) -> bool {
    match result {
        Ok(settled) => settled,
        Err(cause) => {
            retain(failure, Err(cause));
            false
        }
    }
}
fn retain(failure: &mut Option<Box<dyn Error>>, result: Result<(), Box<dyn Error>>) {
    if let Err(cause) = result {
        if failure.is_none() {
            *failure = Some(cause);
        } else {
            tracing::error!(operation="startup_retirement",cause=%cause,"Additional owned startup retirement failure");
        }
    }
}
