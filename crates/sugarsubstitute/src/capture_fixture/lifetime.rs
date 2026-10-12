// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep capture teardown before real worker joins and process-lifetime charge reporting.
use super::*;
use sugarsubstitute_execution::windows::NativeCorrelation;
impl HeadlessLifetime for Fixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result {
        if self.owner.is_some() || self.closing {
            return Err("Capture fixture cannot start twice".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Capture execution missing")?.client();
        self.client = Some(client.clone());
        // All scopes and the handler watch exist before any bridge effect.
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.start_scope = Some(client.create_scope(
            ScopeSpec { id: ScopeId::new(2)?, deadline: Some(self.budget.until()) },
            None,
        )?);
        self.native_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
        );
        let watch = client.reserve_process_watch(
            self.native_scope.as_ref().ok_or("Capture watch scope missing")?,
            specification::watch()?,
            NativeCorrelation::new(specification::WATCH_JOB)?,
        )?;
        let input = self.input.take().ok_or("Capture input missing")?;
        match self.capture.submit_start(
            &client,
            self.start_scope.as_ref().ok_or("Capture start scope missing")?,
            specification::finite(
                specification::START_JOB,
                self.budget.until(),
                input.required_reservation_bytes() as u64,
            )?,
            self.budget,
            watch,
            input,
        ) {
            Ok(()) => {
                self.start_admitted = true;
                Ok(())
            }
            Err(rejection) => {
                let (reason, watch, input) = rejection.into_parts()?;
                self.input = Some(input);
                self.rejected_watch = Some(watch.cancel_unbound());
                Err(Box::new(reason))
            }
        }
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        [
            (!self.closing && !self.registration_expired).then_some(self.registration_alarm),
            self.stop.as_ref().and_then(StopPlan::next_deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn request_shutdown(&mut self) -> Result {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        if let Some(scope) = &self.start_scope {
            scope.cancel();
        }
        // A real timestamp is mandatory; failed clock metadata cannot renew a budget.
        let started =
            self.client.as_ref().map_or_else(|| Ok(self.clock.now()), ExecutionClient::now)?;
        let native = Instant::now();
        let after =
            self.client.as_ref().map_or_else(|| Ok(self.clock.now()), ExecutionClient::now)?;
        self.stop = Some(StopPlan::new(started, started, native, after)?);
        self.demand.request();
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.closing
            || self.rejected_watch.is_some()
            || self.demand.has_pending()
            || (self.start_admitted && self.capture.snapshot().state != CaptureState::Closed)
        {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.execution_stopping {
            let deadline =
                self.stop.as_ref().ok_or("Capture shutdown budget missing")?.budget.settle_until;
            self.execution_stopping = true;
            owner.request_shutdown(deadline);
        }
        let progress = owner.poll_shutdown()?;
        if progress.settled {
            self.workers = Some(progress.workers);
            self.retained_bytes_at_finish =
                Some(self.start_scope.as_ref().map_or(0, |s| s.snapshot().reserved_bytes));
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result {
        if !self.poll_shutdown()? {
            return Err("Capture fixture cannot finish retained native work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.client = None;
        self.owner = None;
        self.cleanup_scope = None;
        self.native_scope = None;
        self.start_scope = None;
        self.finished = true;
        Ok(())
    }
}
