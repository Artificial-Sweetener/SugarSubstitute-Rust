// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate explicit conflict decisions, finite native work and a fresh launch after retirement.
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_application::instance_conflict::Phase;
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_contracts::instance_conflict::{
    ConflictAction, ConflictBackend, ConflictController, ConflictObservation as Observation,
    ConflictOperation as Op, ConflictView,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Drive conflict resolution through admitted jobs and a platform-owned backend.
/// The caller retains this service until `finished`, including surface failures.
pub struct Service<B: ConflictBackend> {
    session: B,
    clock: Arc<dyn Clock>,
    relaunch_started: bool,
    replacement: Option<OwnedJob<(), B::Error>>,
    native_retired: bool,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    jobs: crate::job_sequence::JobSequence,
    pending: Option<(Op, OwnedJob<Observation, B::Error>)>,
    wanted: Option<Op>,
    phase: Phase,
    next_poll: Moment,
    stopping: bool,
    failure: Option<Box<dyn Error>>,
}
impl<B: ConflictBackend> Service<B> {
    /// Retain an inert backend; native work starts only when the surface supplies its wake.
    pub fn new(session: B) -> Self {
        Self {
            session,
            clock: MonotonicClock::new(),
            relaunch_started: false,
            replacement: None,
            native_retired: false,
            owner: None,
            client: None,
            scope: None,
            jobs: crate::job_sequence::JobSequence::new(),
            pending: None,
            wanted: Some(Op::Discover),
            phase: Phase::Checking,
            next_poll: Moment::from_nanos(0),
            stopping: false,
            failure: None,
        }
    }
    /// True only after native custody and all execution workers have retired.
    pub fn finished(&self) -> bool {
        matches!(self.phase, Phase::Done(_))
    }
    /// Whether a completed resolution launched a fresh application.
    pub fn continued(&self) -> bool {
        self.phase == Phase::Done(true)
    }
    fn initialize(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() {
            return Ok(());
        }
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
        self.client = Some(owner.client());
        self.owner = Some(owner);
        self.scope = Some(
            self.client
                .as_ref()
                .ok_or("Conflict execution missing")?
                .create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        Ok(())
    }
    fn fail(&mut self, cause: Box<dyn Error>) {
        tracing::error!(operation="profile_conflict_resolution", cause=?cause, "Profile conflict resolution failed with original cause retained");
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
        self.wanted = None;
        self.phase = Phase::Failed;
    }
    fn drive(&mut self) -> Result<()> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        client.drain_completions(NonZeroUsize::new(32).ok_or("Conflict drain bound")?)?;
        if let Some((operation, job)) = self.pending.as_mut()
            && let Some(completion) = job.try_take_completion()
        {
            let operation = *operation;
            self.pending = None;
            let observation = match completion.outcome {
                JobOutcome::Success(value) => value,
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Profile conflict operation did not complete successfully".into()),
            };
            if operation == Op::Retire {
                if observation != Observation::Retired {
                    return Err("Conflict backend did not confirm actual retirement".into());
                }
                self.native_retired = true;
            } else if !matches!(self.phase, Phase::Retiring(_)) {
                match observation {
                    Observation::Owner(_) => {
                        self.phase = Phase::Ready;
                    }
                    Observation::Unresponsive
                        if matches!(self.phase, Phase::Checking | Phase::AwaitingExit) =>
                    {
                        self.phase = Phase::Unresponsive;
                    }
                    Observation::Closing if self.phase == Phase::Checking => {
                        self.phase = Phase::AwaitingExit;
                    }
                    Observation::Unresponsive | Observation::Closing => {}
                    Observation::Released => {
                        self.phase = Phase::Retiring(true);
                        self.wanted = Some(Op::Retire);
                    }
                    Observation::Waiting => {}
                    Observation::Retired => {
                        return Err("Unexpected profile conflict retirement".into());
                    }
                }
            }
        }
        if let Some(job) = self.replacement.as_mut()
            && let Some(completion) = job.try_take_completion()
        {
            self.replacement = None;
            match completion.outcome {
                JobOutcome::Success(()) => self.stopping = true,
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Replacement launch did not complete successfully".into()),
            }
        }
        if self.native_retired && !self.stopping && self.replacement.is_none() {
            if self.phase == Phase::Retiring(true) && !self.relaunch_started {
                let launch = self.session.clone();
                self.relaunch_started = true;
                self.replacement = Some(client.submit_blocking(
                    self.scope.as_ref().ok_or("Conflict scope missing")?,
                    JobSpec {
                        id: self.jobs.allocate()?,
                        operation: Operation::Infrastructure,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(client.now()?.checked_add(Duration::from_secs(30))?),
                        reservation_bytes: 1024 * 1024,
                        replacement: None,
                    },
                    move |context| launch.relaunch(&context),
                )?);
            } else if matches!(self.phase, Phase::Retiring(false)) {
                // Any replacement outputs must be retired in an admitted worker before shutdown.
                if self.relaunch_started {
                    let launch = self.session.clone();
                    self.replacement = Some(client.submit_blocking(
                        self.scope.as_ref().ok_or("Conflict scope missing")?,
                        JobSpec {
                            id: self.jobs.allocate()?,
                            operation: Operation::Infrastructure,
                            class: JobClass::InfrastructureLifecycle,
                            deadline: Some(client.now()?.checked_add(Duration::from_secs(30))?),
                            reservation_bytes: 1024 * 1024,
                            replacement: None,
                        },
                        move |_| launch.retire_relaunch(),
                    )?);
                } else {
                    self.stopping = true;
                }
            }
            if self.stopping {
                self.owner
                    .as_mut()
                    .ok_or("Conflict owner missing")?
                    .request_shutdown(client.now()?.checked_add(Duration::from_secs(30))?);
            }
        }
        if self.stopping {
            let owner = self.owner.as_mut().ok_or("Conflict owner missing")?;
            owner.request_shutdown(client.now()?.checked_add(Duration::from_secs(30))?);
            if owner.poll_shutdown()?.settled {
                owner.finish_shutdown()?;
                self.phase = Phase::Done(matches!(self.phase, Phase::Retiring(true)));
                self.owner = None;
                self.client = None;
                self.scope = None;
            }
            return Ok(());
        }
        if self.pending.is_some() {
            return Ok(());
        }
        if self.wanted.is_none()
            && matches!(
                self.phase,
                Phase::Checking | Phase::Closing(_) | Phase::Forcing | Phase::AwaitingExit
            )
            && self.clock.now() >= self.next_poll
        {
            self.wanted = Some(Op::Observe);
        }
        let Some(operation) = self.wanted.take() else {
            return Ok(());
        };
        let session = self.session.clone();
        let spec = JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(client.now()?.checked_add(Duration::from_secs(30))?),
            reservation_bytes: 4 * 1024 * 1024,
            replacement: None,
        };
        self.pending = Some((
            operation,
            client.submit_blocking(
                self.scope.as_ref().ok_or("Conflict scope missing")?,
                spec,
                move |context| session.execute(operation, &context),
            )?,
        ));
        self.next_poll = self.clock.now().checked_add(Duration::from_millis(250))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
impl<B: ConflictBackend> ConflictController for Service<B> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) {
        if let Err(cause) = self.initialize(wake) {
            self.fail(cause);
        }
    }
    fn poll(&mut self) -> ConflictView {
        if let Err(cause) = self.drive() {
            self.fail(cause);
        }
        self.phase.view(self.clock.now())
    }
    fn choose(&mut self, action: ConflictAction) {
        if let Some(operation) = self.phase.choose(action, self.clock.now()) {
            if operation == Op::Retire && self.scope.is_none() {
                if let (Some(owner), Some(client)) = (&mut self.owner, &self.client) {
                    let deadline: Result<_> =
                        (|| Ok(client.now()?.checked_add(Duration::from_secs(30))?))();
                    match deadline {
                        Ok(deadline) => {
                            owner.request_shutdown(deadline);
                            self.stopping = true;
                        }
                        Err(cause) => self.fail(cause),
                    }
                } else {
                    self.phase = Phase::Done(false);
                }
                self.wanted = None;
                return;
            }
            self.wanted = Some(operation);
        }
    }
}
impl<B: ConflictBackend> HeadlessLifetime for Service<B> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        self.initialize(wake)
    }
    fn prepare_native_runtime(&mut self, _: NonZeroUsize) -> Result<StartupPreparationStep> {
        self.drive()?;
        Ok(StartupPreparationStep::Ready)
    }
    fn activity(&mut self, _: NonZeroUsize) -> Result<HeadlessStep> {
        self.drive()?;
        Ok(HeadlessStep::Shutdown)
    }
    fn next_deadline(&self) -> Option<Instant> {
        Some(Instant::now() + Duration::from_millis(25))
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.choose(ConflictAction::Cancel);
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        self.drive()?;
        Ok(matches!(self.phase, Phase::Done(_)))
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if matches!(self.phase, Phase::Done(_)) {
            Ok(())
        } else {
            Err("Conflict work remains unretired".into())
        }
    }
}
