// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual ordinary output when the central clock closes all admission.
use super::release::{generation, ordinary};
use super::*;
use crate::coordinator_entry::fixture::ClockPort;
use std::{sync::Condvar, time::Duration};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CompletionWake, ExecutionError, JobClass, JobId, JobSpec, Operation, ScopeId,
    ScopeSpec,
};
use sugarsubstitute_execution::ExecutionOwner;
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[test]
fn fixed_record_fatal_original_clock_retains_actual_stage_and_charge() -> TestResult {
    let clock = Arc::new(ClockPort::default());
    clock.0.store(17, std::sync::atomic::Ordering::Release);
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut old = generation(resolved().namespace, 1)?;
    old.state = crate::native_claims::RecordState::Released;
    let fake = Fake::new(RecordRead::Present(old.encode()));
    lock(&fake.0).expiry = Some((Effect::Create, clock.clone(), Moment::from_nanos(16)));
    let physical = fake.clone();
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let original_context = Arc::new(Mutex::new(None));
    let owned_context = original_context.clone();
    let desired = generation(resolved().namespace, 2)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            class: JobClass::InfrastructureLifecycle,
            operation: Operation::NativeLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 4096,
            replacement: None,
        },
        move |context| -> Result<(), ClaimError> {
            let mut fake = fake;
            adopt(&output, &mut fake, &context)?;
            *lock(&owned_context) = Some(context.clone());
            let slot = lock(&output);
            ordinary(
                slot.as_ref().ok_or_else(|| ClaimError::invalid("fatal_owner_missing"))?,
                &desired,
                &mut fake,
                &context,
            )
        },
    )?;
    let done = loop {
        let old = *lock(&wake.count);
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        let (_guard, timeout) = wake
            .changed
            .wait_timeout_while(lock(&wake.count), Duration::from_secs(10), |count| *count == old)
            .map_err(|_| "fatal clock wake poisoned")?;
        if timeout.timed_out() {
            return Err("fatal clock terminal outcome missing".into());
        }
    };
    assert!(matches!(done.outcome, JobOutcome::Panicked(_)), "{done:?}");
    let cause = client.now().err().ok_or("fatal clock produced a timestamp")?;
    assert_eq!(cause.operation(), Operation::Infrastructure);
    assert!(
        cause
            .source()
            .is_some_and(|source| source.to_string().contains("Monotonic interval regressed"))
    );
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::Acquire), 16);
    assert_eq!(lock(&physical.0).effects, [Effect::Create]);
    assert_eq!(physical.live(), 2);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    let original = lock(&original_context).take().ok_or("original context lost")?;
    physical.externally_retire(&original)?;
    assert_eq!(physical.live(), 0);
    {
        let slot = lock(&retained);
        let file = slot.as_ref().ok_or("fatal owner lost")?;
        assert!(lock(&file.stage).is_some());
        assert!(lock(&file.publication).is_some());
    }
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(lock(&retained).take());
    drop((original, done, job));
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
