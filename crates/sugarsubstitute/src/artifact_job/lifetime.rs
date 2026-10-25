// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the one artifact scope through terminal delivery and observed worker settlement.
use super::*;
impl<T: Send + 'static> HeadlessLifetime for Lifetime<T> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result {
        if self.owner.is_some() || self.closing {
            return Err("Artifact lifetime cannot restart".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Artifact owner missing")?.client();
        self.client = Some(client.clone());
        self.scope =
            Some(client.create_scope(
                ScopeSpec { id: ScopeId::new(1)?, deadline: Some(self.until) },
                None,
            )?);
        self.job = Some(client.submit_blocking_owned_input(
            self.scope.as_ref().ok_or("Artifact scope missing")?,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(self.until),
                reservation_bytes: self.bytes,
                replacement: None,
            },
            &mut self.producer,
            |producer, context| producer(context),
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
        (!self.expiry_observed).then_some(self.deadline)
    }
    fn request_shutdown(&mut self) -> Result {
        self.closing = true;
        if let Some(job) = &self.job {
            job.cancel();
        }
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(self.until);
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.job.is_some() {
            return Ok(false);
        }
        let Some(owner) = &self.owner else { return Ok(true) };
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !self.expiry_observed {
            self.expiry_observed = true;
            return Err("Artifact workers remain retained beyond the original deadline".into());
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result {
        if self.job.is_some() || !self.poll_shutdown()? {
            return Err("Artifact work remains unsettled".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.scope = None;
        self.client = None;
        self.owner = None;
        Ok(())
    }
}
