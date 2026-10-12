// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual native state is installed before effects and never returned as job data.
use super::*;
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::JobContext};

pub(super) fn start(
    shared: Arc<Shared>,
    owner: CoordinatorKey,
    context: JobContext,
) -> Result<(), CrashError> {
    crate::native::check_deadline(&context)?;
    let lease = shared.port.create(context.memory())?;
    let mut native = lock(&shared.native);
    native.lease = Some(lease);
    native.deadline = context.deadline();
    let input = lock(&shared.input)
        .take()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?;
    let mut first = None;
    if let Some(lease) = native.lease.as_mut() {
        let result = lease.start(&input, owner, &context);
        let handler = lease.handler();
        if let Err(cause) = result {
            native.retain(cause.clone());
            first = Some(cause);
        }
        native.handler = handler;
    }
    // Even a failed registration may have created a real handler. Its retained
    // output must be watched before interpreting success or discarding results.
    if let Some(permit) = lock(&shared.permit).take() {
        let watch = if native.lease.as_ref().is_some_and(|lease| lease.has_process()) {
            match native
                .lease
                .as_mut()
                .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?
                .bind_watch(permit)
            {
                Ok(watch) => watch,
                Err(failure) => {
                    if first.is_none() {
                        first = Some(failure.cause.clone());
                    }
                    native.retain(failure.cause);
                    failure.permit.cancel_unbound()
                }
            }
        } else {
            permit.cancel_unbound()
        };
        native.watch = Some(watch);
    } else if first.is_none() {
        first = Some(CrashError::invalid(CaptureOperation::Observation));
    }
    let registered = native.lease.as_ref().is_some_and(|lease| lease.registered());
    // The native counter is only a polling bound. Healthy publication also
    // requires the original effective execution-clock deadline to remain live.
    if (context.cancellation_requested()
        || context.deadline().is_none_or(|deadline| context.now() >= deadline))
        && first.is_none()
    {
        first = Some(CrashError::invalid(CaptureOperation::Registration));
    }
    native.registered =
        first.is_none() && registered && native.handler.is_some() && native.watch.is_some();
    drain_errors(&mut native);
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
    let mut native = lock(&shared.native);
    let deadline =
        context.deadline().ok_or_else(|| CrashError::invalid(CaptureOperation::Retirement))?;
    if native.close_deadline.is_some_and(|old| deadline > old) {
        return Err(CrashError::invalid(CaptureOperation::Retirement));
    }
    native.close_deadline = Some(native.close_deadline.map_or(deadline, |old| old.min(deadline)));
    crate::native::check_deadline(&context)?;
    let Some(lease) = native.lease.as_mut() else {
        native.retired = true;
        return Ok(());
    };
    let stop = lease.request_stop();
    let retire = lease.retire(&context);
    native.retired = matches!(retire, Ok(true));
    let mut first = None;
    for result in [stop, retire.map(|_| ())] {
        if let Err(cause) = result {
            if first.is_none() {
                first = Some(cause.clone());
            }
            native.retain(cause);
        }
    }
    drain_errors(&mut native);
    first.map_or(Ok(()), Err)
}

fn drain_errors(native: &mut NativeState) {
    loop {
        let cause = native.lease.as_mut().and_then(|lease| lease.take_error());
        let Some(cause) = cause else {
            break;
        };
        native.retain(cause);
    }
    let omitted = native.lease.as_mut().map_or(0, |lease| lease.take_omitted_errors());
    native.omitted = native.omitted.saturating_add(omitted);
}
