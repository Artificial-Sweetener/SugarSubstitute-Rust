// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe CPU cancellation, panic and actual teardown at controlled boundaries.

use super::*;

/// Inject a standard unwind payload into the production supervisor boundary.
fn fixture_unwind(message: &'static str) -> ! {
    std::panic::resume_unwind(Box::new(message))
}

#[test]
fn expired_pending_cpu_deadline_does_not_spin_the_async_supervisor() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            task(move |_| {
                worker_entered.open();
                worker_release.wait().map_err(|error| error.to_string())?;
                Ok(CpuStep::Complete(7))
            })
        })?;
    entered.wait()?;
    let called = Arc::new(AtomicBool::new(false));
    let worker_called = called.clone();
    let mut request = spec(2, JobClass::CpuInteractive, 0)?;
    request.deadline = Some(Moment::from_nanos(100));
    let mut pending = rig.client.submit_cpu(&rig.scope, request, move |_| {
        worker_called.store(true, Ordering::SeqCst);
        success()
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(100))?;
    rig.clock.advance(100);
    let mut barrier =
        rig.client.submit_async(&rig.scope, spec(3, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    assert!(matches!(rig.completion(&mut barrier)?.outcome, JobOutcome::Success(())));
    assert_eq!(active.status().phase, JobPhase::Running);
    assert_eq!(pending.status().phase, JobPhase::Queued);
    assert_eq!(rig.clock.expired_sleep_calls.load(Ordering::SeqCst), 0);
    release.open();
    assert!(matches!(rig.completion(&mut pending)?.outcome, JobOutcome::TimedOut));
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(7)));
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn deadline_during_step_waits_for_actual_return_and_discards_success() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let mut request = spec(1, JobClass::CpuInteractive, 0)?;
    request.deadline = Some(Moment::from_nanos(5));
    let mut job = rig.client.submit_cpu(&rig.scope, request, move |_| {
        task(move |_| {
            worker_entered.open();
            worker_release.wait().map_err(|error| error.to_string())?;
            Ok(CpuStep::Complete(7))
        })
    })?;
    entered.wait()?;
    rig.clock.advance(5);
    assert!(job.try_take_completion().is_none());
    assert_eq!(job.status().phase, JobPhase::Running);
    release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    Ok(())
}

#[test]
fn abandoning_started_cpu_work_retains_actual_join_and_releases_bytes() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let destroyed = Arc::new(AtomicBool::new(false));
    let (a, b, c) = (entered.clone(), release.clone(), destroyed.clone());
    let job = rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuDecode, 128)?, move |_| {
        let mut work = task(move |_| {
            a.open();
            b.wait().map_err(|error| error.to_string())?;
            Ok(CpuStep::Complete(7))
        });
        work.on_drop = Some(Box::new(move || c.store(true, Ordering::SeqCst)));
        work
    })?;
    entered.wait()?;
    drop(job);
    assert_eq!(rig.scope.snapshot().running, 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 128);
    assert!(!destroyed.load(Ordering::SeqCst));
    release.open();
    drop(rig.owner);
    assert!(destroyed.load(Ordering::SeqCst));
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(rig.scope.snapshot().running, 0);
    Ok(())
}

#[test]
fn typed_cpu_failure_retains_original_error_and_worker_recovers() -> TestResult {
    let rig = Rig::new()?;
    let mut failure =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, |_| {
            task(|_| Err("Original CPU domain cause".into()))
        })?;
    assert!(
        matches!(rig.completion(&mut failure)?.outcome, JobOutcome::Failure(cause) if cause == "Original CPU domain cause")
    );
    let mut next =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 0)?, |_| success())?;
    assert!(matches!(rig.completion(&mut next)?.outcome, JobOutcome::Success(7)));
    Ok(())
}

#[test]
fn cancellation_and_shutdown_wait_for_step_and_destructor() -> TestResult {
    let mut rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let destroying = Arc::new(Gate::default());
    let destroy_release = Arc::new(Gate::default());
    let (a, b, c, d) =
        (entered.clone(), release.clone(), destroying.clone(), destroy_release.clone());
    let mut job =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 64)?, move |_| {
            let mut work = task(move |_| {
                a.open();
                b.wait().map_err(|e| e.to_string())?;
                Ok(CpuStep::Complete(7))
            });
            work.on_drop = Some(Box::new(move || {
                c.open();
                assert!(d.wait().is_ok());
            }));
            work
        })?;
    entered.wait()?;
    job.cancel();
    rig.owner.request_shutdown(Moment::from_nanos(5));
    rig.clock.advance(5);
    assert!(job.try_take_completion().is_none());
    let progress = rig.owner.poll_shutdown()?;
    assert!(progress.deadline_expired && !progress.settled);
    assert_eq!(progress.unresolved_job_ids, [job.spec().id]);
    assert!(rig.owner.finish_shutdown().is_err());
    release.open();
    destroying.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(job.status().phase, JobPhase::Running);
    destroy_release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    drop(job);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn cpu_factory_step_and_task_drop_panics_are_typed_and_lane_survives() -> TestResult {
    let rig = Rig::new()?;
    let mut factory =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, |_| -> Task {
            fixture_unwind("CPU factory fixture")
        })?;
    assert!(
        matches!(rig.completion(&mut factory)?.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "CPU factory fixture")
    );
    let mut step =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 0)?, |_| {
            task(|_| fixture_unwind("CPU step fixture"))
        })?;
    assert!(
        matches!(rig.completion(&mut step)?.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "CPU step fixture")
    );
    let mut destruction =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 0)?, |_| {
            let mut work = success();
            work.on_drop = Some(Box::new(|| fixture_unwind("CPU destructor fixture")));
            work
        })?;
    assert!(
        matches!(rig.completion(&mut destruction)?.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "CPU destructor fixture")
    );
    let mut next =
        rig.client.submit_cpu(&rig.scope, spec(4, JobClass::CpuInteractive, 0)?, |_| success())?;
    assert!(matches!(rig.completion(&mut next)?.outcome, JobOutcome::Success(7)));
    Ok(())
}

/// Panic only when the never-invoked application factory releases its captured state.
struct FactoryDrop;
impl Drop for FactoryDrop {
    fn drop(&mut self) {
        fixture_unwind("Pending factory destructor fixture");
    }
}

#[test]
fn cancelled_factory_destruction_is_observed_before_terminal() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let work_entered = entered.clone();
    let work_release = release.clone();
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuDecode, 0)?, move |_| {
            task(move |_| {
                work_entered.open();
                work_release.wait().map_err(|e| e.to_string())?;
                Ok(CpuStep::Complete(0))
            })
        })?;
    entered.wait()?;
    let captured = FactoryDrop;
    let mut pending =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 0)?, move |_| {
            drop(captured);
            success()
        })?;
    pending.cancel();
    release.open();
    assert!(
        matches!(rig.completion(&mut pending)?.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "Pending factory destructor fixture")
    );
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(0)));
    Ok(())
}

/// Hold actual capture destruction at an external boundary and observe its thread.
struct ControlledDrop {
    entered: Arc<Gate>,
    release: Arc<Gate>,
    thread: Arc<Mutex<Option<std::thread::ThreadId>>>,
}
impl Drop for ControlledDrop {
    fn drop(&mut self) {
        *lock(&self.thread) = Some(std::thread::current().id());
        self.entered.open();
        assert!(self.release.wait().is_ok());
    }
}

#[test]
fn coalesced_factory_destruction_runs_on_worker_after_submit_returns() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let caller = std::thread::current().id();
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let destroying = Arc::new(Gate::default());
    let destroy_release = Arc::new(Gate::default());
    let thread = Arc::new(Mutex::new(None));
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let mut active =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            task(move |_| {
                worker_entered.open();
                worker_release.wait().map_err(|error| error.to_string())?;
                Ok(CpuStep::Complete(1))
            })
        })?;
    entered.wait()?;
    let captured = ControlledDrop {
        entered: destroying.clone(),
        release: destroy_release.clone(),
        thread: thread.clone(),
    };
    let called = Arc::new(AtomicBool::new(false));
    let worker_called = called.clone();
    let mut replaced =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 128)?, move |_| {
            worker_called.store(true, Ordering::SeqCst);
            drop(captured);
            success()
        })?;
    let mut newest =
        rig.client.submit_cpu(&rig.scope, spec(3, JobClass::CpuInteractive, 64)?, |_| success())?;
    // Submit returned while the sole worker remains inside the active step.
    assert!(lock(&thread).is_none());
    assert!(replaced.try_take_completion().is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, 192);
    release.open();
    destroying.wait()?;
    assert!(lock(&thread).is_some_and(|worker| worker != caller));
    assert!(!called.load(Ordering::SeqCst));
    assert!(replaced.try_take_completion().is_none());
    assert_eq!(replaced.status().phase, JobPhase::Running);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 192);
    destroy_release.open();
    assert!(
        matches!(rig.completion(&mut replaced)?.outcome, JobOutcome::Coalesced { replacement } if replacement == JobId::new(3)?)
    );
    assert!(matches!(rig.completion(&mut active)?.outcome, JobOutcome::Success(1)));
    assert!(matches!(rig.completion(&mut newest)?.outcome, JobOutcome::Success(7)));
    Ok(())
}

#[test]
fn abandoned_async_result_uses_cpu_cleanup_and_blocks_shutdown_until_destroyed() -> TestResult {
    let mut rig = Rig::with_workers(1)?;
    let caller = std::thread::current().id();
    let destroying = Arc::new(Gate::default());
    let destroy_release = Arc::new(Gate::default());
    let thread = Arc::new(Mutex::new(None));
    let captured = ControlledDrop {
        entered: destroying.clone(),
        release: destroy_release.clone(),
        thread: thread.clone(),
    };
    let observed = rig.wake.count();
    let job = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoLifecycle, 128)?,
        move |_| async move { Ok::<_, String>(captured) },
    )?;
    rig.wake.wait_after(observed)?;
    assert_eq!(job.status().phase, JobPhase::Terminal(TerminalKind::Success));
    drop(job);
    // Abandon returned without waiting for the untransferred result's destructor.
    destroying.wait()?;
    assert!(lock(&thread).is_some_and(|worker| worker != caller));
    assert_eq!(rig.scope.snapshot().reserved_bytes, 128);
    assert_eq!(rig.scope.snapshot().running, 1);
    rig.owner.request_shutdown(Moment::from_nanos(5));
    rig.clock.advance(5);
    let progress = rig.owner.poll_shutdown()?;
    assert!(progress.deadline_expired && !progress.settled);
    assert_eq!(progress.unresolved_job_ids, [JobId::new(1)?]);
    assert!(rig.owner.finish_shutdown().is_err());
    destroy_release.open();
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(rig.scope.snapshot().running, 0);
    Ok(())
}
