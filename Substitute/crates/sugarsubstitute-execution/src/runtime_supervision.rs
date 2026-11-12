// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain asynchronous and finite-blocking joins and observe every stop.

use crate::{
    admission::{Core, State},
    scope::lock,
    task::{self, QueuedJob, Work},
    worker_exit::WorkerExit,
};
use std::{future::Future, sync::Arc};
use sugarsubstitute_contracts::execution::{JobClass, JobPhase, Moment, PanicCause};
use tokio::task::{JoinError, JoinHandle};

/// A dispatched job's join. Its contexts moved into the task, so supervising it holds no
/// reservation; the task hands them to its jobs when it publishes their terminal outcomes.
struct Running {
    handle: JoinHandle<()>,
    job: Arc<dyn task::ErasedJob>,
    started: Moment,
    receiving: Option<Arc<dyn task::ErasedJob>>,
}

impl Running {
    /// Publish a panic or abort that ended the task before it published its own outcome.
    fn unwound(&self, error: &JoinError) {
        let cause = PanicCause::new(error.to_string());
        if let Some(receiving) = &self.receiving {
            receiving.panicked_unwound(self.started, cause.clone());
        }
        self.job.panicked_unwound(self.started, cause);
    }
}

/// The exit guard is the first parameter, so it drops after every retained join.
pub(super) async fn supervise(_exit: WorkerExit, core: Arc<Core>) {
    let mut running: Vec<Running> = Vec::with_capacity(14);
    loop {
        let notified = core.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        core.mailbox.flush_deferred_wake();
        core.flush_notifications();
        let mut index = 0;
        while index < running.len() {
            if running[index].handle.is_finished() {
                let mut stopped = running.swap_remove(index);
                if let Err(error) = (&mut stopped.handle).await {
                    stopped.unwound(&error);
                }
            } else {
                index += 1;
            }
        }
        while let Some(queued) = core.take(|class| {
            matches!(
                class,
                sugarsubstitute_contracts::execution::JobClass::IoOrdinary
                    | sugarsubstitute_contracts::execution::JobClass::IoLifecycle
                    | sugarsubstitute_contracts::execution::JobClass::InfrastructureOrdinary
                    | sugarsubstitute_contracts::execution::JobClass::InfrastructureLifecycle
            )
        }) {
            if let Some(reason) = queued.stop_reason(core.clock.retirement_now())
                && !matches!(queued.work, Work::BlockingHeld(_))
            {
                task::stop_queued(queued, reason);
                continue;
            }
            let started = core.clock.retirement_now();
            let QueuedJob { context, job, work } = queued;
            let (future, receiving) = match work {
                Work::ReportExportPair(pair) => {
                    // The pair task owns its receiving context; this admission copy only served
                    // the queued stop check.
                    let crate::report_export::pair::Work { context: _, job: receiving, factory } =
                        *pair;
                    (factory(context, started), Some(receiving))
                }
                Work::Async(factory) | Work::Blocking(factory) | Work::BlockingHeld(factory) => {
                    (factory(context, started), None)
                }
                Work::Cpu(_) => continue,
                #[cfg(any(windows, test))]
                Work::Native(_) => continue,
            };
            running.push(Running { handle: tokio::spawn(future), job, started, receiving });
        }
        let now = core.clock.retirement_now();
        let (stopping, deadline, expired_native, custody_notice) = {
            let mut state = lock(&core.state);
            let timers_enabled = !state.stopping;
            let (mut deadline, mut expired_native) = deadlines(&state, now);
            let mut custody_notice = false;
            #[cfg(any(windows, test))]
            for record in state.jobs.values_mut() {
                if record.phase == JobPhase::Running
                    && let Some(slots) = &mut record.custody_deadlines
                {
                    let (next, changed) = slots.scan(now);
                    expired_native |= changed;
                    custody_notice |= changed;
                    if let Some(next) = next {
                        deadline = Some(deadline.map_or(next, |old| old.min(next)));
                    }
                }
                let commands =
                    timers_enabled && matches!(record.phase, JobPhase::Queued | JobPhase::Running);
                let capture_cleanup = (record.native_custody.is_some() && record.held_native())
                    || (record.process_creation && record.phase == JobPhase::Running);
                if let Some(slots) = &mut record.native_deadlines {
                    let (command_deadline, command_expired) =
                        if commands { slots.scan(now) } else { (None, false) };
                    let (cleanup_deadline, cleanup_expired) = if capture_cleanup {
                        slots.scan_capture_cleanup(now)
                    } else {
                        (None, false)
                    };
                    expired_native |= cleanup_expired;
                    if let Some(cleanup_deadline) = cleanup_deadline {
                        deadline = Some(
                            deadline
                                .map_or(cleanup_deadline, |earlier| earlier.min(cleanup_deadline)),
                        );
                    }
                    expired_native |= command_expired;
                    if let Some(command_deadline) = command_deadline {
                        deadline = Some(
                            deadline
                                .map_or(command_deadline, |earlier| earlier.min(command_deadline)),
                        );
                    }
                }
            }
            (state.stopping, deadline, expired_native, custody_notice)
        };
        if custody_notice && core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
        // A notification may win select and discard an already-ready deadline
        // future. Independently wake parked native ownership at the fresh scan;
        // notifying this supervisor again would repeatedly poll expired work.
        #[cfg(any(windows, test))]
        if expired_native && let Some(thread) = core.native_thread.get() {
            thread.unpark();
        }
        #[cfg(not(any(windows, test)))]
        let _ = expired_native;
        if stopping && core.settled() && running.is_empty() {
            break;
        }
        let deadline_wait = async {
            match deadline {
                Some(deadline) => core.clock.retirement_sleep_until(deadline).await,
                None => std::future::pending().await,
            }
        };
        let has_running = !running.is_empty();
        let stopped = std::future::poll_fn(|context| {
            for (index, item) in running.iter_mut().enumerate() {
                if let std::task::Poll::Ready(result) =
                    std::pin::Pin::new(&mut item.handle).poll(context)
                {
                    return std::task::Poll::Ready((index, result));
                }
            }
            std::task::Poll::Pending
        });
        tokio::select! {
            () = &mut notified => {},
            () = deadline_wait => { core.changed(); },
            (index, result) = stopped, if has_running => {
                let stopped = running.swap_remove(index);
                if let Err(error) = result { stopped.unwound(&error); }
            }
        }
    }
}

fn deadlines(state: &State, now: Moment) -> (Option<Moment>, bool) {
    // Shutdown already wakes/cancels every owner. Expiry cannot schedule retries
    // of retained native teardown, including unrelated unbound native permits.
    if state.stopping {
        return (None, false);
    }
    let mut future = None;
    let mut expired_native = false;
    for record in state.jobs.values() {
        let native = record.spec.class == JobClass::NativeObservation
            || record.desktop_capture_child()
            || record.native_deadlines.is_some();
        if record.phase != JobPhase::Queued && !(native && record.phase == JobPhase::Running) {
            continue;
        }
        // A pipe that has selected stop still owns native retirement. Its
        // inherited deadline has already been consumed and cannot retry close.
        #[cfg(any(windows, test))]
        if record.native_deadlines.as_ref().is_some_and(|slots| slots.stopped()) {
            continue;
        }
        if let Some(deadline) = record.deadline {
            if deadline > now {
                future = Some(future.map_or(deadline, |earlier: Moment| earlier.min(deadline)));
            } else if native {
                expired_native = true;
            }
        }
    }
    (future, expired_native)
}

#[cfg(test)]
#[path = "runtime_supervision_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_supervision_pipe_tests.rs"]
mod pipe_tests;

#[cfg(test)]
#[path = "symbol_cleanup_timer_tests.rs"]
mod symbol_timer_tests;
