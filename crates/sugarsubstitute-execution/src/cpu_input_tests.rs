// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exclusive CPU input follows real admission, worker custody and deterministic stop boundaries.
use crate::{
    OwnedInputError,
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::ThreadId,
};
use sugarsubstitute_contracts::execution::*;

/// Run one controlled CPU step; the real execution owner supervises its factory and destruction.
struct Step<F>(Option<F>);
impl<T: Send + 'static, F> CpuTask<T, io::Error> for Step<F>
where
    F: FnOnce(&JobContext) -> Result<CpuStep<T>, io::Error> + Send + 'static,
{
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<T>, io::Error> {
        self.0.take().ok_or_else(|| io::Error::other("CPU test step already consumed"))?(context)
    }
}
/// Release external test boundaries before the execution owner joins, even on assertion failure.
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
/// Observe actual input destruction without using timing or polling for a likely order.
struct InputDrop(Arc<Mutex<Option<ThreadId>>>);
impl Drop for InputDrop {
    fn drop(&mut self) {
        *lock(&self.0) = Some(std::thread::current().id());
    }
}

#[test]
fn admitted_graph_evaluation_cannot_occupy_interactive_display_capacity() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut graph_spec = spec(1, JobClass::CpuDecode, 4096)?;
    graph_spec.operation = Operation::ImageEvaluate;
    let mut graph = rig.client.submit_cpu(&rig.scope, graph_spec, move |_| {
        Step(Some(move |_: &JobContext| {
            worker_entered.open();
            worker_returned.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
            Ok(CpuStep::Complete(()))
        }))
    })?;
    entered.wait()?;
    let mut input = Some(Arc::new(()));
    let original = input.as_ref().ok_or("Display input absent")?.clone();
    let mut display = rig.client.submit_cpu_owned_input(
        &rig.scope,
        spec(2, JobClass::CpuInteractive, 4096)?,
        &mut input,
        |value, _| Step(Some(move |_: &JobContext| Ok(CpuStep::Complete(value)))),
    )?;
    let JobOutcome::Success(actual) = rig.completion(&mut display)?.outcome else {
        return Err("Independent interactive work did not complete".into());
    };
    assert!(Arc::ptr_eq(&actual, &original));
    assert!(graph.try_take_completion().is_none());
    returned.open();
    assert!(matches!(rig.completion(&mut graph)?.outcome, JobOutcome::Success(())));
    Ok(())
}

#[test]
fn cpu_owned_input_rejection_preserves_identity_and_success_runs_factory_on_worker() -> TestResult {
    let rig = Rig::new()?;
    let original = Arc::new(());
    let mut input = Some(original.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    for (id, class, bytes) in [
        (1, JobClass::InfrastructureOrdinary, 4096),
        (2, JobClass::CpuInteractive, 512 * 1024 * 1024),
    ] {
        let worker_calls = calls.clone();
        let result = rig.client.submit_cpu_owned_input(
            &rig.scope,
            spec(id, class, bytes)?,
            &mut input,
            move |value, _| {
                worker_calls.fetch_add(1, Ordering::AcqRel);
                Step(Some(move |_: &JobContext| Ok(CpuStep::Complete(value))))
            },
        );
        assert!(matches!(result, Err(OwnedInputError::Admission(_))));
        assert!(Arc::ptr_eq(input.as_ref().ok_or("Input lost on rejection")?, &original));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    }
    let mut absent: Option<Arc<()>> = None;
    assert!(matches!(
        rig.client.submit_cpu_owned_input(
            &rig.scope,
            spec(3, JobClass::CpuInteractive, 4096)?,
            &mut absent,
            |value, _| { Step(Some(move |_: &JobContext| Ok(CpuStep::Complete(value)))) }
        ),
        Err(OwnedInputError::MissingInput)
    ));
    let caller = std::thread::current().id();
    let mut job = rig.client.submit_cpu_owned_input(
        &rig.scope,
        spec(4, JobClass::CpuInteractive, 4096)?,
        &mut input,
        move |value, _| {
            assert_ne!(std::thread::current().id(), caller);
            Step(Some(move |_: &JobContext| Ok(CpuStep::Complete(value))))
        },
    )?;
    assert!(input.is_none());
    let JobOutcome::Success(value) = rig.completion(&mut job)?.outcome else {
        return Err("CPU input did not complete".into());
    };
    assert!(Arc::ptr_eq(&value, &original));
    Ok(())
}

#[test]
fn cancelled_queued_cpu_input_is_destroyed_off_caller_without_invoking_factory() -> TestResult {
    let rig = Rig::with_workers(1)?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut blocker =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuDecode, 4096)?, move |_| {
            Step(Some(move |_: &JobContext| {
                worker_entered.open();
                worker_returned.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
                Ok(CpuStep::Complete(()))
            }))
        })?;
    entered.wait()?;
    let dropped = Arc::new(Mutex::new(None));
    let mut input = Some(InputDrop(dropped.clone()));
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = calls.clone();
    let mut job = rig.client.submit_cpu_owned_input(
        &rig.scope,
        spec(2, JobClass::CpuInteractive, 4096)?,
        &mut input,
        move |value, _| {
            worker_calls.fetch_add(1, Ordering::AcqRel);
            Step(Some(move |_: &JobContext| {
                drop(value);
                Ok(CpuStep::Complete(()))
            }))
        },
    )?;
    assert!(input.is_none());
    job.cancel();
    assert_eq!(*lock(&dropped), None);
    returned.open();
    assert!(matches!(rig.completion(&mut blocker)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    let destroyed_on = lock(&dropped).ok_or("Queued input destruction unobserved")?;
    assert_ne!(destroyed_on, std::thread::current().id());
    Ok(())
}

#[test]
fn cpu_owned_input_obeys_original_injected_deadline_and_destroys_yielded_state() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let dropped = Arc::new(Mutex::new(None));
    let mut input = Some(InputDrop(dropped.clone()));
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut request = spec(1, JobClass::CpuInteractive, 4096)?;
    request.deadline = Some(Moment::from_nanos(10));
    let mut job =
        rig.client.submit_cpu_owned_input(&rig.scope, request, &mut input, move |value, _| {
            Step(Some(move |_: &JobContext| {
                worker_entered.open();
                worker_returned.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
                std::hint::black_box(&value);
                Ok::<_, io::Error>(CpuStep::<()>::Yield)
            }))
        })?;
    entered.wait()?;
    rig.clock.advance(10);
    returned.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    let destroyed_on = lock(&dropped).ok_or("Yielded input destruction unobserved")?;
    assert_ne!(destroyed_on, std::thread::current().id());
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
