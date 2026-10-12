// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise atomic queued and simultaneous full-run admission through the real executor.
use super::{conflict_tests::*, test_support::*, *};
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::{execution::*, lifecycle::InstanceId};

struct HeldPrepare {
    boundary: Arc<Boundary>,
    entered: Arc<Gate>,
    release: Arc<Gate>,
}
impl AttachLease for HeldPrepare {
    fn prepare(
        &mut self,
        observed: types::CaptureEndpointObservation,
        context: &JobContext,
        signal: Arc<Signal>,
        shared: &Arc<Shared>,
    ) -> Result<(), NativeObservationError> {
        self.entered.open();
        self.release.wait().map_err(|cause| {
            NativeObservationError::new(
                "controlled_prepare",
                std::io::Error::other(cause.to_string()),
            )
        })?;
        Lease(self.boundary.clone()).prepare(observed, context, signal, shared)
    }
    fn progress(&mut self) -> Result<Option<[usize; 2]>, NativeObservationError> {
        Lease(self.boundary.clone()).progress()
    }
    fn prepared(&self) -> bool {
        Lease(self.boundary.clone()).prepared()
    }
    fn retire(
        &mut self,
        borrowed: bool,
        cleanup: &CaptureCleanupSnapshot,
    ) -> Result<bool, NativeObservationError> {
        Lease(self.boundary.clone()).retire(borrowed, cleanup)
    }
}
struct ReleaseGate(Arc<Gate>);
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        self.0.open();
    }
}
#[test]
fn queued_capture_run_conflicts_across_scopes_before_any_source_factory_or_effect() -> TestResult {
    let rig = Rig::new()?;
    let first = Arc::new(scope(&rig)?);
    let second = Arc::new(another_scope(&rig, 3)?);
    let blocker = Arc::new(Boundary::default());
    let _release_blocker = ReleaseBoundary::new(&rig.client.core, &blocker);
    blocker.ready.store(true, Ordering::Release);
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = ReleaseGate(release.clone());
    let mut other_run = expected()?.run();
    other_run.instance = InstanceId::new(2)?;
    let mut blocking = admission::reserve(
        &rig.client.core,
        &first,
        attach_spec(1)?,
        expectation(other_run)?,
        blocker.clone(),
        Box::new(HeldPrepare {
            boundary: blocker.clone(),
            entered: entered.clone(),
            release: release.clone(),
        }),
    )
    .map_err(|cause| format!("{cause:?}"))?;
    entered.wait()?;
    let queued_boundary = Arc::new(Boundary::default());
    let _release_queued = ReleaseBoundary::new(&rig.client.core, &queued_boundary);
    queued_boundary.ready.store(true, Ordering::Release);
    let mut queued = admission::reserve(
        &rig.client.core,
        &first,
        attach_spec(2)?,
        expected()?,
        queued_boundary.clone(),
        Box::new(Lease(queued_boundary.clone())),
    )
    .map_err(|cause| format!("{cause:?}"))?;
    assert_eq!(queued.status().phase, JobPhase::Queued);
    rejected(&rig, &second, attach_spec(2)?, expected()?)?;
    assert_eq!(queued_boundary.observations.load(Ordering::Acquire), 0);
    assert_eq!(queued_boundary.starts.load(Ordering::Acquire), 0);
    release.open();
    let blocker_pin = take_pin(&rig, &mut blocking)?;
    let queued_pin = take_pin(&rig, &mut queued)?;
    let _first_retirement = blocker_pin.release();
    blocker.settle()?;
    blocker.source_retired.wait()?;
    let _second_retirement = queued_pin.release();
    queued_boundary.settle()?;
    queued_boundary.source_retired.wait()?;
    drop(blocking);
    drop(queued);
    drop(rig.owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    assert_eq!(second.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn two_actual_blocking_factories_concurrently_admitting_same_run_have_one_atomic_winner()
-> TestResult {
    let rig = Rig::new()?;
    let first = Arc::new(scope(&rig)?);
    let second = Arc::new(another_scope(&rig, 3)?);
    let entered = Arc::new(AtomicUsize::new(0));
    let both = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = ReleaseGate(release.clone());
    let boundaries = [Arc::new(Boundary::default()), Arc::new(Boundary::default())];
    let _boundary_guards: Vec<_> = boundaries
        .iter()
        .map(|boundary| ReleaseBoundary::new(&rig.client.core, boundary))
        .collect();
    for boundary in &boundaries {
        boundary.ready.store(true, Ordering::Release);
    }
    let sources: [Arc<dyn types::CaptureEndpointSource>; 2] =
        [boundaries[0].clone(), boundaries[1].clone()];
    let mut drivers = Vec::new();
    for (side, native) in [first.clone(), second.clone()].into_iter().enumerate() {
        let entered = entered.clone();
        let both = both.clone();
        let release = release.clone();
        let boundary = boundaries[side].clone();
        let source = sources[side].clone();
        let core = rig.client.core.clone();
        let expected = expected()?;
        let request = attach_spec(3)?;
        drivers.push(rig.client.submit_blocking(
            &rig.scope,
            spec(side as u64 + 1, JobClass::InfrastructureLifecycle, 4096)?,
            move |_| {
                if entered.fetch_add(1, Ordering::AcqRel) + 1 == 2 {
                    both.open();
                }
                release.wait().map_err(|cause| std::io::Error::other(cause.to_string()))?;
                Ok::<_, std::io::Error>((
                    side,
                    admission::reserve(
                        &core,
                        &native,
                        request,
                        expected,
                        source,
                        Box::new(Lease(boundary)),
                    ),
                ))
            },
        )?);
    }
    both.wait()?;
    assert_eq!(entered.load(Ordering::Acquire), 2);
    release.open();
    let mut winner = None;
    let mut losers = 0;
    for driver in &mut drivers {
        let completion = rig.completion(driver)?;
        let JobOutcome::Success((side, attempt)) = completion.outcome else {
            return Err("Concurrent driver did not complete".into());
        };
        match attempt {
            Ok(job) => {
                assert!(winner.is_none());
                winner = Some((side, job));
            }
            Err(rejection) => {
                losers += 1;
                assert!(
                    matches!(rejection.error,admission::CaptureAttachAdmissionError::RunAlreadyReserved(run) if run==expected()?.run())
                );
                assert!(Arc::ptr_eq(&rejection.source, &sources[side]));
                assert_eq!(rejection.expected, expected()?);
                assert_eq!(boundaries[side].observations.load(Ordering::Acquire), 0);
                assert_eq!(boundaries[side].starts.load(Ordering::Acquire), 0);
            }
        }
        drop(completion.memory);
    }
    assert_eq!(losers, 1);
    let (side, mut job) = winner.ok_or("Atomic admission had no winner")?;
    let snapshots = [first.snapshot(), second.snapshot()];
    assert_eq!(snapshots[side].reserved_bytes, 16_384);
    assert_eq!(snapshots[1 - side].reserved_bytes, 0);
    let pin = take_pin(&rig, &mut job)?;
    let _retirement = pin.release();
    boundaries[side].settle()?;
    boundaries[side].source_retired.wait()?;
    drop(job);
    drop(drivers);
    drop(rig.owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    assert_eq!(second.snapshot().reserved_bytes, 0);
    Ok(())
}
