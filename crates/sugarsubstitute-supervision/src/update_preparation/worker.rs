// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite preparation steps and observe actual outcomes without waiting on the UI thread.
use super::{
    state::{Phase, State},
    *,
};
use crate::update_repository::{Failure, Repository};
use std::{
    io,
    sync::{Arc, Mutex, TryLockError},
};
use sugarsubstitute_contracts::execution::{
    ExecutionId, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
    Scope,
};

/// Actual execution outcome and retained domain cause; neither cancellation nor timeout loses it.
pub struct Observation {
    /// Boundary reached after the execution owner confirms that the step stopped.
    pub progress: Progress,
    /// Original execution outcome, including panic diagnostics and effective deadline expiry.
    pub outcome: JobOutcome<(), Arc<Failure>>,
    /// Original repository/storage error, even if execution cancelled delivery of its result.
    pub failure: Option<Arc<Failure>>,
    /// Cleanup failure retained separately so it never replaces the preparation's original cause.
    pub retirement_failure: Option<Arc<Failure>>,
}

/// Host-owned campaign, independent of the application's run lease and process custody.
///
/// Submit and poll from the host's event loop. Each successful poll permits one more submission;
/// failures require explicit retirement. On close call `cancel`, observe the outstanding job,
/// then submit retirement in a live cleanup scope. Keep this owner until `Closed`, or transfer its
/// ready candidate. Scope cancellation alone is not retirement. There is no internal retry timer.
#[must_use = "Retain the worker through observed retirement or explicit candidate transfer"]
pub struct Worker {
    state: Arc<Mutex<State>>,
    job: Option<OwnedJob<(), Arc<Failure>>>,
    owner: Option<ExecutionId>,
    progress: Progress,
    closing: bool,
    halted: bool,
}
impl Worker {
    /// Construct inert state without touching disk, network, execution, or native exclusions.
    /// # Errors
    /// Rejects relative paths and zero idle bounds. Required engines are prepared independently.
    pub fn new(configuration: Configuration) -> io::Result<Self> {
        Self::configured(configuration, false)
    }

    /// Reopen a verified completed candidate without issuing HTTP or extracting provisioning files.
    /// Current subscription/engine facts and existing trust remain mandatory. Incomplete candidates
    /// use authenticated archive repair; absent repair bytes yield `Unavailable`. Invalid, expired
    /// or revoked evidence remains an observed error.
    /// # Errors
    /// Applies the same inert configuration validation as online preparation.
    pub fn resume_cached(configuration: Configuration) -> io::Result<Self> {
        Self::configured(configuration, true)
    }

    /// Keep online and restart campaigns under the same admission and retirement contract.
    fn configured(configuration: Configuration, cached: bool) -> io::Result<Self> {
        if !configuration.installation.is_absolute()
            || !configuration.workspace.is_absolute()
            || configuration.idle.is_zero()
        {
            return Err(invalid("Invalid automatic preparation configuration"));
        }
        let state = State {
            repository: Repository::new(configuration.policy, configuration.idle),
            installation: configuration.installation,
            workspace: configuration.workspace,
            repository_id: configuration.repository,
            cached,
            subscription: None,
            phase: Phase::Open,
            progress: Progress::Opening,
            failure: None,
            retirement_failure: None,
        };
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            job: None,
            owner: None,
            progress: Progress::Opening,
            closing: false,
            halted: false,
        })
    }

    /// Last observed completed boundary; reading it never locks against running native work.
    pub fn progress(&self) -> Progress {
        self.progress
    }

    /// Whether an admitted step still requires its actual terminal observation.
    pub fn pending(&self) -> bool {
        self.job.is_some()
    }

    /// Admit the next finite step using the host's increasing job identity and monotonic deadline.
    /// `now` returns current Unix UTC seconds whenever authentication needs them. The host owns
    /// clock-failure handling and retry/backoff policy. Callers drain executor wake metadata as
    /// usual; `poll` takes this job's typed completion. Cleanup uses reserved lifecycle capacity.
    /// # Errors
    /// Preserves admission failures; rejected factories leave all original custody unchanged.
    /// Rejects overlapping work and advancement after a terminal decision or failed step.
    /// An admitted campaign remains bound to its original execution owner's memory ledger.
    pub fn submit(
        &mut self,
        execution: &impl ExecutionPort,
        scope: &Scope,
        id: JobId,
        deadline: Moment,
        now: impl FnMut() -> i64 + Send + 'static,
    ) -> io::Result<()> {
        if self.job.is_some()
            || self.owner.is_some_and(|owner| owner != scope.owner_id())
            || self.progress == Progress::Closed
            || (!self.closing
                && (self.halted
                    || matches!(
                        self.progress,
                        Progress::Ready
                            | Progress::EngineReady
                            | Progress::AlreadyPreparing
                            | Progress::Decision(_)
                            | Progress::Unavailable
                            | Progress::Failed
                    )))
        {
            return Err(invalid("Update preparation cannot admit another step"));
        }
        let retiring = self.closing;
        let state = Arc::clone(&self.state);
        let spec = JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: if retiring {
                JobClass::InfrastructureLifecycle
            } else {
                JobClass::InfrastructureOrdinary
            },
            deadline: Some(deadline),
            reservation_bytes: self.progress.reservation(),
            replacement: None,
        };
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let mut state = crate::registry::lock(&state);
                if retiring {
                    state.retire().map_err(|cause| {
                        let cause = Arc::new(cause);
                        state.retirement_failure = Some(cause.clone());
                        cause
                    })
                } else {
                    state.advance(now, &context)
                }
            })
            .map_err(io::Error::other)?;
        self.job = Some(job);
        self.owner = Some(scope.owner_id());
        Ok(())
    }

    /// Request cancellation without claiming completion or acquiring the worker's mutex.
    /// An already admitted cleanup step remains observed and is never cancelled by this method.
    pub fn cancel(&mut self) {
        if self.progress == Progress::Closed {
            return;
        }
        if !self.closing
            && let Some(job) = &self.job
        {
            job.cancel();
        }
        self.closing = true;
        self.progress = Progress::Retiring;
    }

    /// Nonblocking terminal observation. No native/storage retirement happens on this caller.
    pub fn poll(&mut self) -> Option<Observation> {
        let completion = self.job.as_mut()?.try_take_completion()?;
        self.job = None;
        let state = crate::registry::lock(&self.state);
        let success = matches!(completion.outcome, JobOutcome::Success(()));
        self.halted = !success || state.progress == Progress::Failed;
        self.progress = if self.closing && state.progress != Progress::Closed {
            Progress::Retiring
        } else if !success && !self.closing {
            Progress::Failed
        } else {
            state.progress
        };
        Some(Observation {
            progress: self.progress,
            outcome: completion.outcome,
            failure: state.failure.clone(),
            retirement_failure: state.retirement_failure.clone(),
        })
    }

    /// Transfer a fully observed prepared candidate to its activation owner without native I/O.
    /// # Errors
    /// Rejects in-flight, failed, cancelled, or incomplete preparation. Activation must separately
    /// acquire run exclusion and reauthorize; this method never starts the candidate.
    pub fn take_candidate(&mut self) -> io::Result<Box<SignedCandidate>> {
        match self.transfer(Progress::Ready)? {
            Phase::Ready(candidate) => Ok(candidate),
            _ => Err(invalid("Application candidate custody is absent")),
        }
    }

    /// Transfer fully observed updater custody without I/O or granting new process capabilities.
    /// Prepared engines require supervised health acceptance; accepted images may be launched
    /// using their own executable. Neither case authorizes this process to impersonate that engine.
    /// # Errors
    /// Rejects in-flight, canceled, failed or incomplete preparation and unsettled HTTP custody.
    pub fn take_engine(&mut self) -> io::Result<EngineUpdate> {
        match self.transfer(Progress::EngineReady)? {
            Phase::EngineReady(engine) => Ok(engine),
            _ => Err(invalid("Updater custody is absent")),
        }
    }

    /// Release a ready phase only after typed completion and actual HTTP retirement.
    fn transfer(&mut self, expected: Progress) -> io::Result<Phase> {
        if self.pending() || self.closing || self.halted || self.progress != expected {
            return Err(invalid("Update custody is not ready for transfer"));
        }
        let mut state = match self.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(cause)) => cause.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(invalid("Update worker still owns state")),
        };
        if !state.repository.settled() {
            return Err(invalid("Update HTTP custody remains"));
        }
        if !matches!(
            (&state.phase, expected),
            (Phase::Ready(_), Progress::Ready) | (Phase::EngineReady(_), Progress::EngineReady)
        ) {
            return Err(invalid("Prepared update custody is absent"));
        }
        let phase = std::mem::replace(&mut state.phase, Phase::Empty);
        state.progress = Progress::Closed;
        self.progress = Progress::Closed;
        Ok(phase)
    }
}

/// Internal protocol errors retain their I/O category without becoming product copy.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
