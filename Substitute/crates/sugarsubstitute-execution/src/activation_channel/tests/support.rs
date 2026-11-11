// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit native channel fixtures through the existing execution rig and deterministic clock.
use super::*;
use crate::runtime_test_support::{ManualClock, Rig, spec};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, Moment, Scope};
type WorkerResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Run native work inside actual finite blocking admission, then consume its terminal result.
pub(super) fn run_reserved(
    bytes: u64,
    body: impl FnOnce(&JobContext, &ManualClock) -> WorkerResult + Send + 'static,
) -> TestResult {
    let rig = Rig::new()?;
    let clock = rig.clock.clone();
    run(&rig, &rig.scope, 1, bytes, move |context| body(&context, &clock))
}

/// Retain the same ledger and original scope across a native cancellation/cleanup transition.
pub(super) fn phase<T: Send + 'static>(
    rig: &Rig,
    scope: &Scope,
    id: u64,
    body: impl FnOnce(JobContext) -> WorkerResult<T> + Send + 'static,
) -> TestResult<T> {
    run(rig, scope, id, CHANNEL_RESERVATION_BYTES * 3, body)
}

/// Keep native operations on the production executor and preserve their actual terminal cause.
fn run<T: Send + 'static>(
    rig: &Rig,
    scope: &Scope,
    id: u64,
    bytes: u64,
    body: impl FnOnce(JobContext) -> WorkerResult<T> + Send + 'static,
) -> TestResult<T> {
    let mut specification = spec(id, JobClass::InfrastructureLifecycle, bytes)?;
    specification.deadline = Some(Moment::from_nanos(2_000_000_000));
    let mut job = rig.client.submit_blocking(scope, specification, body)?;
    match rig.completion(&mut job)?.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Unexpected channel fixture outcome: {:?}", other.kind()).into()),
    }
}
