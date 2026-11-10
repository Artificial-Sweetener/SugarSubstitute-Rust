// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise queue, lifecycle and infrastructure admission through production workers.

use crate::runtime_test_support::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;

struct HoldingCpu {
    started: Arc<Gate>,
    release: Arc<Gate>,
}
impl CpuTask<(), String> for HoldingCpu {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, String> {
        self.started.open();
        self.release.wait().map_err(|error| error.to_string())?;
        Ok(CpuStep::Complete(()))
    }
}
struct ControlledFactoryDrop {
    started: Arc<Gate>,
    release: Arc<Gate>,
    thread: Arc<std::sync::Mutex<Option<std::thread::ThreadId>>>,
}
impl Drop for ControlledFactoryDrop {
    fn drop(&mut self) {
        *crate::scope::lock(&self.thread) = Some(std::thread::current().id());
        self.started.open();
        if let Err(error) = self.release.wait() {
            std::panic::resume_unwind(Box::new(error.to_string()));
        }
    }
}

#[test]
fn coalesced_factory_cleanup_is_owned_off_the_submitting_thread() -> TestResult {
    let rig = Rig::new()?;
    let started = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let task_started = started.clone();
    let task_release = release.clone();
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 100)?, move |_| {
            HoldingCpu { started: task_started, release: task_release }
        })?;
    started.wait()?;
    let drop_started = Arc::new(Gate::default());
    let drop_release = Arc::new(Gate::default());
    let drop_thread = Arc::new(std::sync::Mutex::new(None));
    let cleanup = ControlledFactoryDrop {
        started: drop_started.clone(),
        release: drop_release.clone(),
        thread: drop_thread.clone(),
    };
    let factory_called = Arc::new(AtomicBool::new(false));
    let called = factory_called.clone();
    let mut pending =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 100)?, move |_| {
            let _captured = cleanup;
            called.store(true, Ordering::Release);
            HoldingCpu { started: Arc::new(Gate::default()), release: Arc::new(Gate::default()) }
        })?;
    let next_release = Arc::new(Gate::default());
    next_release.open();
    let mut replacement =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 100)?, move |_| {
            HoldingCpu { started: Arc::new(Gate::default()), release: next_release }
        })?;
    // Submission returned while the sole interactive worker was still in its
    // first step. Retired factory destruction remains owned and unobserved.
    assert!(pending.try_take_completion().is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, 300);
    release.open();
    drop_started.wait()?;
    assert_ne!(*crate::scope::lock(&drop_thread), Some(std::thread::current().id()));
    assert!(pending.try_take_completion().is_none());
    assert!(!factory_called.load(Ordering::Acquire));
    drop_release.open();
    assert!(
        matches!(rig.completion(&mut pending)?.outcome, JobOutcome::Coalesced { replacement } if replacement == JobId::new(3)?)
    );
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut replacement)?.outcome, JobOutcome::Success(())));
    Ok(())
}

fn controlled_io(
    rig: &Rig,
    id: u64,
    class: JobClass,
    release: Arc<Gate>,
    started: Arc<Gate>,
) -> TestResult<OwnedJob<(), ()>> {
    Ok(rig.client.submit_async(&rig.scope, spec(id, class, 0)?, move |_| async move {
        started.open();
        release.future().await;
        Ok(())
    })?)
}

#[test]
fn ordinary_io_and_queue_cannot_consume_reserved_lifecycle_slots() -> TestResult {
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let mut jobs = Vec::new();
    for id in 1..=8 {
        let started = Arc::new(Gate::default());
        jobs.push(controlled_io(&rig, id, JobClass::IoOrdinary, release.clone(), started.clone())?);
        started.wait()?;
    }
    let ordinary_queued = Arc::new(AtomicBool::new(false));
    for id in 9..=36 {
        let invoked = ordinary_queued.clone();
        jobs.push(rig.client.submit_async(
            &rig.scope,
            spec(id, JobClass::IoOrdinary, 0)?,
            move |_| {
                invoked.store(true, Ordering::Release);
                async { Ok(()) }
            },
        )?);
    }
    assert_eq!(rig.scope.snapshot().running, 8);
    assert_eq!(rig.scope.snapshot().queued, 28);
    assert!(!ordinary_queued.load(Ordering::Acquire));
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(37, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::QueueSaturated { .. })
    ));
    for id in 37..=40 {
        let started = Arc::new(Gate::default());
        jobs.push(controlled_io(
            &rig,
            id,
            JobClass::IoLifecycle,
            release.clone(),
            started.clone(),
        )?);
        started.wait()?;
    }
    assert_eq!(rig.scope.snapshot().running, 12);
    for id in 41..=44 {
        jobs.push(controlled_io(
            &rig,
            id,
            JobClass::IoLifecycle,
            release.clone(),
            Arc::new(Gate::default()),
        )?);
    }
    assert_eq!(rig.scope.snapshot().queued, 32);
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(45, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::QueueSaturated { .. })
    ));
    release.open();
    for job in &mut jobs {
        assert!(matches!(rig.completion(job)?.outcome, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn lifecycle_io_can_use_idle_ordinary_capacity_beyond_four() -> TestResult {
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let mut jobs = Vec::new();
    for id in 1..=12 {
        let started = Arc::new(Gate::default());
        jobs.push(controlled_io(
            &rig,
            id,
            JobClass::IoLifecycle,
            release.clone(),
            started.clone(),
        )?);
        started.wait()?;
    }
    assert_eq!(rig.scope.snapshot().running, 12);
    release.open();
    for job in &mut jobs {
        assert!(matches!(rig.completion(job)?.outcome, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn blocking_ordinary_admission_cannot_bypass_lifecycle_worker_reserve() -> TestResult {
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let first_started = Arc::new(Gate::default());
    let started = first_started.clone();
    let wait = release.clone();
    let mut first = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 0)?,
        move |_| {
            started.open();
            wait.wait().map_err(|error| error.to_string())
        },
    )?;
    first_started.wait()?;
    let ordinary_invoked = Arc::new(AtomicBool::new(false));
    let invoked = ordinary_invoked.clone();
    let mut second = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 0)?,
        move |_| {
            invoked.store(true, Ordering::Release);
            Ok::<_, String>(())
        },
    )?;
    let lifecycle_started = Arc::new(Gate::default());
    let started = lifecycle_started.clone();
    let wait = release.clone();
    let mut lifecycle = rig.client.submit_blocking(
        &rig.scope,
        spec(3, JobClass::InfrastructureLifecycle, 0)?,
        move |_| {
            started.open();
            wait.wait().map_err(|error| error.to_string())
        },
    )?;
    lifecycle_started.wait()?;
    assert!(!ordinary_invoked.load(Ordering::Acquire));
    assert_eq!(second.status().phase, JobPhase::Queued);
    assert_eq!(rig.scope.snapshot().running, 2);
    release.open();
    assert!(matches!(rig.completion(&mut first)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut lifecycle)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut second)?.outcome, JobOutcome::Success(())));
    assert!(ordinary_invoked.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn completion_flood_stays_bounded_and_metadata_drain_does_not_ack_results() -> TestResult {
    let rig = Rig::new()?;
    let mut jobs = Vec::new();
    for id in 1..=48 {
        let class = if id <= 44 { JobClass::IoOrdinary } else { JobClass::IoLifecycle };
        jobs.push(
            rig.client
                .submit_async(&rig.scope, spec(id, class, 0)?, |_| async { Ok::<_, ()>(()) })?,
        );
        loop {
            let stamp = rig.wake.count();
            let batch = rig.client.drain_completions(NonZeroUsize::MIN)?;
            if !batch.notifications.is_empty() {
                break;
            }
            rig.wake.wait_after(stamp)?;
        }
    }
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 48);
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(49, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::CompletionSaturated)
    ));
    let first = jobs[0]
        .try_take_completion()
        .ok_or_else(|| std::io::Error::other("Terminal completion missing"))?;
    drop(first);
    let mut accepted =
        rig.client.submit_async(&rig.scope, spec(49, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    assert!(matches!(rig.completion(&mut accepted)?.outcome, JobOutcome::Success(())));
    Ok(())
}

#[test]
fn abandoned_scopes_retire_only_after_children_and_retained_bytes() -> TestResult {
    let rig = Rig::new()?;
    let mut scopes = Vec::new();
    for id in 2..=64 {
        scopes.push(
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(id)?, deadline: None }, None)?,
        );
    }
    assert!(matches!(
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(65)?, deadline: None }, None),
        Err(AdmissionError::ScopeCapacity)
    ));
    drop(scopes.pop());
    notification_barrier(&rig)?;
    let parent =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(65)?, deadline: None }, None)?;
    drop(scopes.pop());
    notification_barrier(&rig)?;
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(66)?, deadline: None }, Some(&parent))?;
    drop(parent);
    assert!(matches!(
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(67)?, deadline: None }, None),
        Err(AdmissionError::ScopeCapacity)
    ));
    drop(child);
    notification_barrier(&rig)?;
    let accepted =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(67)?, deadline: None }, None)?;
    assert_eq!(accepted.id(), ScopeId::new(67)?);
    assert!(
        matches!(rig.client.create_scope(ScopeSpec { id: ScopeId::new(65)?, deadline: None }, None), Err(AdmissionError::ScopeIdentityOrder { latest }) if latest == ScopeId::new(67)?)
    );
    Ok(())
}

fn notification_barrier(rig: &Rig) -> TestResult {
    let core = &rig.client.core_for_test();
    drop(
        core.cpu_changed
            .wait_while(crate::scope::lock(&core.state), |state| {
                state.notifications_inflight > 0
                    || state.scopes.values().any(|scope| scope.cancel.pending())
            })
            .map_err(|error| std::io::Error::other(error.to_string()))?,
    );
    Ok(())
}

#[path = "admission_replacement_tests.rs"]
mod replacements;
