// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive genuine cancelled native work without inventing physical retirement facts.
use super::{
    input::InheritedCaptureLease,
    native_abi::{CleanupSource, IssueSource},
    *,
};
use crate::{
    admission::Core,
    capture_bundle::CaptureBundleRetirement,
    native::NativeObservationError,
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake},
    scope::lock,
    task::Work,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

/// The external fixture owns no physical handles and observes only unused loan return.
struct UnusedInput(Arc<AtomicUsize>);
impl InheritedCaptureLease for UnusedInput {
    fn native_input(
        &self,
        _: CleanupSource,
        _: IssueSource,
        _: usize,
    ) -> Result<NativeCaptureInput, NativeObservationError> {
        std::panic::resume_unwind(Box::new("Cancelled unissued input must never dispatch"))
    }
    fn return_unissued(&mut self) -> Result<(), NativeObservationError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire(&mut self, _: &CaptureBundleRetirement) -> Result<bool, NativeObservationError> {
        std::panic::resume_unwind(Box::new("Unused loan return must not issue physical close"))
    }
}
/// Genuine queued cancellation must return its original unused loan and detach metadata.
#[test]
fn cancelled_before_native_dispatch_returns_unused_loan_without_physical_retirement() -> TestResult
{
    let now = Moment::from_nanos(0);
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut input = tests::input()?;
    let returned = Arc::new(AtomicUsize::new(0));
    input.replace_lease(Box::new(UnusedInput(returned.clone())));
    let mut session = submission::reserve(
        &core,
        &scope,
        tests::request()?,
        input,
        tests::inert_driver(),
        tests::event,
    )?;
    assert_eq!(session.control().temporary_settled(), DesktopCaptureTemporaryState::Pending);
    session.job().cancel();
    let (queued, wake) =
        core.take_native_at(now).ok_or("Cancelled child was not selected by native owner")?;
    core.deliver_capacity_wake(wake);
    let Work::Native(crate::native::NativeWork::DesktopCapture(work)) = queued.work else {
        return Err("Cancelled child reached finite factory".into());
    };
    let custody = work.shared.clone();
    let metadata;
    let mut active = worker::DesktopCaptureActive::new(*work, queued.context, &core, now);
    metadata = active.metadata();
    active.progress(&core, now, true);
    assert!(active.completed());
    assert_eq!(
        session.control().temporary_settled(),
        DesktopCaptureTemporaryState::UnissuedReturned
    );
    assert_eq!(returned.load(Ordering::Acquire), 1);
    {
        let stage = lock(&custody.state);
        assert_eq!(stage.phase, control::DesktopCapturePhase::UnissuedReturned);
        assert!(!stage.registered_published);
    }
    {
        let cleanup = lock(&custody.cleanup.state);
        assert!(!cleanup.active);
        assert!(!cleanup.native_retired);
        assert!(!cleanup.source_retired);
    }
    let result = session.job().try_take_completion().ok_or("Cancelled child terminal missing")?;
    assert!(matches!(result.outcome, JobOutcome::Cancelled));
    drop(result);
    drop(active);
    drop(metadata);
    drop(custody);
    drop(queued.job);
    drop(session);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
