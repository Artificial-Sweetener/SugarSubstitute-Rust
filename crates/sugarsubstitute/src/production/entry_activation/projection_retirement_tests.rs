// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual failure/control allocations charged after passive source abandonment.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    generation: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut generation =
            self.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[test]
fn unused_projection_source_return_keeps_first_cause_controls_and_original_charge_after_ack()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    // No authenticated loan/native authority is fabricated in this failure fixture.
    let mut input = Some(ProjectionInput {
        request: None,
        snapshot: None,
        capture_root: PathBuf::new(),
        cause: ProjectionCause::NotStarted,
        post: None,
        _charge: None,
    });
    let mut job = client.submit_blocking_owned_box(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: PROJECTION_BYTES,
            replacement: None,
        },
        &mut input,
        |mut retained, context| {
            retained.input_mut()._charge = match context.memory().claim(PROJECTION_DATA_BYTES) {
                Ok(claim) => Some(claim),
                Err(cause) => {
                    retained.input_mut().cause = ProjectionCause::Claim(cause);
                    return Err::<ProjectedEntry, _>(ProjectionFailure { retained });
                }
            };
            retained.input_mut().cause = ProjectionCause::Operation(Box::new(
                std::io::Error::other("original projection boundary cause"),
            ));
            retained.input_mut().post = Some(ControlFailure::Expired);
            Err::<ProjectedEntry, _>(ProjectionFailure { retained })
        },
    )?;
    let mut completed = loop {
        let observed = *wake.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let generation = wake.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (generation, timeout) = wake
            .changed
            .wait_timeout_while(generation, Duration::from_secs(10), |generation| {
                *generation == observed
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(generation);
        if timeout.timed_out() {
            return Err("Actual projection completion did not arrive".into());
        }
    };
    drop(job);
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    for _ in 0..2 {
        return_unused_source(&mut completed.outcome);
        let JobOutcome::Failure(failure) = &completed.outcome else {
            return Err("Original failure missing".into());
        };
        assert_eq!(
            failure.source().ok_or("First cause absent")?.to_string(),
            "original projection boundary cause"
        );
        assert!(matches!(failure.retained.input().post, Some(ControlFailure::Expired)));
        assert!(failure.retained.input()._charge.is_some());
        assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    }
    drop(completed);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
