// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual child cleanup admission open until native/render ownership settles.
use super::*;

impl ProductionStartup {
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
        self.commands.close();
        let mut failure = None;
        retain(&mut failure, self.advance_shutdown_budget());
        let policy =
            self.policy.close(self.startup.as_mut().map(PreparedChildStartup::session_mut));
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
                retain(&mut failure, applied);
            }
            Ok(None) => {}
            Err(cause) => retain(&mut failure, Err(cause)),
        }
        if let Some(render) = &mut self.preparation {
            retain(&mut failure, render.request_retirement());
        }
        retain(&mut failure, self.refresh_deadline());
        failure.map_or(Ok(()), Err)
    }
    /// Await the existing capture measurement before admitting authenticated exit intent.
    /// An absent capture permits exactly one local sample; pending capture never does.
    fn advance_shutdown_budget(&mut self) -> Result<(), Box<dyn Error>> {
        let initiated = self.child.begin_shutdown();
        let existing = self.child.shutdown_budget();
        let mut failure = None;
        let origin = match initiated {
            Ok(origin) => {
                Some(existing.map_or(origin, super::super::child::ShutdownInitiation::Captured))
            }
            Err(cause) if existing.is_some() => {
                retain(&mut failure, Err(cause));
                existing.map(super::super::child::ShutdownInitiation::Captured)
            }
            Err(cause) => {
                retain(&mut failure, Err(self.shutdown.reject(cause)));
                None
            }
        };
        if let Some(origin) = origin {
            match self.shutdown.request(origin, || {
                if self.execution.has_owner() {
                    self.execution.now()
                } else {
                    use sugarsubstitute_contracts::execution::Clock;
                    Ok(self.clock.now())
                }
            }) {
                Ok(Some(budget)) => retain(&mut failure, self.child.request_shutdown(budget)),
                Ok(None) => {}
                Err(cause) => retain(&mut failure, Err(cause)),
            }
        }
        self.shutdown.tighten(self.child.shutdown_budget());
        if let Ok(budget) = self.shutdown.budget() {
            self.execution.retain_shutdown_cutoff(budget.settle_until);
        }
        retain(&mut failure, self.refresh_deadline());
        failure.map_or(Ok(()), Err)
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
        let execution = self.execution.poll_shutdown(render && child && measured);
        let execution = observe_settlement(&mut failure, execution);
        failure.map_or(Ok(render && child && execution), Err)
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
