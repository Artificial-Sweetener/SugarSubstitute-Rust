// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Obtain constructor storage only through an actual admitted execution result.
use super::{Result, TestClock};
use crate::completion_wake::Wake;
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;

/// The production adapter admits and delivers this claim; no lease is fabricated.
pub(crate) fn claim(bytes: u64) -> Result<MemoryClaim> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(TestClock::default()), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        },
        move |context| context.memory().claim(bytes),
    )?;
    loop {
        let observed = wake.revision();
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            return successful(completion);
        }
        wake.wait_after(observed);
    }
}
fn successful(completion: JobCompletion<MemoryClaim, MemoryClaimError>) -> Result<MemoryClaim> {
    match completion.outcome {
        JobOutcome::Success(claim) => Ok(claim),
        JobOutcome::Failure(cause) => Err(cause.into()),
        _ => Err("Actual backend storage claim did not complete".into()),
    }
}
