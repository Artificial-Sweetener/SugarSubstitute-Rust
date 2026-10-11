// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain asynchronous and finite-blocking joins and observe every stop.

use crate::{
    admission::{Core, State},
    scope::lock,
    task::{self, Work},
};
use std::{future::Future, sync::Arc};
use sugarsubstitute_contracts::execution::{JobClass, JobPhase, Moment};
use tokio::task::JoinHandle;

struct Running {
    handle: JoinHandle<()>,
    context: sugarsubstitute_contracts::execution::JobContext,
    job: Arc<dyn task::ErasedJob>,
    started: Moment,
}

pub(super) async fn supervise(core: Arc<Core>) {
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
                let stopped = running.swap_remove(index);
                if let Err(error) = stopped.handle.await {
                    stopped.job.panicked(
                        &stopped.context,
                        stopped.started,
                        sugarsubstitute_contracts::execution::PanicCause::new(error.to_string()),
                    );
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
            if let Some(reason) = task::stop_reason(&queued.context) {
                task::stop_queued(queued, reason);
                continue;
            }
            let started = core.clock.now();
            let future = match queued.work {
                Work::Async(factory) | Work::Blocking(factory) => {
                    factory(queued.context.clone(), started)
                }
                Work::Cpu(_) => continue,
                #[cfg(any(windows, test))]
                Work::Native(_) => continue,
            };
            running.push(Running {
                handle: tokio::spawn(future),
                context: queued.context,
                job: queued.job,
                started,
            });
        }
        let now = core.clock.now();
        let (stopping, deadline, expired_native) = {
            let mut state = lock(&core.state);
            let timers_enabled = !state.stopping;
            let (mut deadline, mut expired_native) = deadlines(&state, now);
            #[cfg(any(windows, test))]
            for record in state.jobs.values_mut() {
                if !timers_enabled {
                    continue;
                }
                if !matches!(record.phase, JobPhase::Queued | JobPhase::Running) {
                    continue;
                }
                if let Some(slots) = &mut record.native_deadlines {
                    let (command_deadline, command_expired) = slots.scan(now);
                    expired_native |= command_expired;
                    if let Some(command_deadline) = command_deadline {
                        deadline = Some(
                            deadline
                                .map_or(command_deadline, |earlier| earlier.min(command_deadline)),
                        );
                    }
                }
            }
            (state.stopping, deadline, expired_native)
        };
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
                Some(deadline) => core.clock.sleep_until(deadline).await,
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
                if let Err(error) = result { stopped.job.panicked(&stopped.context, stopped.started, sugarsubstitute_contracts::execution::PanicCause::new(error.to_string())); }
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
        let native = record.spec.class == JobClass::NativeObservation;
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
