// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own one bounded process-registration retirement thread through its actual join.

use super::{NativeWork, ProcessLease, Signal};
use crate::{
    admission::Core,
    scope::lock,
    task::{self, QueuedJob, Work},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    thread::{self, JoinHandle},
};
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, Moment, PanicCause,
};

struct Active {
    work: NativeWork,
    context: JobContext,
    started: Moment,
    signal: Arc<Signal>,
    lease: Option<Box<dyn ProcessLease>>,
    outcome: Option<JobOutcome<super::ProcessExit, super::NativeObservationError>>,
    initialized: bool,
    completed: bool,
}
impl Active {
    fn new(queued: QueuedJob, core: &Arc<Core>, now: Moment) -> Option<Self> {
        let Work::Native(work) = queued.work else {
            return None;
        };
        let lease = work.binding.take();
        Some(Self {
            work,
            context: queued.context,
            started: now,
            signal: Signal::new(core),
            lease,
            outcome: None,
            initialized: false,
            completed: false,
        })
    }
    fn fail_owner(&mut self, cause: &PanicCause) {
        self.initialized = true;
        if !matches!(self.outcome, Some(JobOutcome::Failure(_))) {
            self.outcome = Some(JobOutcome::Panicked(cause.clone()));
        }
    }
    fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        if !self.initialized {
            self.initialized = true;
            if let Some(reason) = task::stop_reason_at(&self.context, now) {
                self.outcome = Some(task::stopped_outcome(reason));
            } else if let Some(lease) = &mut self.lease {
                if let Err(error) = lease.start(self.work.correlation, self.signal.clone()) {
                    self.outcome = Some(JobOutcome::Failure(error));
                }
            } else {
                self.outcome = Some(JobOutcome::Cancelled);
            }
        }
        if self.outcome.is_none() {
            // Registration or an earlier lease retirement may have taken time;
            // expired work cannot use the iteration's earlier clock sample.
            let observed = if clock_available { core.clock.now() } else { now };
            if let Some(reason) = task::stop_reason_at(&self.context, observed) {
                self.outcome = Some(task::stopped_outcome(reason));
            } else if self.signal.completed() {
                self.outcome = Some(match self.lease.as_mut() {
                    Some(lease) => match lease.read_exit() {
                        Ok(exit) => JobOutcome::Success(exit),
                        Err(error) => JobOutcome::Failure(error),
                    },
                    None => JobOutcome::Failure(super::NativeObservationError::new(
                        "read_process_exit",
                        std::io::Error::other("Process lease missing"),
                    )),
                });
            }
        }
        if self.outcome.is_none() {
            return;
        }
        if let Some(lease) = &mut self.lease
            && let Err(error) = lease.retire()
        {
            match &mut self.outcome {
                Some(JobOutcome::Failure(primary)) => primary.retain_cleanup(error.clone()),
                outcome => *outcome = Some(JobOutcome::Failure(error.clone())),
            }
            core.mailbox.record_failure(ExecutionError::new(
                self.context.spec().operation,
                Box::new(error),
            ));
            core.mailbox.deliver_wake();
            return;
        }
        // Only a successful retire permits destroying the callback/handle owner.
        drop(self.lease.take());
        // Observe the end after synchronous retirement actually returns. A clock
        // panic here is caught with this Active and its outcome still retained.
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}

pub(crate) struct NativeWorker {
    core: Arc<Core>,
    handle: Option<JoinHandle<()>>,
}
impl NativeWorker {
    pub(crate) fn start(core: Arc<Core>) -> Result<Self, ExecutionError> {
        let worker_core = core.clone();
        let handle = thread::Builder::new()
            .name("sugarsubstitute-native-retirement".into())
            .spawn(move || worker(worker_core))
            .map_err(crate::runtime::error)?;
        Ok(Self { core, handle: Some(handle) })
    }
    pub(crate) fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub(crate) fn finish(mut self) -> Result<(), ExecutionError> {
        self.join()
    }
    fn join(&mut self) -> Result<(), ExecutionError> {
        if let Some(handle) = self.handle.take() {
            handle.join().map_err(|payload| {
                crate::runtime::error(std::io::Error::other(
                    task::panic_cause(payload).as_str().to_owned(),
                ))
            })?;
        }
        Ok(())
    }
}
impl Drop for NativeWorker {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.core.shutdown();
            if let Err(error) = self.join() {
                tracing::error!(operation="native_retirement_join",cause=%error,"Native retirement worker teardown failed");
            }
        }
    }
}

fn owner_panicked(
    core: &Core,
    fatal: &mut Option<PanicCause>,
    payload: Box<dyn std::any::Any + Send>,
) {
    let cause = task::panic_cause(payload);
    core.mailbox
        .record_failure(crate::runtime::error(std::io::Error::other(cause.as_str().to_owned())));
    if fatal.is_none() {
        *fatal = Some(cause);
        core.shutdown();
    }
    core.mailbox.deliver_wake();
}

fn worker(core: Arc<Core>) {
    if core.native_thread.set(thread::current()).is_err() {
        core.mailbox.record_failure(crate::runtime::error(std::io::Error::other(
            "Native worker identity installed twice",
        )));
        return;
    }
    // Active native ownership never moves into an unwinding closure. Fatal
    // boundary/clock failure stops admission, then retires every retained lease.
    let mut active: Vec<Active> = Vec::with_capacity(512);
    let mut fatal = None;
    let mut now = Moment::from_nanos(0);
    loop {
        if fatal.is_none() {
            match catch_unwind(AssertUnwindSafe(|| core.clock.now())) {
                Ok(observed) => now = observed,
                Err(payload) => owner_panicked(&core, &mut fatal, payload),
            }
        }
        while let Some((queued, wake)) = core.take_native_at(now) {
            if let Some(item) = Active::new(queued, &core, now) {
                active.push(item);
            }
            if let Err(payload) =
                catch_unwind(AssertUnwindSafe(|| core.deliver_capacity_wake(wake)))
            {
                owner_panicked(&core, &mut fatal, payload);
            }
        }
        let mut index = 0;
        while index < active.len() {
            if let Some(cause) = &fatal {
                active[index].fail_owner(cause);
            }
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                active[index].progress(&core, now, fatal.is_none())
            })) {
                owner_panicked(&core, &mut fatal, payload);
            }
            if active[index].completed {
                active.swap_remove(index);
            } else {
                index += 1;
            }
        }
        let settled = {
            let state = lock(&core.state);
            state.stopping && state.settled()
        };
        if settled && active.is_empty() {
            break;
        }
        // unpark retains a token, so bind/callback/cancel cannot be lost between inspection and park.
        thread::park();
    }
}
