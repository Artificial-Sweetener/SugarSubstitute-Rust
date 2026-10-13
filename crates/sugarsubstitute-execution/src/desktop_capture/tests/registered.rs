// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive actual successful registration, temporary settlement and permanent charge transfer.
use super::*;
/// Fake only the successful native reply; all admission/counter/transfer owners are real.
struct RegisteredDriver(Driver);
impl NativeCaptureDriver for RegisteredDriver {
    fn prepare(
        &mut self,
        admission: crate::CaptureBundleAdmission,
    ) -> Result<(), NativeObservationError> {
        self.0.prepare(admission)
    }
    fn attach(
        &self,
        _: &NativeCaptureInput,
        out: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.0.boundary.attaches.fetch_add(1, Ordering::AcqRel);
        *out = NativeCaptureReply {
            version: 2,
            status: 1,
            registered: 1,
            attach: 1,
            ..Default::default()
        };
        Ok(())
    }
    fn poll(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        std::panic::resume_unwind(Box::new(
            "Historical registered fixture must not poll fresh effects",
        ))
    }
    fn issue_step(
        &self,
        _: &NativeCaptureStep,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        std::panic::resume_unwind(Box::new("Registered fixture requests no finite stage"))
    }
    fn retire(
        &self,
        input: &NativeCaptureProgress,
        out: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        let mut budget = CleanupBudget::default();
        assert!((input.cleanup.request)(input.cleanup.context, &mut budget));
        *out = NativeCaptureReply {
            version: 2,
            status: 4,
            registered: 1,
            attach: input.attach,
            retired: 1,
            cleanup_state: 1,
            cleanup_started: budget.started,
            cleanup_grace: budget.grace,
            cleanup_total: budget.total,
            frequency: budget.frequency,
            ..Default::default()
        };
        self.0.boundary.retired.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn retire_prepared(
        &mut self,
        _: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        std::panic::resume_unwind(Box::new(
            "Permanent fixture capsule must never close its native source",
        ))
    }
    fn retained_pin_bytes(&self) -> u64 {
        self.0.claims.iter().map(MemoryClaim::bytes).sum()
    }
}

/// Registered publication precedes cleanup; temporary transfer preserves full original charge.
#[test]
fn registered_temporary_transfer_retains_original_capsule_without_physical_retirement() -> TestResult
{
    use crate::{
        admission::Core,
        runtime_test_support::{ClockPort, ManualClock, Wake},
        task::Work,
    };
    let now = Moment::from_nanos(0);
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let boundary = Arc::new(Boundary::default());
    let driver =
        RegisteredDriver(Driver { boundary: boundary.clone(), blocked: false, claims: Vec::new() });
    let mut session =
        submission::reserve(&core, &scope, request()?, input()?, Box::new(driver), event)?;
    let (queued, wake) = core.take_native_at(now).ok_or("Actual native child not selected")?;
    core.deliver_capacity_wake(wake);
    let Work::Native(crate::native::NativeWork::DesktopCapture(work)) = queued.work else {
        return Err("Child reached finite factory".into());
    };
    let custody = work.shared.clone();
    let metadata;
    let mut active = worker::DesktopCaptureActive::new(*work, queued.context, &core, now);
    metadata = active.metadata();
    active.progress(&core, now, true);
    let completion =
        session.job().try_take_completion().ok_or("Actual registered result missing")?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    drop(completion);
    assert!(!active.completed());
    assert_eq!(session.control().temporary_settled(), DesktopCaptureTemporaryState::Pending);
    assert!(session.control().try_take_process_lifetime().is_none());
    let original = session.control().request_cleanup();
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
        budget,
        counter: Ok(counter),
    } = original
    else {
        return Err("Actual original cleanup tuple missing".into());
    };
    assert_eq!(budget.graceful_until.as_nanos(), 10_000_000_000);
    assert_eq!(budget.settle_until.as_nanos(), 15_000_000_000);
    active.progress(&core, now, true);
    assert!(active.completed());
    assert_eq!(session.control().temporary_settled(), DesktopCaptureTemporaryState::Transferred);
    let mut permanent = session
        .control()
        .try_take_process_lifetime()
        .ok_or("Actual transfer not delivered")?
        .map_err(|cause| format!("{cause:?}"))?;
    assert_eq!(permanent.reservation_bytes(), 4_194_304);
    assert!(session.control().try_take_process_lifetime().is_none());
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched {
        counter: Ok(after), ..
    } = session.control().cleanup_snapshot()
    else {
        return Err("Original cleanup tuple replaced".into());
    };
    assert_eq!(counter, after);
    {
        let cleanup = lock(&custody.cleanup.state);
        assert!(!cleanup.active);
        assert!(!cleanup.native_retired && !cleanup.source_retired);
    }
    assert_eq!(boundary.retired.load(Ordering::Acquire), 1);
    drop(active);
    drop(metadata);
    drop(custody);
    drop(queued.job);
    drop(session);
    drop(lock(&boundary.context).take());
    assert_eq!(scope.snapshot().reserved_bytes, 4_194_304);
    // This external fixture owns no kernel/module outputs. Release its actual
    // private fake capsule only after the production transfer/retention assertions;
    // no physical retirement flag or public process-exit proof is manufactured.
    let capsule = permanent.capsule.take().ok_or("Transferred fixture capsule missing")?;
    drop(permanent);
    drop(capsule);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
