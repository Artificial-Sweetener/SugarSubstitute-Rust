// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise CPU scheduling and actual destruction through the production submission port.

use crate::{runtime_test_support::*, scope::lock};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

type Step = Box<dyn FnMut(&JobContext) -> Result<CpuStep<usize>, String> + Send>;

/// Supply controlled external boundaries without replacing the scheduler under test.
struct Task {
    step: Step,
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}
impl CpuTask<usize, String> for Task {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<usize>, String> {
        (self.step)(context)
    }
}
impl Drop for Task {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            on_drop();
        }
    }
}
fn task(step: impl FnMut(&JobContext) -> Result<CpuStep<usize>, String> + Send + 'static) -> Task {
    Task { step: Box::new(step), on_drop: None }
}
fn success() -> Task {
    task(|_| Ok(CpuStep::Complete(7)))
}

#[test]
fn yields_retain_one_factory_task_and_cpu_thread() -> TestResult {
    let rig = Rig::new()?;
    let caller = std::thread::current().id();
    let factories = Arc::new(AtomicUsize::new(0));
    let threads = Arc::new(Mutex::new(Vec::new()));
    let constructed = factories.clone();
    let observed = threads.clone();
    let mut job =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 32)?, move |_| {
            constructed.fetch_add(1, Ordering::SeqCst);
            lock(&observed).push(std::thread::current().id());
            let steps = observed.clone();
            let mut remaining = 8;
            let mut work = task(move |_| {
                lock(&steps).push(std::thread::current().id());
                remaining -= 1;
                Ok(if remaining == 0 { CpuStep::Complete(8) } else { CpuStep::Yield })
            });
            work.on_drop =
                Some(Box::new(move || lock(&observed).push(std::thread::current().id())));
            work
        })?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(8)));
    assert_eq!(factories.load(Ordering::SeqCst), 1);
    let threads = lock(&threads);
    assert_eq!(threads.len(), 10);
    assert!(threads.iter().all(|id| *id == threads[0] && *id != caller));
    Ok(())
}

#[test]
fn cancelled_pending_never_replaces_the_started_task() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let work_entered = entered.clone();
    let work_release = release.clone();
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            let mut steps = 0;
            task(move |_| {
                steps += 1;
                if steps == 1 {
                    work_entered.open();
                    work_release.wait().map_err(|e| e.to_string())?;
                }
                Ok(if steps == 1 { CpuStep::Yield } else { CpuStep::Complete(steps) })
            })
        })?;
    entered.wait()?;
    let called = Arc::new(AtomicBool::new(false));
    let factory_called = called.clone();
    let mut pending =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 0)?, move |_| {
            factory_called.store(true, Ordering::SeqCst);
            success()
        })?;
    pending.cancel();
    release.open();
    assert!(matches!(rig.completion(&mut pending)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(2)));
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn latest_pending_surface_revision_coalesces_without_cancelling_active() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let work_entered = entered.clone();
    let work_release = release.clone();
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            task(move |_| {
                work_entered.open();
                work_release.wait().map_err(|e| e.to_string())?;
                Ok(CpuStep::Complete(1))
            })
        })?;
    entered.wait()?;
    let called = Arc::new(AtomicBool::new(false));
    let factory_called = called.clone();
    let mut replaced =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 0)?, move |_| {
            factory_called.store(true, Ordering::SeqCst);
            success()
        })?;
    let mut newest =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 0)?, |_| success())?;
    assert!(replaced.try_take_completion().is_none());
    assert_eq!(active.status().phase, JobPhase::Running);
    assert!(!active.status().cancellation_requested);
    assert!(!called.load(Ordering::SeqCst));
    release.open();
    assert!(
        matches!(rig.completion(&mut replaced)?.outcome, JobOutcome::Coalesced { replacement } if replacement == JobId::new(3)?)
    );
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(1)));
    assert!(matches!(rig.completion(&mut newest)?.outcome, JobOutcome::Success(7)));
    Ok(())
}

/// Observe release of a queued factory without executing its application factory.
struct RecordedDrop {
    id: usize,
    order: Arc<Mutex<Vec<usize>>>,
}
impl Drop for RecordedDrop {
    fn drop(&mut self) {
        lock(&self.order).push(self.id);
    }
}

#[test]
fn retirement_and_cancellation_share_four_cleanup_items_before_active_step() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let order = Arc::new(Mutex::new(Vec::new()));
    let (worker_entered, worker_release, worker_order) =
        (entered.clone(), release.clone(), order.clone());
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            let mut first = true;
            task(move |_| {
                if first {
                    first = false;
                    worker_entered.open();
                    worker_release.wait().map_err(|error| error.to_string())?;
                    Ok(CpuStep::Yield)
                } else {
                    lock(&worker_order).push(0);
                    Ok(CpuStep::Complete(1))
                }
            })
        })?;
    entered.wait()?;
    let mut pending = Vec::new();
    for id in 2..=7 {
        let captured = RecordedDrop { id: id as usize, order: order.clone() };
        pending.push(rig.client.submit_cpu(
            &rig.scope,
            spec(id, JobClass::CpuInteractive, 0)?,
            move |_| {
                drop(captured);
                success()
            },
        )?);
    }
    let captured = RecordedDrop { id: 8, order: order.clone() };
    let mut cancelled =
        rig.client.submit_cpu(&rig.scope, spec(8, JobClass::CpuDecode, 0)?, move |_| {
            drop(captured);
            success()
        })?;
    cancelled.cancel();
    release.open();
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(1)));
    for job in &mut pending {
        let completion = rig.completion(job)?;
        if completion.spec.id.get() == 7 {
            assert!(matches!(completion.outcome, JobOutcome::Success(7)));
        } else {
            assert!(
                matches!(completion.outcome, JobOutcome::Coalesced { replacement } if replacement.get() == completion.spec.id.get() + 1)
            );
        }
    }
    assert!(matches!(rig.completion(&mut cancelled)?.outcome, JobOutcome::Cancelled));
    assert_eq!(*lock(&order), [2, 8, 3, 4, 0, 5, 6, 7]);
    Ok(())
}

#[test]
fn decode_capacity_one_preserves_interactive_worker() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let work_entered = entered.clone();
    let work_release = release.clone();
    let mut first =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuDecode, 0)?, move |_| {
            task(move |_| {
                work_entered.open();
                work_release.wait().map_err(|e| e.to_string())?;
                Ok(CpuStep::Complete(1))
            })
        })?;
    entered.wait()?;
    let called = Arc::new(AtomicBool::new(false));
    let factory_called = called.clone();
    let mut second =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuDecode, 0)?, move |_| {
            factory_called.store(true, Ordering::SeqCst);
            success()
        })?;
    let mut interactive =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 0)?, |_| success())?;
    assert!(matches!(rig.completion(&mut interactive)?.outcome, JobOutcome::Success(7)));
    assert_eq!(second.status().phase, JobPhase::Queued);
    assert!(!called.load(Ordering::SeqCst));
    release.open();
    assert!(matches!(rig.completion(&mut first)?.outcome, JobOutcome::Success(1)));
    assert!(matches!(rig.completion(&mut second)?.outcome, JobOutcome::Success(7)));
    Ok(())
}

#[test]
fn single_worker_prioritizes_interactive_and_yields_to_decode() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let work_entered = entered.clone();
    let work_release = release.clone();
    let mut barrier =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            task(move |_| {
                work_entered.open();
                work_release.wait().map_err(|e| e.to_string())?;
                Ok(CpuStep::Complete(0))
            })
        })?;
    entered.wait()?;
    let order = Arc::new(Mutex::new(Vec::new()));
    let decode_order = order.clone();
    let mut decode =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuDecode, 0)?, move |_| {
            task(move |_| {
                lock(&decode_order).push("decode");
                Ok(CpuStep::Complete(2))
            })
        })?;
    let interactive_order = order.clone();
    let mut interactive =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 0)?, move |_| {
            let mut steps = 0;
            task(move |_| {
                steps += 1;
                lock(&interactive_order).push("interactive");
                Ok(if steps == 3 { CpuStep::Complete(3) } else { CpuStep::Yield })
            })
        })?;
    release.open();
    assert!(matches!(rig.completion(&mut barrier)?.outcome, JobOutcome::Success(0)));
    assert!(matches!(rig.completion(&mut decode)?.outcome, JobOutcome::Success(2)));
    assert!(matches!(rig.completion(&mut interactive)?.outcome, JobOutcome::Success(3)));
    assert_eq!(*lock(&order), ["interactive", "decode", "interactive", "interactive"]);
    Ok(())
}

#[path = "cpu_lifecycle_tests.rs"]
mod lifecycle;
