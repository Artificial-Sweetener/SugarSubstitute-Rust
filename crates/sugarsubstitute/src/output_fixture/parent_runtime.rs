// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle capture, authenticated channels and containment before execution shutdown.
use super::{Result, parent::Parent};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionPort, ScopeId, ScopeSpec};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep};
use sugarsubstitute_execution::ExecutionOwner;
impl HeadlessLifetime for Parent {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Output parent cannot start twice/closed".into());
        }
        let owner = ExecutionOwner::new(self.clock.clone(), wake.clone())?;
        let client = owner.client();
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.native_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        );
        self.owner = Some(owner);
        self.client = Some(client);
        self.wake = Some(wake);
        self.reserve_next()
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        Parent::activity(self, max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        let original = (!self.deadline_observed).then_some(self.deadline);
        let stop = self
            .cases
            .get(self.current)
            .filter(|case| !case.retired)
            .and_then(|case| case.stop.as_ref())
            .and_then(|stop| stop.next());
        match (original, stop) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.registry.close_launch_admission();
        for case in &mut self.cases {
            drop(case.guard.take());
            if let Some(session) = &case.output {
                session.control.close();
            }
            if let Some(channel) = &mut case.channel {
                channel.close();
            }
            if let Some(job) = &case.store_job {
                job.cancel();
            }
        }
        self.ensure_stop()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.closing
            || !self.registry.is_empty()
            || self.cases.iter().any(|case| {
                case.store_job.is_some()
                    || case.output.is_some() && case.output_terminal.is_none()
                    || case.channel.as_ref().is_some_and(|channel| !channel.terminal)
            })
        {
            return Ok(false);
        }
        if !self.execution_stopping {
            self.execution_stopping = true;
            for case in &mut self.cases {
                case.output = None;
                case.store = None;
                drop(case.guard.take());
            }
            if let Some(owner) = &mut self.owner {
                owner.request_shutdown(self.original);
            }
        }
        let Some(owner) = &self.owner else { return Ok(true) };
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !progress.settled && !self.deadline_observed {
            self.deadline_observed = true;
            return Err("Output execution remains owned after original60s budget".into());
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Output parent has unresolved resources/workers".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.native_scope = None;
        self.scope = None;
        self.client = None;
        self.wake = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
