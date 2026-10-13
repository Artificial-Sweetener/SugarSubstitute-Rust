// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native calls borrow durable ownership without holding admission/lifetime locks.
use super::loan::NativeLoan;
use super::*;
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::JobContext};

pub(super) fn start(
    shared: Arc<Shared>,
    owner: CoordinatorKey,
    context: JobContext,
) -> Result<(), CrashError> {
    crate::native::check_deadline(&context)?;
    let admission = shared
        .execution
        .capture_bundle_admission(&context)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let memory = context
        .memory()
        .claim(crate::capture_endpoint::SOURCE_BYTES)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    let attempts = crate::capture_endpoint::Attempts::new(owner, memory)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    *lock(&shared.attempts) = Some(attempts);
    let lease = shared.port.create(admission, shared.execution.clone())?;
    let mut loan = NativeLoan::install(shared.clone(), lease, context.deadline())?;
    let input = lock(&shared.input)
        .take()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?;
    let result = loan.lease()?.start(input, owner, &context);
    let mut first = result.err();
    if let Some(cause) = &first {
        lock(&shared.native).retain(cause.clone());
    }
    match loan.lease()?.build_identity().map(CaptureBuildIdentity::new).transpose() {
        Ok(build) => *lock(&shared.build) = build,
        Err(cause) => {
            lock(&shared.native).retain(cause.clone());
            if first.is_none() {
                first = Some(cause);
            }
        }
    }
    let creator = loan.lease()?.creator();
    let handler = loan.lease()?.handler();
    {
        let mut native = lock(&shared.native);
        native.handler = handler;
        native.creator = creator;
    }
    // Every external call is outside Shared locks; failed starts still bind their
    // actual retained process before interpreting healthy publication.
    let permit = lock(&shared.permit).take();
    if let Some(permit) = permit {
        let watch = if loan.lease()?.has_process() {
            match loan.lease()?.bind_watch(permit) {
                Ok(watch) => watch,
                Err(failure) => {
                    if first.is_none() {
                        first = Some(failure.cause.clone());
                    }
                    lock(&shared.native).retain(failure.cause);
                    failure.permit.cancel_unbound()
                }
            }
        } else {
            permit.cancel_unbound()
        };
        lock(&shared.native).watch = Some(watch);
    } else if first.is_none() {
        first = Some(CrashError::invalid(CaptureOperation::Observation));
    }
    let registered = loan.lease()?.registered();
    let now = context.now();
    let cancelled = context.cancellation_requested();
    if (cancelled || context.deadline().is_none_or(|deadline| now >= deadline)) && first.is_none() {
        first = Some(CrashError::invalid(CaptureOperation::Registration));
    }
    loan.drain_errors()?;
    let mut native = lock(&shared.native);
    native.registered =
        first.is_none() && registered && native.handler.is_some() && native.watch.is_some();
    if let Some(cause) = first {
        native.retain(cause.clone());
        Err(cause)
    } else if !native.registered {
        let cause = CrashError::invalid(CaptureOperation::Registration);
        native.retain(cause.clone());
        Err(cause)
    } else {
        Ok(())
    }
}

pub(super) fn close(shared: Arc<Shared>, context: JobContext) -> Result<(), CrashError> {
    let borrowed = {
        let mut attempts = lock(&shared.attempts);
        if let Some(attempts) = attempts.as_mut() {
            attempts.close();
            attempts.has_unretired()
        } else {
            false
        }
    };
    if borrowed {
        return Ok(());
    }
    let deadline =
        context.deadline().ok_or_else(|| CrashError::invalid(CaptureOperation::Retirement))?;
    {
        let mut native = lock(&shared.native);
        if native.borrowed || native.close_deadline.is_some_and(|old| deadline > old) {
            return Err(CrashError::invalid(CaptureOperation::Retirement));
        }
        native.close_deadline =
            Some(native.close_deadline.map_or(deadline, |old| old.min(deadline)));
    }
    crate::native::check_deadline(&context)?;
    let Some(mut loan) = NativeLoan::take(shared.clone(), CaptureOperation::Retirement)? else {
        lock(&shared.native).retired = true;
        return Ok(());
    };
    let mut first = loan.lease()?.request_stop().err();
    if let Some(cause) = &first {
        lock(&shared.native).retain(cause.clone());
    }
    let retire = loan.lease()?.retire(&context);
    lock(&shared.native).retired = matches!(retire, Ok(true));
    if let Err(cause) = retire {
        if first.is_none() {
            first = Some(cause.clone());
        }
        lock(&shared.native).retain(cause);
    }
    loan.drain_errors()?;
    first.map_or(Ok(()), Err)
}
