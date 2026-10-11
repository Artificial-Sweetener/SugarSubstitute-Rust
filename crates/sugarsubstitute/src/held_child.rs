// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the isolated held child through admitted event retirement and native teardown.
//! Parsed event tokens carry no authority. The dedicated invocation supplies its
//! exclusive inherited inputs; failed pre-bind input/reservation exits without
//! claiming clean child-side retirement. The parent observes actual process death.
mod completion;
#[cfg(test)]
mod completion_tests;
mod failure;
mod options;
#[cfg(test)]
mod options_tests;

use completion::ChildCompletion;
pub(crate) use options::ChildOptions;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Operation, Scope, ScopeId,
    ScopeSpec,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::windows_fixture::MIN_FIXTURE_SESSION_BYTES;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Run on the native main thread; return the requested code only after actual teardown.
/// Errors retain native/panic/cancellation/deadline causes while cleanup remains owned.
/// Execution alone translates/enforces the original QPC deadline and retires events;
/// this consumer creates no native timer, window, process, raw wait or scheduler.
pub(crate) fn run(options: ChildOptions) -> Result<i32> {
    let mut child = HeldChild::new(options);
    if let Err(error) = run_headless(&mut child) {
        tracing::error!(operation = "held_child_lifetime", correlation = child.options.correlation.get(),
            cause = %error, original_cause = ?error.source(),
            "Held child retains its first failure after actual shutdown");
        return Err(Box::new(error));
    }
    if !child.finished || child.state.report().is_none() {
        return Err("Held child lacks actual successful event retirement".into());
    }
    Ok(child.options.exit_code)
}

struct HeldChild {
    options: ChildOptions,
    state: ChildCompletion,
    clock: Arc<MonotonicClock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    finished: bool,
}
impl HeldChild {
    fn new(options: ChildOptions) -> Self {
        let state = ChildCompletion::new(options.correlation);
        Self {
            options,
            state,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            finished: false,
        }
    }
}
impl HeadlessLifetime for HeldChild {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.state.closing() {
            return Err("Held child cannot start twice or after closure".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Held child execution owner missing")?.client();
        self.client = Some(client.clone());
        self.scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: MIN_FIXTURE_SESSION_BYTES,
            replacement: None,
        };
        let permit = client.reserve_fixture_child(
            self.scope.as_ref().ok_or("Held child native scope missing")?,
            spec,
            self.options.correlation,
            self.options.deadline,
        )?;
        let events =
            self.options.events.take().ok_or("Held child inherited inputs already transferred")?;
        self.state.install(permit.bind(events))
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            let batch = client.drain_completions(max)?;
            self.state.consume(&batch)?;
        }
        Ok(if self.state.closing() || self.state.terminal_consumed() {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
    fn next_deadline(&self) -> Option<Instant> {
        // The native execution owner enrolled the conservative translated deadline
        // before admission and wakes terminal consumption after actual retirement.
        None
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if !self.state.request_close() {
            return Ok(());
        }
        if let Some(scope) = &self.scope {
            scope.cancel();
        }
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(self.state.shutdown_deadline(self.clock.now()));
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.state.closing() {
            return Ok(false);
        }
        let (actual, expired) = match &self.owner {
            Some(owner) => {
                let progress = owner.poll_shutdown()?;
                (progress.settled, progress.deadline_expired)
            }
            None => (true, false),
        };
        self.state.observe_shutdown(actual, expired)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Held child cannot finish before actual event/job/worker settlement".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?
        }
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
