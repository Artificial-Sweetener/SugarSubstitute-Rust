// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exit0 only after fixed descendant creation, forwarding closure and real worker joins.
mod failure;
use crate::held_child::ChildOptions;
use failure::terminal;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock,
    windows_fixture::{FixtureForwardingSession, MIN_FIXTURE_SESSION_BYTES},
};
use sugarsubstitute_supervision::fixture_descendant::{
    FixtureDescendantInput, FixtureDescendantLaunch,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run(options: ChildOptions) -> Result<i32> {
    let mut root = Root::new(options);
    run_headless(&mut root)?;
    if !root.finished || !root.created || !root.forwarded {
        return Err("Forwarding root lacks actual complete creation/retirement".into());
    }
    Ok(0)
}
struct Root {
    options: ChildOptions,
    clock: Arc<MonotonicClock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    native_scope: Option<Scope>,
    creation_scope: Option<Scope>,
    retirement_scope: Option<Scope>,
    session: Option<FixtureForwardingSession>,
    launch: Option<FixtureDescendantLaunch>,
    deadline: Option<Moment>,
    next_job: u64,
    created: bool,
    forwarded: bool,
    closing: bool,
    stopping: bool,
    finished: bool,
    retirement_attempted: bool,
}
impl Root {
    fn new(options: ChildOptions) -> Self {
        Self {
            options,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            native_scope: None,
            creation_scope: None,
            retirement_scope: None,
            session: None,
            launch: None,
            deadline: None,
            next_job: 2,
            created: false,
            forwarded: false,
            closing: false,
            stopping: false,
            finished: false,
            retirement_attempted: false,
        }
    }
    fn specification(&mut self, bytes: u64, deadline: Option<Moment>) -> Result<JobSpec> {
        let spec = JobSpec {
            id: JobId::new(self.next_job)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline,
            reservation_bytes: bytes,
            replacement: None,
        };
        self.next_job = self.next_job.checked_add(1).ok_or("Root job identity overflow")?;
        Ok(spec)
    }
    fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            let _ = client.drain_completions(max)?;
        }
        if let Some(launch) = &mut self.launch {
            if let Some(completion) = launch.take_completion() {
                terminal(completion.outcome)?;
                if let Some(started) = launch.take_started() {
                    if started.correlation != self.options.correlation {
                        return Err("Root creation correlation mismatch".into());
                    }
                    tracing::info!(
                        operation = "fixture_descendant_started",
                        correlation = started.correlation.get(),
                        process_id = started.process_id,
                        creation_time_100ns = started.creation_time_100ns,
                        "Actual inherited descendant resumed"
                    );
                    self.created = true;
                }
            }
        } else if !self.closing
            && let Some(session) = &self.session
            && let Some(events) = session.control.take_events()?
        {
            let input = FixtureDescendantInput::current(
                events,
                self.options.deadline,
                self.options.correlation,
                self.options.exit_code as u32,
            )?;
            let spec = self.specification(input.required_reservation_bytes(), self.deadline)?;
            self.launch = Some(FixtureDescendantLaunch::submit(
                self.client.as_ref().ok_or("Root client")?,
                self.creation_scope.as_ref().ok_or("Root creation scope")?,
                spec,
                input,
            )?);
        }
        if let Some(session) = &mut self.session
            && let Some(completion) = session.completion.try_take_completion()
        {
            self.session = None;
            match completion.outcome {
                JobOutcome::Success(report)
                    if report.prepared && report.correlation == self.options.correlation =>
                {
                    self.forwarded = true
                }
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => {
                    return Err(
                        "Root forwarding failed to retire successful prepared ownership".into()
                    );
                }
            }
        }
        if self.closing
            || (self.created
                && self.forwarded
                && self.launch.as_ref().is_some_and(FixtureDescendantLaunch::native_retired))
        {
            Ok(HeadlessStep::Shutdown)
        } else {
            Ok(HeadlessStep::Active)
        }
    }
}
impl HeadlessLifetime for Root {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Root cannot start twice or after closure".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Root owner")?.client();
        self.native_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        self.creation_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
        self.retirement_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?);
        let permit = client.reserve_fixture_forwarding(
            self.native_scope.as_ref().ok_or("Root native scope")?,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: MIN_FIXTURE_SESSION_BYTES,
                replacement: None,
            },
            self.options.correlation,
            self.options.deadline,
        )?;
        let session =
            permit.bind(self.options.events.take().ok_or("Root inherited input already consumed")?);
        self.deadline = session.completion.spec().deadline;
        self.session = Some(session);
        self.client = Some(client);
        Ok(())
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.closing = true;
        if let Some(scope) = &self.creation_scope {
            scope.cancel();
        }
        if let Some(session) = &self.session {
            session.control.cancel();
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.closing {
            return Ok(false);
        }
        if let Some(launch) = &mut self.launch
            && let Some(completion) = launch.take_completion()
        {
            match completion.outcome {
                JobOutcome::Success(()) | JobOutcome::Cancelled => {}
                other => {
                    terminal(other)?;
                }
            }
        }
        if self.launch.as_ref().is_some_and(|launch| !launch.native_retired()) {
            if self.retirement_attempted {
                return Ok(false);
            }
            let spec = self.specification(4096, None)?;
            match self.launch.as_mut().ok_or("Root launch")?.submit_retirement(
                self.client.as_ref().ok_or("Root client")?,
                self.retirement_scope.as_ref().ok_or("Root retirement scope")?,
                spec,
            ) {
                Ok(_) => self.retirement_attempted = true,
                Err(sugarsubstitute_supervision::RegistryAdmissionError::Busy) => {}
                Err(cause) => return Err(Box::new(cause)),
            }
            return Ok(false);
        }
        if let Some(session) = &mut self.session {
            let Some(completion) = session.completion.try_take_completion() else {
                return Ok(false);
            };
            self.session = None;
            match completion.outcome {
                JobOutcome::Success(report) if report.prepared => self.forwarded = true,
                JobOutcome::Cancelled => {}
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Root forwarding cleanup did not retire successfully".into()),
            }
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.stopping {
            self.stopping = true;
            owner.request_shutdown(self.deadline.unwrap_or_else(|| self.clock.now()));
        }
        let progress = owner.poll_shutdown()?;
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Root still owns unresolved native outputs".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.launch = None;
        self.native_scope = None;
        self.creation_scope = None;
        self.retirement_scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
