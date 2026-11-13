// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Isolate future polling and actual future/outcome/panic-payload destruction.

use super::*;
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) fn borrowed_panic_cause(payload: &(dyn Any + Send)) -> PanicCause {
    let text = if let Some(text) = payload.downcast_ref::<String>() {
        text.as_str()
    } else if let Some(text) = payload.downcast_ref::<&str>() {
        *text
    } else {
        "Non-text panic payload"
    };
    let mut end = text.len().min(1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    PanicCause::new(text[..end].to_owned())
}

pub(crate) fn panic_cause(payload: Box<dyn Any + Send>) -> PanicCause {
    let first = borrowed_panic_cause(payload.as_ref()).as_str().to_owned();
    let mut next = Some(payload);
    let mut destructor_failed = false;
    while let Some(payload) = next.take() {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => {}
            Err(payload) => {
                destructor_failed = true;
                next = Some(payload);
            }
        }
    }
    let detail = if destructor_failed {
        format!("{first}; panic payload destruction also panicked")
    } else {
        first
    };
    PanicCause::new(detail)
}

struct Supervised<F> {
    future: Pin<Box<F>>,
}
impl<F: Future> Future for Supervised<F> {
    type Output = Result<F::Output, PanicCause>;
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match catch_unwind(AssertUnwindSafe(|| this.future.as_mut().poll(context))) {
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(panic_cause(payload))),
        }
    }
}

pub(crate) async fn run_async<T, E, F, M>(
    job: Arc<Job<T, E>>,
    context: JobContext,
    _dispatched: Moment,
    factory: M,
) where
    T: Send + 'static,
    E: Send + 'static,
    F: Future<Output = Result<T, E>> + Send + 'static,
    M: FnOnce(JobContext) -> F + Send + 'static,
{
    let started = context.now();
    if let Some(reason) = stop_reason(&context) {
        let outcome = match catch_unwind(AssertUnwindSafe(|| drop(factory))) {
            Ok(()) => stopped_outcome(reason),
            Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
        };
        job.finish(context, Some(started), outcome);
        return;
    }
    let constructed = catch_unwind(AssertUnwindSafe(|| factory(context.clone())));
    let outcome = match constructed {
        Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
        Ok(future) => {
            let mut future = Supervised { future: Box::pin(future) };
            let deadline = context.deadline();
            let deadline_wait = async {
                match deadline {
                    Some(deadline) => context.sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            };
            let outcome = tokio::select! {
                biased;
                () = deadline_wait => JobOutcome::TimedOut,
                () = context.cancelled() => JobOutcome::Cancelled,
                result = &mut future => match result { Ok(Ok(value)) => JobOutcome::Success(value), Ok(Err(error)) => JobOutcome::Failure(error), Err(cause) => JobOutcome::Panicked(cause) },
            };
            let outcome = stop_after_failure(&context, outcome);
            match catch_unwind(AssertUnwindSafe(|| drop(future))) {
                Ok(()) => outcome,
                Err(payload) => {
                    let cause = panic_cause(payload);
                    let discarded = catch_unwind(AssertUnwindSafe(|| drop(outcome)));
                    let cause = match discarded {
                        Ok(()) => cause,
                        Err(payload) => {
                            let secondary = panic_cause(payload);
                            PanicCause::new(format!(
                                "{}; discarded outcome: {}",
                                cause.as_str(),
                                secondary.as_str()
                            ))
                        }
                    };
                    JobOutcome::Panicked(cause)
                }
            }
        }
    };
    job.finish(context, Some(started), outcome);
}

/// Classify a failure that completed while a stop was already requested as that stop.
///
/// The biased select observes a stop only before polling the future, so a cancellation or
/// deadline that arrives during that same poll can surface as the future's own failure. As for
/// blocking jobs, the stop takes precedence; a failure without a pending stop stays a failure and
/// successful values are kept.
fn stop_after_failure<T, E>(context: &JobContext, outcome: JobOutcome<T, E>) -> JobOutcome<T, E> {
    match outcome {
        JobOutcome::Failure(error) => match stop_reason(context) {
            Some(reason) => match catch_unwind(AssertUnwindSafe(|| drop(error))) {
                Ok(()) => stopped_outcome(reason),
                Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
            },
            None => JobOutcome::Failure(error),
        },
        outcome => outcome,
    }
}
