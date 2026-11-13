// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the fresh subordinate's original execution, capture and actual Desktop lifetimes.
#[path = "lifetime/observations.rs"]
mod observations;
#[path = "lifetime/progress.rs"]
mod progress;
#[path = "lifetime/retirement.rs"]
mod retirement;
#[path = "lifetime/retirement_notice.rs"]
mod retirement_notice;
use super::{
    capture::Capture,
    channel::{Channel, Outgoing, Sent},
    cleanup::Cleanup,
    role::Role,
};
use crate::job_sequence::JobSequence;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, PreparedStartupSnapshot,
    recovery_supervisor::{AuthenticatedRecoveryOrigin, RecoveryDesktopEnrollment},
    windows_lifecycle_pipe::CapturedLifecycleStart,
};
use sugarsubstitute_supervision::{
    RecoveryChildCreation, RecoveryChildInput, RecoveryChildStatus, RegistryAdmissionError,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Startup,
    AcceptedQueued,
    Registering,
    RegisteredQueued,
    Attaching,
    Creating,
    CreatedQueued,
    Enrollment,
    Resuming,
    Running,
    ExitedQueued,
    Finished,
}
/// Application fields precede the actual execution owner so partial startup remains observable.
pub(super) struct Lifetime {
    role: Role,
    channel: Option<Channel>,
    capture: Option<Capture>,
    origin: Option<AuthenticatedRecoveryOrigin>,
    startup: Option<PreparedStartupSnapshot>,
    child_input: Option<RecoveryChildInput>,
    child: Option<RecoveryChildCreation>,
    enrollment: Option<RecoveryDesktopEnrollment>,
    child_status: Option<RecoveryChildStatus>,
    child_failure_stage: Option<&'static str>,
    child_cleanup_submitted: bool,
    stage: Stage,
    cleanup: Cleanup,
    notice: retirement_notice::RetirementNotice,
    setup_wake: Option<Instant>,
    cleanup_wake: Option<Instant>,
    closing: bool,
    execution_stopping: bool,
    native: Option<Scope>,
    finite: Option<Scope>,
    cleanup_scope: Option<Scope>,
    jobs: JobSequence,
    client: Option<ExecutionClient>,
    clock: Arc<MonotonicClock>,
    execution: Option<ExecutionOwner>,
}
impl Lifetime {
    /// Retain inert role inputs; construction creates no native handle, worker or window.
    pub(super) fn new(role: Role) -> Self {
        Self {
            role,
            channel: None,
            capture: None,
            origin: None,
            startup: None,
            child_input: None,
            child: None,
            enrollment: None,
            child_status: None,
            child_failure_stage: None,
            child_cleanup_submitted: false,
            stage: Stage::Startup,
            cleanup: Cleanup::new(),
            notice: retirement_notice::RetirementNotice::new(),
            setup_wake: None,
            cleanup_wake: None,
            closing: false,
            execution_stopping: false,
            native: None,
            finite: None,
            cleanup_scope: None,
            jobs: JobSequence::new(),
            client: None,
            clock: MonotonicClock::new(),
            execution: None,
        }
    }
    /// Consume each actual original owner before propagating the first failed observation.
    fn drain(&mut self, max: NonZeroUsize) -> Result<()> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        let mut failure = None;
        match client.drain_completions(max) {
            Ok(batch) => {
                if let Some(capture) = &mut self.capture {
                    retain(&mut failure, capture.observe(&batch));
                }
            }
            Err(cause) => retain(&mut failure, Err(Box::new(cause))),
        }
        if let (Some(channel), Some(finite)) = (&mut self.channel, &self.finite) {
            retain(&mut failure, channel.activity(client, finite, &self.jobs, max));
        }
        if let Some(child) = &mut self.child {
            match child.observe() {
                Ok(status) => {
                    self.child_status = Some(status);
                    if !self.closing {
                        let unexpected = status
                            .creation_terminal
                            .filter(|kind| *kind != TerminalKind::Success)
                            .or(status
                                .watch_terminal
                                .filter(|kind| *kind != TerminalKind::Success));
                        if let Some(kind) = unexpected {
                            retain(
                                &mut failure,
                                Err(format!("Recovery actual child owner ended with {kind:?}")
                                    .into()),
                            );
                        }
                    }
                }
                Err(cause) => {
                    if self.child_failure_stage != Some(cause.stage()) {
                        self.child_failure_stage = Some(cause.stage());
                        retain(&mut failure, Err(Box::new(cause)));
                    }
                    // The same immutable original outcome and cause remain in the child owner.
                }
            }
        }
        if failure.is_none() {
            retain(&mut failure, self.observe_protocol());
        }
        if failure.is_some() {
            self.notice.abandon();
            if let Some(channel) = &mut self.channel {
                channel.request_close();
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
impl HeadlessLifetime for Lifetime {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.execution.is_some() || self.closing {
            return Err("Fresh recovery lifetime cannot start twice".into());
        }
        self.execution = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.execution.as_ref().ok_or("Recovery execution absent")?.client();
        self.client = Some(client.clone());
        self.native = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        self.finite =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?);
        self.channel = Some(Channel::reserve(
            &client,
            self.native.as_ref().ok_or("Recovery native scope absent")?,
            &self.jobs,
            &mut self.role.input,
            self.role.original,
        )?);
        self.setup_wake = Some(self.clock.instant_for(
            self.channel.as_ref().ok_or("Recovery channel absent")?.original_until(),
        )?);
        Ok(())
    }
    fn prepare_native_runtime(&mut self, max: NonZeroUsize) -> Result<StartupPreparationStep> {
        self.drain(max)?;
        match if self.notice.stop_received() {
            Ok(HeadlessStep::Shutdown)
        } else {
            self.progress()
        }? {
            HeadlessStep::Shutdown => Ok(StartupPreparationStep::Closing),
            HeadlessStep::Active if self.stage == Stage::Running => {
                Ok(StartupPreparationStep::Ready)
            }
            HeadlessStep::Active => Ok(StartupPreparationStep::Waiting),
        }
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.drain(max)?;
        if self.closing {
            Ok(HeadlessStep::Active)
        } else if self.notice.stop_received() {
            Ok(HeadlessStep::Shutdown)
        } else {
            self.progress()
        }
    }
    fn next_deadline(&self) -> Option<Instant> {
        if self.closing {
            self.cleanup_wake
        } else if self.stage == Stage::Running {
            None
        } else {
            self.setup_wake
        }
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.begin_shutdown()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        let mut failure = None;
        retain(&mut failure, self.drain(NonZeroUsize::MIN.saturating_add(31)));
        let settled = match self.retire() {
            Ok(settled) => settled,
            Err(cause) => {
                retain(&mut failure, Err(cause));
                false
            }
        };
        failure.map_or(Ok(settled), Err)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if let Some(execution) = &mut self.execution {
            execution.finish_shutdown()?;
        }
        Ok(())
    }
}
/// Independent owners remain observable even when an earlier owner failed during this turn.
fn retain(failure: &mut Option<Box<dyn Error>>, result: Result<()>) {
    if let Err(cause) = result {
        if failure.is_none() {
            *failure = Some(cause);
        } else {
            tracing::error!(operation="recovery_supervisor_retirement",cause=%cause,"Additional original recovery owner failure retained");
        }
    }
}
