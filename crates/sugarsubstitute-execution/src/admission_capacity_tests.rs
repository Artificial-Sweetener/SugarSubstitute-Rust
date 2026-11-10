// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise native retry activity through real admission, terminal transfer and retained contexts.

use crate::{runtime_test_support::*, scope::lock};
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::execution::*;

const LIMIT: u64 = 256 * 1024 * 1024;

#[test]
fn startup_memory_capacity_controls_retained_admission_and_retry_wakes() -> TestResult {
    assert_eq!(Rig::new()?.owner.limits().memory_bytes, LIMIT);
    for limit in [1024, 512 * 1024 * 1024] {
        let clock = Arc::new(ManualClock::default());
        let wake = Arc::new(Wake::default());
        let owner = crate::ExecutionOwner::with_memory_capacity(
            Arc::new(ClockPort(clock.clone())),
            wake.clone(),
            std::num::NonZeroU64::new(limit).ok_or("Zero capacity")?,
        )?;
        assert_eq!(owner.limits().memory_bytes, limit);
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let rig = Rig { owner, client, clock, wake, scope };
        let retained = retained_context(&rig, 1, limit - 16)?;
        let small = retained_context(&rig, 2, 16)?;
        drain(&rig)?;
        assert_eq!(rig.scope.snapshot().reserved_bytes, limit);
        assert!(matches!(
            rig.client.submit_blocking(
                &rig.scope,
                spec(3, JobClass::InfrastructureLifecycle, 17)?,
                |_| Ok::<_, ()>(()),
            ),
            Err(AdmissionError::MemorySaturated { requested_bytes: 17, available_bytes: 0 })
        ));
        let before = rig.wake.count();
        drop(small);
        wait_bytes(&rig, limit - 16)?;
        assert_eq!(rig.wake.count(), before);
        drop(retained);
        rig.wake.wait_after(before)?;
        drain(&rig)?;
        let whole = retained_context(&rig, 3, limit)?;
        drop(whole);
        wait_bytes(&rig, 0)?;
    }
    Ok(())
}

fn retained_context(rig: &Rig, id: u64, bytes: u64) -> TestResult<JobContext> {
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(id, JobClass::InfrastructureLifecycle, bytes)?,
        Ok::<_, ()>,
    )?;
    match rig.completion(&mut job)?.outcome {
        JobOutcome::Success(context) => Ok(context),
        _ => Err(std::io::Error::other("Context producer did not succeed").into()),
    }
}

fn drain(rig: &Rig) -> TestResult {
    let batch = rig.client.drain_completions(NonZeroUsize::new(32).ok_or("Zero drain")?)?;
    assert!(batch.notifications.is_empty());
    assert_eq!(batch.remaining, 0);
    Ok(())
}

fn wait_bytes(rig: &Rig, expected: u64) -> TestResult {
    let core = rig.client.core_for_test();
    drop(
        core.cpu_changed
            .wait_while(lock(&core.state), |state| state.bytes != expected)
            .map_err(|error| std::io::Error::other(error.to_string()))?,
    );
    Ok(())
}

#[test]
fn final_original_context_release_wakes_native_retry_after_completion_transfer() -> TestResult {
    let rig = Rig::new()?;
    let original = retained_context(&rig, 1, LIMIT)?;
    drain(&rig)?;
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, LIMIT);
    assert!(matches!(
        rig.client.submit_blocking(
            &rig.scope,
            spec(2, JobClass::InfrastructureLifecycle, 16)?,
            |_| Ok::<_, ()>(()),
        ),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    let before = rig.wake.count();
    drop(original);
    rig.wake.wait_after(before)?;
    drain(&rig)?;
    let next = retained_context(&rig, 2, 16)?;
    drop(next);
    wait_bytes(&rig, 0)?;
    Ok(())
}

#[test]
fn cancelled_scope_final_memory_release_wakes_its_composing_pump() -> TestResult {
    let rig = Rig::new()?;
    let first = retained_context(&rig, 1, 32)?;
    let second = retained_context(&rig, 2, 16)?;
    drain(&rig)?;
    rig.scope.cancel();
    let before = rig.wake.count();
    drop(first);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 16);
    assert_eq!(rig.wake.count(), before);
    drop(second);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(rig.wake.count(), before + 1);
    drain(&rig)?;
    Ok(())
}

#[test]
fn cancelled_scope_release_coalesces_with_an_unconsumed_foreign_terminal() -> TestResult {
    let rig = Rig::new()?;
    let other =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let retained = retained_context(&rig, 1, 32)?;
    drain(&rig)?;
    rig.scope.cancel();
    let before = rig.wake.count();
    let mut job =
        rig.client.submit_async(&other, spec(2, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    rig.wake.wait_after(before)?;
    let pending_wake = rig.wake.count();
    drop(retained);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(rig.wake.count(), pending_wake);
    let batch = rig.client.drain_completions(NonZeroUsize::MIN)?;
    assert_eq!(batch.notifications.len(), 1);
    assert_eq!(batch.notifications[0].scope_id, other.id());
    drop(job.try_take_completion().ok_or("Foreign terminal missing")?);
    Ok(())
}

#[test]
fn insufficient_or_unrequested_memory_releases_do_not_wake() -> TestResult {
    let rig = Rig::new()?;
    let large = retained_context(&rig, 1, LIMIT - 8)?;
    let small = retained_context(&rig, 2, 8)?;
    drain(&rig)?;
    for bytes in [32, 16, 32] {
        assert!(matches!(
            rig.client.submit_async(&rig.scope, spec(3, JobClass::IoLifecycle, bytes)?, |_| {
                async { Ok::<_, ()>(()) }
            }),
            Err(AdmissionError::MemorySaturated { .. })
        ));
    }
    let before = rig.wake.count();
    drop(small);
    wait_bytes(&rig, LIMIT - 8)?;
    assert_eq!(rig.wake.count(), before);
    drop(large);
    rig.wake.wait_after(before)?;
    wait_bytes(&rig, 0)?;
    assert_eq!(rig.wake.count(), before + 1);
    drain(&rig)?;
    let unrequested = retained_context(&rig, 3, 64)?;
    drain(&rig)?;
    let before = rig.wake.count();
    drop(unrequested);
    wait_bytes(&rig, 0)?;
    assert_eq!(rig.wake.count(), before);
    Ok(())
}

#[test]
fn impossible_request_and_post_shutdown_guards_do_not_wake() -> TestResult {
    let mut rig = Rig::new()?;
    let original = retained_context(&rig, 1, LIMIT)?;
    drain(&rig)?;
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoLifecycle, LIMIT + 1)?, |_| {
            async { Ok::<_, ()>(()) }
        }),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    let before = rig.wake.count();
    drop(original);
    wait_bytes(&rig, 0)?;
    assert_eq!(rig.wake.count(), before);
    let original = retained_context(&rig, 2, LIMIT)?;
    drain(&rig)?;
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(3, JobClass::IoLifecycle, 1)?, |_| {
            async { Ok::<_, ()>(()) }
        }),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    let before = rig.wake.count();
    rig.owner.request_shutdown(Moment::from_nanos(0));
    drop(rig.owner);
    // Worker return is the only shutdown wake; released capacity afterwards stays silent.
    assert_eq!(rig.wake.count(), before + 1);
    rig.wake.fail.store(true, std::sync::atomic::Ordering::Release);
    drop(original);
    assert_eq!(rig.wake.count(), before + 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert!(rig.client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    Ok(())
}

#[test]
fn capacity_wake_failure_is_observed_without_terminal_fabrication() -> TestResult {
    let rig = Rig::new()?;
    let original = retained_context(&rig, 1, LIMIT)?;
    drain(&rig)?;
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoLifecycle, 1)?, |_| {
            async { Ok::<_, ()>(()) }
        }),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    rig.wake.fail.store(true, std::sync::atomic::Ordering::Release);
    drop(original);
    wait_bytes(&rig, 0)?;
    drop(rig.owner);
    let error =
        rig.client.drain_completions(NonZeroUsize::MIN).err().ok_or("Wake error missing")?;
    assert_eq!(error.operation(), Operation::NativeLifecycle);
    assert!(error.to_string().contains("Owned wake fixture failed"));
    assert!(rig.client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    rig.wake.fail.store(false, std::sync::atomic::Ordering::Release);
    Ok(())
}

struct FactoryDrop {
    started: Arc<Gate>,
    release: Arc<Gate>,
}
impl Drop for FactoryDrop {
    fn drop(&mut self) {
        self.started.open();
        if let Err(error) = self.release.wait() {
            std::panic::resume_unwind(Box::new(error.to_string()));
        }
    }
}

#[test]
fn ready_queue_release_wakes_before_cancelled_factory_cleanup_finishes() -> TestResult {
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let mut jobs = Vec::new();
    for id in 1..=8 {
        let started = Arc::new(Gate::default());
        let began = started.clone();
        let finish = release.clone();
        jobs.push(rig.client.submit_async(
            &rig.scope,
            spec(id, JobClass::IoOrdinary, 0)?,
            move |_| async move {
                began.open();
                finish.future().await;
                Ok::<_, ()>(())
            },
        )?);
        started.wait()?;
    }
    let drop_started = Arc::new(Gate::default());
    let drop_release = Arc::new(Gate::default());
    for id in 9..=36 {
        let capture = (id == 9)
            .then(|| FactoryDrop { started: drop_started.clone(), release: drop_release.clone() });
        jobs.push(rig.client.submit_async(
            &rig.scope,
            spec(id, JobClass::IoOrdinary, 0)?,
            move |_| {
                drop(capture);
                async { Ok::<_, ()>(()) }
            },
        )?);
    }
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(37, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::QueueSaturated { .. })
    ));
    let before = rig.wake.count();
    jobs[8].cancel();
    drop_started.wait()?;
    assert_eq!(rig.wake.count(), before + 1);
    drain(&rig)?;
    assert!(jobs[8].try_take_completion().is_none());
    jobs.push(rig.client.submit_async(
        &rig.scope,
        spec(37, JobClass::IoOrdinary, 0)?,
        |_| async { Ok::<_, ()>(()) },
    )?);
    drop_release.open();
    release.open();
    for job in &mut jobs {
        let _completion = rig.completion(job)?;
    }
    Ok(())
}

#[path = "admission_capacity_credit_tests.rs"]
mod credits;

#[path = "admission_capacity_race_tests.rs"]
mod races;
