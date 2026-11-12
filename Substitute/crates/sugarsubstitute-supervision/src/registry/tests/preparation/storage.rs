// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual storage progress without changing registry clocks or completion consumption.
use super::*;
use crate::setup_storage::test_worker::{StorageTrace, observation::Observation};

/// Run a native fixture phase on the registry's original execution and admission ledger.
#[track_caller]
pub(super) fn run<T: Send + 'static>(
    h: &mut Harness,
    bytes: u64,
    body: impl FnOnce(JobContext) -> Result<T, Failure> + Send + 'static,
) -> Result<T, Box<dyn Error>> {
    let source = std::panic::Location::caller();
    let phase = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let worker_phase = phase.clone();
    let observation = Observation::new(h.wake.clone());
    let worker_observation = observation.clone();
    let trace = StorageTrace::new(observation.clone());
    let worker_trace = trace.clone();
    let mut spec = h.spec(JobClass::InfrastructureLifecycle, bytes)?;
    spec.deadline = Some(Moment::from_nanos(2_000_000_000));
    let mut job = h.client.submit_blocking(&h.scope, spec, move |context| {
        worker_phase.store(1, Ordering::Release);
        worker_observation.started();
        let result = tracing::subscriber::with_default(worker_trace, || body(context));
        worker_phase.store(2, Ordering::Release);
        result
    })?;
    let completion = loop {
        let revision = *lock(&h.wake.count);
        h.registry.consume(
            &h.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?,
        )?;
        if let Some(completion) = job.try_take_completion() {
            observation.completed()?;
            break completion;
        }
        observation.check().map_err(|cause| {
            format!(
                "Native storage phase {source}: state={}; {cause}; {}",
                phase.load(Ordering::Acquire),
                trace.snapshot()
            )
        })?;
        drop(
            h.wake
                .changed
                .wait_while(lock(&h.wake.count), |current| *current == revision)
                .map_err(|_| "Native storage observer poisoned")?,
        );
    };
    match completion.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Storage job stopped: {:?}", other.kind()).into()),
    }
}
