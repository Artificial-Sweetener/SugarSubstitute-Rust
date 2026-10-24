// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive preparation and explicit cleanup from the existing native headless pump.
use super::*;
impl HeadlessLifetime for Session {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing || self.finished {
            return Err("Update session cannot start twice or after close".into());
        }
        let until = self.clock.now().checked_add(CAMPAIGN)?;
        self.until = Some(until);
        self.wake_at =
            Some(Instant::now().checked_add(CAMPAIGN).ok_or("Update deadline overflow")?);
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Update execution absent")?.client();
        self.client = Some(client.clone());
        self.cleanup =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.work = Some(
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: Some(until) }, None)?,
        );
        let factory = self.factory.take().ok_or("Update factory consumed")?;
        self.initializing = Some(client.submit_blocking(
            self.work.as_ref().ok_or("Update work scope absent")?,
            JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(until),
                reservation_bytes: self.initialization_bytes,
                replacement: None,
            },
            move |context| factory(&context),
        )?);
        Ok(())
    }
    fn prepare_native_runtime(&mut self, maximum: NonZeroUsize) -> Result<StartupPreparationStep> {
        Ok(match self.advance(maximum)? {
            HeadlessStep::Active => StartupPreparationStep::Waiting,
            HeadlessStep::Shutdown => StartupPreparationStep::Closing,
        })
    }
    fn activity(&mut self, maximum: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(maximum)
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.wake_at
            .into_iter()
            .chain(self.engine.as_ref().and_then(super::super::engine::Engine::next_deadline))
            .min()
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        if let Some(job) = &self.initializing {
            job.cancel();
        }
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
        if let Some(work) = &self.work {
            work.cancel();
        }
        let budget = sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(self.clock.now())?;
        self.cleanup_until = Some(budget.settle_until);
        self.wake_at =
            Some(Instant::now().checked_add(CLEANUP).ok_or("Update cleanup deadline overflow")?);
        if let Some(engine) = &mut self.engine {
            engine.close(budget)?;
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.closing {
            return Ok(false);
        }
        self.advance(NonZeroUsize::MIN.saturating_add(31))?;
        if self.initializing.is_some() {
            return self.unsettled();
        }
        if let Some(worker) = &self.worker
            && worker.progress() != Progress::Closed
        {
            if !worker.pending() {
                self.submit(true)?;
            }
            return self.unsettled();
        }
        if let Some(engine) = &self.engine
            && !engine.settled()?
        {
            return self.unsettled();
        }
        let Some(owner) = &mut self.owner else {
            return Ok(true);
        };
        if !self.stopping {
            owner.request_shutdown(self.cleanup_until.ok_or("Update cleanup cutoff absent")?);
            self.stopping = true;
        }
        if owner.poll_shutdown()?.settled { Ok(true) } else { self.unsettled() }
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if self.finished || !self.closing || !self.poll_shutdown()? {
            return Err("Update session has not settled or already finished".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.worker = None;
        self.engine = None;
        self.work = None;
        self.cleanup = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}

impl Session {
    /// Report an expired cutoff only while actual work remains; delayed observation is not failure.
    fn unsettled(&mut self) -> Result<bool> {
        if self.cleanup_until.is_some_and(|until| self.clock.now() >= until)
            && self.wake_at.take().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Update cleanup remains owned after its cutoff",
            )
            .into());
        }
        Ok(false)
    }
}
