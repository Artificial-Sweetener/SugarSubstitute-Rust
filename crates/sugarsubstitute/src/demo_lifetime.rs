// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the demo's sole execution owner through native teardown and worker joins.

use super::*;
use crate::job_sequence::JobSequence;
use sugarsubstitute_contracts::execution::{ExecutionPort, Moment, ScopeId, ScopeSpec};

impl SplashDemo {
    /// Create execution after native wake exists and retain partial setup until shutdown.
    pub(super) fn start_execution(
        &mut self,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<(), Box<dyn Error>> {
        if self.execution.is_some() || self.closing {
            return Err("Demo execution cannot restart".into());
        }
        self.execution = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.execution.as_ref().ok_or("Demo execution absent")?.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let mut preparation = PreparationDriver::new(
            &self.locale,
            client,
            scope,
            JobSequence::new(),
            sugarsubstitute_contracts::presentation::ConsolePresentation::Raster,
        )?;
        preparation.set_visible(self.visible)?;
        self.preparation = Some(preparation);
        Ok(())
    }

    /// Drain once at the composition boundary and route the actual batch to rendering.
    pub(super) fn consume_preparation(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let Some(owner) = &self.execution else {
            return Ok(None);
        };
        let batch = owner.client().drain_completions(maximum)?;
        match &mut self.preparation {
            Some(preparation) => preparation.consume(&batch),
            None => Ok(None),
        }
    }

    /// Cancel rendering and establish the original demo execution shutdown budget once.
    pub(super) fn retire_execution(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.visible = false;
        let render =
            self.preparation.as_mut().map(PreparationDriver::request_retirement).transpose();
        let deadline = self.clock.now().checked_add(Duration::from_secs(5));
        if let Some(owner) = &mut self.execution {
            owner.request_shutdown(deadline.unwrap_or(Moment::from_nanos(u64::MAX)));
        }
        render?;
        deadline?;
        Ok(())
    }

    /// Keep native wake and apartment custody until rendering and every worker actually settle.
    pub(super) fn poll_execution_retirement(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        self.consume_preparation(NonZeroUsize::MIN)?;
        let render_settled = match &mut self.preparation {
            Some(preparation) => preparation.poll_retirement()?,
            None => true,
        };
        let Some(owner) = &self.execution else {
            return Ok(render_settled);
        };
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !self.shutdown_deadline_reported {
            self.shutdown_deadline_reported = true;
            tracing::warn!(operation = "demo_shutdown", unresolved_jobs = ?progress.unresolved_job_ids,
                queued = progress.queued, running = progress.running, "Owned shutdown deadline expired");
        }
        Ok(render_settled && progress.settled)
    }

    /// Dispose render cache and join settled execution only after native surface teardown.
    pub(super) fn finish_execution_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.poll_execution_retirement()? {
            return Err("Demo execution retirement is unsettled".into());
        }
        if let Some(preparation) = &mut self.preparation {
            preparation.finish_retirement()?;
        }
        self.preparation = None;
        if let Some(owner) = &mut self.execution {
            owner.finish_shutdown()?;
        }
        self.execution = None;
        Ok(())
    }
}
