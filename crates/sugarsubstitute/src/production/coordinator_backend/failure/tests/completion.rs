// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Obtain a real typed failure through the same central execution boundary.
use super::*;
use crate::completion_wake::Wake;
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::execution::ScopeId;
use sugarsubstitute_execution::ExecutionOwner;

pub(super) fn native_failure() -> Result<JobCompletion<WindowsOutputReport, NativeObservationError>>
{
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(
        Arc::new(super::super::super::tests::TestClock::default()),
        wake.clone(),
    )?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: sugarsubstitute_contracts::execution::JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: 4096,
            replacement: None,
        },
        |_| {
            Err::<WindowsOutputReport, _>(NativeObservationError::new(
                "output_read",
                std::io::Error::from_raw_os_error(5),
            ))
        },
    )?;
    loop {
        let observed = wake.revision();
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        wake.wait_after(observed);
    }
}
