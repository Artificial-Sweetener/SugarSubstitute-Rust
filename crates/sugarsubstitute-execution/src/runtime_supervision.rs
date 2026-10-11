// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain asynchronous and finite-blocking joins and observe every stop.

use crate::{
    admission::Core,
    scope::lock,
    task::{self, Work},
};
use std::{future::Future, sync::Arc};
use sugarsubstitute_contracts::execution::Moment;
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
        let (stopping, deadline) = {
            let state = lock(&core.state);
            (
                state.stopping,
                state
                    .jobs
                    .values()
                    .filter(|record| {
                        record.phase == sugarsubstitute_contracts::execution::JobPhase::Queued
                            || (record.spec.class
                                == sugarsubstitute_contracts::execution::JobClass::NativeObservation
                                && record.phase
                                    == sugarsubstitute_contracts::execution::JobPhase::Running)
                    })
                    .filter_map(|record| record.deadline)
                    .filter(|deadline| *deadline > now)
                    .min(),
            )
        };
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
