// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run admitted source writes and keep isolated leaf writers alive until owned Job stop.
use super::{
    Result,
    channel::{Channel, Message},
    options::{ChildOptions, Role},
    writers::{self, Written},
};
use std::{io, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionBatch, ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation,
        OwnedJob, Scope, TerminalKind,
    },
    transcript::OutputStream,
};
use sugarsubstitute_desktop::run_headless;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::fixture_descendant::{
    FixtureDescendantInput, FixtureDescendantLaunch,
};
pub(super) fn run(
    options: ChildOptions,
    health: crate::diagnostics::DiagnosticHealth,
) -> Result<i32> {
    let mut child = Child::new(options, health);
    run_headless(&mut child)?;
    if !child.finished || child.options.role == Role::Leaf {
        return Err("Output child lacks actual orderly shutdown".into());
    }
    Ok(0)
}
pub(super) struct Child {
    health: crate::diagnostics::DiagnosticHealth,
    pub(super) options: ChildOptions,
    pub(super) clock: Arc<MonotonicClock>,
    pub(super) owner: Option<ExecutionOwner>,
    pub(super) client: Option<ExecutionClient>,
    pub(super) scope: Option<Scope>,
    pub(super) native_scope: Option<Scope>,
    pub(super) channel: Option<Channel>,
    pub(super) deadline: Option<Moment>,
    pub(super) deadline_wall: Option<Instant>,
    pub(super) deadline_observed: bool,
    pub(super) shutdown_expiry_observed: bool,
    pub(super) retirement_requested: bool,
    pub(super) closing: bool,
    pub(super) finished: bool,
    pub(super) writes: [Option<OwnedJob<Written, io::Error>>; 2],
    written: [Option<Written>; 2],
    writers_started: bool,
    done_queued: bool,
    exit_requested: bool,
    forward: Option<FixtureDescendantLaunch>,
    forward_spec: Option<JobSpec>,
    forward_started: bool,
    write_notices: [Option<TerminalKind>; 2],
    forward_notice: Option<TerminalKind>,
    phase: Role,
    pub(super) execution_stopping: bool,
    next_job: u64,
}
impl Child {
    fn new(options: ChildOptions, health: crate::diagnostics::DiagnosticHealth) -> Self {
        let phase = options.role;
        Self {
            health,
            options,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            native_scope: None,
            channel: None,
            deadline: None,
            deadline_wall: None,
            deadline_observed: false,
            shutdown_expiry_observed: false,
            retirement_requested: false,
            closing: false,
            finished: false,
            writes: [None, None],
            written: [None, None],
            writers_started: false,
            done_queued: false,
            exit_requested: false,
            forward: None,
            forward_spec: None,
            forward_started: false,
            write_notices: [None, None],
            forward_notice: None,
            phase,
            execution_stopping: false,
            next_job: 2,
        }
    }
    fn spec(&mut self, bytes: u64) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Output child job exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: self.deadline,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Use the containment adapter's operation while retaining the original campaign deadline.
    fn descendant_spec(&mut self, bytes: u64) -> Result<JobSpec> {
        let mut spec = self.spec(bytes)?;
        spec.operation = Operation::NativeLifecycle;
        Ok(spec)
    }
    pub(super) fn start_writers(&mut self) -> Result<()> {
        if self.writers_started {
            return Err("Output writers cannot start twice".into());
        }
        self.writers_started = true;
        for (i, stream) in [OutputStream::Stdout, OutputStream::Stderr].into_iter().enumerate() {
            let spec = self.spec(40960)?;
            self.writes[i] = Some(writers::submit(
                self.client.as_ref().ok_or("Output child client missing")?,
                self.scope.as_ref().ok_or("Output child scope missing")?,
                spec,
                self.phase,
                stream,
            )?);
        }
        Ok(())
    }
    fn forward(&mut self) -> Result<()> {
        if self.forward.is_some() {
            return Err("Output descendant launch repeated".into());
        }
        let deadline = self.options.deadline.ok_or("Forward root deadline missing")?;
        let input = FixtureDescendantInput::current_output(
            sugarsubstitute_execution::windows_fixture::FixtureDeadline::new(
                u64::try_from(deadline.ticks())?,
                u64::try_from(deadline.frequency())?,
            )?,
            sugarsubstitute_execution::windows::NativeCorrelation::new(1)?,
        )?;
        let spec = self.descendant_spec(input.required_reservation_bytes())?;
        self.forward = Some(FixtureDescendantLaunch::submit(
            self.client.as_ref().ok_or("Forward execution missing")?,
            self.scope.as_ref().ok_or("Forward scope missing")?,
            spec,
            input,
        )?);
        self.forward_spec = Some(spec);
        Ok(())
    }
    pub(super) fn activity(&mut self, max: NonZeroUsize) -> Result<bool> {
        self.observe_deadline()?;
        let batch =
            self.client.as_ref().ok_or("Output child client missing")?.drain_completions(max)?;
        self.capture(&batch)?;
        for i in 0..2 {
            self.consume_write(i)?;
        }
        self.consume_forward()?;
        let message =
            self.channel.as_mut().map(|channel| channel.activity(&batch)).transpose()?.flatten();
        if let Some(message) = message {
            match message {
                Message::Welcome if !self.writers_started => {
                    if self.options.role == Role::Idle {
                        tracing::info!(
                            operation = "output_fixture_stdio_live",
                            "Actual inherited diagnostic write before reader closure"
                        );
                        self.health.check()?;
                        self.channel
                            .as_mut()
                            .ok_or("Idle channel missing")?
                            .queue(Message::Primed)?;
                        self.done_queued = true;
                    }
                    if self.options.role == Role::Flood {
                        self.phase = Role::Prelude;
                        self.start_writers()?;
                    }
                }
                Message::Go
                    if !self.closing
                        && (!self.writers_started
                            || self.options.role == Role::Flood
                                && self.phase == Role::Prelude
                                && self.done_queued) =>
                {
                    if self.options.role == Role::Forward {
                        self.writers_started = true;
                        self.forward()?;
                    } else if self.options.role == Role::Flood {
                        self.writers_started = false;
                        self.done_queued = false;
                        self.written = [None, None];
                        self.phase = Role::Flood;
                        self.start_writers()?;
                    } else {
                        return Err("Idle child cannot write on Go".into());
                    }
                }
                Message::Exit if !self.exit_requested => {
                    self.exit_requested = true;
                    if self.options.role == Role::Idle {
                        tracing::info!(
                            operation = "output_fixture_stdio_closed",
                            "Actual diagnostic write after checked owned-reader retirement"
                        );
                        let code = self
                            .health
                            .native_error()
                            .ok_or("Actual closed-reader diagnostic error missing")?;
                        self.health.check().err().ok_or("Original diagnostic cause missing")?;
                        let channel =
                            self.channel.as_mut().ok_or("Diagnostic reply channel missing")?;
                        channel.diagnostic_error = Some(u32::try_from(code)?);
                        channel.queue(Message::DiagnosticFailed)?;
                    }
                }
                _ => return Err("Output child command state/order differs".into()),
            }
        }
        if let Some(forward) = &mut self.forward
            && self.forward_started
            && forward.native_retired()
            && !self.done_queued
        {
            self.channel.as_mut().ok_or("Forward channel missing")?.queue(Message::Done)?;
            self.done_queued = true;
        }
        if self.writers_started && self.written.iter().all(Option::is_some) && !self.done_queued {
            if let Some(channel) = &mut self.channel {
                channel.queue(if self.phase == Role::Prelude {
                    Message::Primed
                } else {
                    Message::Done
                })?;
            }
            self.done_queued = true;
        }
        Ok(self.closing
            || self.options.role == Role::Corpus && self.done_queued
            || self.exit_requested
                && self.done_queued
                && self.channel.as_ref().is_some_and(|channel| {
                    if self.options.role == Role::Idle {
                        channel.last_written == Some(Message::DiagnosticFailed)
                    } else {
                        matches!(channel.last_written, Some(Message::Done | Message::Primed))
                    }
                }))
    }
    fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        for notice in &batch.notifications {
            for i in 0..2 {
                if self.writes[i].as_ref().is_some_and(|job| {
                    job.scope_id() == notice.scope_id && job.spec().id == notice.job_id
                }) {
                    self.write_notices[i] = Some(notice.kind);
                }
            }
            if self.forward_spec.is_some_and(|spec| spec.id == notice.job_id)
                && self.scope.as_ref().is_some_and(|scope| scope.id() == notice.scope_id)
            {
                self.forward_notice = Some(notice.kind);
            }
        }
        if let Some(channel) = &mut self.channel {
            channel.capture(batch)?;
        }
        Ok(())
    }
    fn consume_forward(&mut self) -> Result<()> {
        let Some(kind) = self.forward_notice else { return Ok(()) };
        let Some(forward) = &mut self.forward else {
            return Err("Forward terminal has no owner".into());
        };
        let Some(completion) = forward.take_completion() else { return Ok(()) };
        let expected = self.forward_spec.take();
        self.forward_notice = None;
        if Some(completion.spec) != expected || completion.outcome.kind() != kind {
            return Err("Forward actual terminal differs".into());
        }
        super::failure::outcome(completion.outcome)?;
        if !self.forward_started
            && let Some(started) = forward.take_started()
        {
            self.channel.as_mut().ok_or("Forward channel missing")?.descendant =
                Some((started.process_id, started.creation_time_100ns));
            self.forward_started = true;
        }
        Ok(())
    }
    fn consume_write(&mut self, i: usize) -> Result<()> {
        let Some(job) = &mut self.writes[i] else { return Ok(()) };
        let Some(kind) = self.write_notices[i] else { return Ok(()) };
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        let expected = (job.scope_id(), job.spec());
        self.writes[i] = None;
        self.write_notices[i] = None;
        if completion.spec != expected.1
            || completion.scope_id != expected.0
            || completion.outcome.kind() != kind
        {
            return Err("Output writer terminal identity differs".into());
        }
        self.written[i] = Some(super::failure::outcome(completion.outcome)?);
        Ok(())
    }
    pub(super) fn writes_settled(&self) -> bool {
        self.writes.iter().all(Option::is_none)
            && self.forward.as_ref().is_none_or(FixtureDescendantLaunch::native_retired)
    }
    pub(super) fn retry_retirement(&mut self) -> Result<()> {
        let ready = self.closing
            && self.forward_spec.is_none()
            && self.forward.as_ref().is_some_and(|forward| !forward.native_retired());
        if self.take_retirement_request(ready) {
            let mut spec = self.descendant_spec(8192)?;
            spec.deadline = None;
            self.forward.as_mut().ok_or("Forward owner missing")?.submit_retirement(
                self.client.as_ref().ok_or("Forward retirement client missing")?,
                self.scope.as_ref().ok_or("Forward retirement scope missing")?,
                spec,
            )?;
            self.forward_spec = Some(spec);
        }
        Ok(())
    }
    fn take_retirement_request(&mut self, ready: bool) -> bool {
        ready && std::mem::take(&mut self.retirement_requested)
    }
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        if self.options.role == Role::Leaf && self.done_queued {
            return None;
        }
        (!self.deadline_observed).then_some(self.deadline_wall).flatten()
    }
    pub(super) fn observe_deadline(&mut self) -> Result<()> {
        // Completed leaf writes retain their actual inherited writers until the
        // outer owned Job stops this process. No local timer fabricates EOF.
        if self.options.role == Role::Leaf && self.done_queued {
            return Ok(());
        }
        if !self.deadline_observed && self.deadline.is_some_and(|at| self.clock.now() >= at) {
            self.deadline_observed = true;
            self.retirement_requested = true;
            if !self.closing {
                return Err("Output child reached its original admitted deadline".into());
            }
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "child_tests.rs"]
mod tests;
