// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual admitted child worker with only external native effects faked.
use super::{
    event::{CompletionEvent, CompletionEventLease},
    input::InheritedCaptureLease,
    native_abi::{CleanupBudget, CleanupSource, IssueSource},
    resources::{CaptureEventClaim, CaptureEventOutput},
    *,
};
use crate::{
    capture_bundle::{CaptureBundleOrigin, CaptureBundleRetirement},
    native::NativeObservationError,
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

/// The fake records real driver entry and retains genuine issued context for observation.
#[derive(Default)]
struct Boundary {
    entered: Gate,
    release: Gate,
    close_entered: Gate,
    close_release: Gate,
    cause_dropped: Arc<AtomicUsize>,
    context: Mutex<Option<JobContext>>,
    prepares: AtomicUsize,
    attaches: AtomicUsize,
    retired: AtomicUsize,
}
/// Opens the external blocking boundary before actual owner teardown on every unwind.
struct Release(Arc<Boundary>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release.open();
        self.0.close_release.open();
    }
}
/// Observe only the original cause payload lifetime, not allocator/control deallocation.
#[derive(Debug)]
struct CauseProbe(Arc<AtomicUsize>);
impl std::fmt::Display for CauseProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Controlled selection failure")
    }
}
impl std::error::Error for CauseProbe {}
impl Drop for CauseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}
struct Driver {
    boundary: Arc<Boundary>,
    blocked: bool,
    claims: Vec<MemoryClaim>,
}
impl NativeCaptureDriver for Driver {
    fn prepare(
        &mut self,
        admission: crate::CaptureBundleAdmission,
    ) -> Result<(), NativeObservationError> {
        admission.check().map_err(|cause| NativeObservationError::new("test_prepare", cause))?;
        let context = admission
            .original_context()
            .map_err(|cause| NativeObservationError::new("test_context", cause))?;
        self.claims.push(
            context
                .memory()
                .claim(super::DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES)
                .map_err(|cause| NativeObservationError::new("test_native_claim", cause))?,
        );
        self.claims.push(
            context
                .memory()
                .claim(65_536)
                .map_err(|cause| NativeObservationError::new("test_callback_claim", cause))?,
        );
        *lock(&self.boundary.context) = Some(context.clone());
        self.boundary.prepares.fetch_add(1, Ordering::AcqRel);
        self.boundary.entered.open();
        if self.blocked {
            self.boundary.release.wait().map_err(|cause| {
                NativeObservationError::new("test_block", std::io::Error::other(cause.to_string()))
            })?;
            return Err(NativeObservationError::new(
                "test_prepare_failure",
                CauseProbe(self.boundary.cause_dropped.clone()),
            ));
        }
        Ok(())
    }
    fn attach(
        &self,
        _: &NativeCaptureInput,
        out: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        self.boundary.attaches.fetch_add(1, Ordering::AcqRel);
        *out = NativeCaptureReply {
            version: 2,
            status: 2,
            attach: 1,
            primary_error: 5,
            primary_domain: 1,
            ..Default::default()
        };
        Ok(())
    }
    fn poll(
        &self,
        _: &NativeCaptureProgress,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        std::panic::resume_unwind(Box::new("Failed attachment must never poll fresh native work"))
    }
    fn issue_step(
        &self,
        _: &NativeCaptureStep,
        _: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError> {
        std::panic::resume_unwind(Box::new("No stage requested by this external fixture"))
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
            attach: input.attach,
            retired: 1,
            primary_error: 5,
            primary_domain: 1,
            cleanup_state: 1,
            cleanup_started: budget.started,
            cleanup_grace: budget.grace,
            cleanup_total: budget.total,
            frequency: budget.frequency,
            ..Default::default()
        };
        Ok(())
    }
    fn retire_prepared(
        &mut self,
        proof: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        proof.check().map_err(|cause| NativeObservationError::new("test_retire", cause))?;
        if self.blocked {
            self.boundary.close_entered.open();
            self.boundary.close_release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "test_close_block",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        self.claims.clear();
        self.boundary.retired.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
    fn retained_pin_bytes(&self) -> u64 {
        0
    }
}
/// No physical handles exist in this fake; the real worker retains the exact Box.
struct Input;
impl InheritedCaptureLease for Input {
    fn native_input(
        &self,
        cleanup: CleanupSource,
        issue: IssueSource,
        event: usize,
    ) -> Result<NativeCaptureInput, NativeObservationError> {
        Ok(NativeCaptureInput {
            version: 2,
            size: 776,
            subject: 1,
            provenance: 1,
            namespace: [1; 32],
            epoch: [2; 16],
            instance: 1,
            generation: 1,
            creator_pid: 3,
            handler_pid: 5,
            creator_birth: 4,
            handler_birth: 6,
            creator_process: 7,
            handler_process: 8,
            attach_counter: i64::MAX,
            frequency: 1,
            reserved_deadline0: 0,
            reserved_deadline1: 0,
            build_length: 1,
            endpoint_length: 1,
            build: [0; 64],
            endpoint: [0; 256],
            cleanup,
            issue,
            completion_event: event,
        })
    }
    fn return_unissued(&mut self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn retire(&mut self, proof: &CaptureBundleRetirement) -> Result<bool, NativeObservationError> {
        proof.check().map_err(|cause| NativeObservationError::new("test_input_retire", cause))?;
        Ok(true)
    }
}
/// The actual two-output producer supplies this fake's identical outside claim.
struct Event {
    original: CaptureEventClaim,
    event: Option<CaptureEventOutput>,
    wait: Option<CaptureEventOutput>,
    closed: bool,
}
impl CompletionEventLease for Event {
    fn prepare(&mut self) -> Result<(), NativeObservationError> {
        self.original
            .check()
            .map_err(|cause| NativeObservationError::new("test_event_current", cause))?;
        self.event = Some(
            self.original
                .reserve_output()
                .map_err(|cause| NativeObservationError::new("test_event_capacity", cause))?,
        );
        self.wait = Some(
            self.original
                .reserve_output()
                .map_err(|cause| NativeObservationError::new("test_wait_capacity", cause))?,
        );
        assert_eq!(self.original.snapshot(), (2, 2));
        assert!(self.original.reserve_output().is_err());
        Ok(())
    }
    fn raw(&self) -> usize {
        9
    }
    fn close(&mut self, proof: &CaptureBundleRetirement) -> Result<(), NativeObservationError> {
        proof.check().map_err(|cause| NativeObservationError::new("test_event_close", cause))?;
        drop(self.wait.take());
        drop(self.event.take());
        self.closed = true;
        assert_eq!(self.original.snapshot(), (0, 2));
        Ok(())
    }
    fn settled_for(&self, origin: &CaptureBundleOrigin, identity: usize) -> bool {
        self.closed
            && self.original.identity() == identity
            && self.original.original().same_allocation(origin)
    }
}
/// Return the existing inert nonblocked fake driver for the central selector owner.
pub(crate) fn inert_driver() -> Box<dyn NativeCaptureDriver> {
    Box::new(Driver { boundary: Arc::new(Boundary::default()), blocked: false, claims: Vec::new() })
}
/// Construct only inert fake storage from the real issued two-output owner.
pub(crate) fn event(
    _: Arc<crate::native::Signal>,
    original: CaptureEventClaim,
) -> Result<CompletionEvent, NativeObservationError> {
    let value =
        Box::new(Event { original: original.clone(), event: None, wait: None, closed: false });
    Ok(CompletionEvent::from_boundary(value, original))
}
/// Prepare exact run facts with one move-only inert external input.
pub(crate) fn input() -> TestResult<InheritedCaptureAttachInput> {
    let owner = sugarsubstitute_contracts::lifecycle::CoordinatorKey {
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
        epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
    };
    let run = sugarsubstitute_contracts::lifecycle::RunKey {
        owner,
        instance: sugarsubstitute_contracts::lifecycle::InstanceId::new(1)?,
        generation: sugarsubstitute_contracts::lifecycle::RunGeneration::new(1)?,
    };
    Ok(InheritedCaptureAttachInput::simulated(
        sugarsubstitute_contracts::crash::CaptureTarget::Desktop(run),
        Moment::from_nanos(100_000_000_000),
        Box::new(Input),
    ))
}
/// Use the real exact admission requirements, keeping the original cutoff unchanged.
pub(crate) fn request() -> TestResult<JobSpec> {
    let mut value = spec(1, JobClass::InfrastructureLifecycle, 4_194_304)?;
    value.operation = Operation::NativeLifecycle;
    value.deadline = Some(Moment::from_nanos(100_000_000_000));
    Ok(value)
}

/// Observe the actual terminal while retaining each expected structured native failure.
/// Completion notification is not treated as a byte-release or native-retirement signal.
fn completion(
    rig: &Rig,
    session: &mut DesktopCaptureSession,
) -> TestResult<(
    JobCompletion<DesktopCaptureAttached, crate::capture_attach::types::CaptureAttachFailure>,
    Vec<ExecutionError>,
)> {
    let mut causes = Vec::new();
    loop {
        let observed = rig.wake.count();
        match rig.client.drain_completions(std::num::NonZeroUsize::MIN) {
            Ok(_) => {}
            Err(cause) => causes.push(cause),
        }
        if let Some(value) = session.job().try_take_completion() {
            return Ok((value, causes));
        }
        rig.wake.wait_after(observed)?;
    }
}
#[test]
fn over_reserved_rejection_returns_identical_input_and_driver_before_effects() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Arc::new(Boundary::default());
    let inherited = input()?;
    let input_identity = std::ptr::from_ref(inherited.lease()) as *const () as usize;
    let driver: Box<dyn NativeCaptureDriver> =
        Box::new(Driver { boundary: boundary.clone(), blocked: false, claims: Vec::new() });
    let driver_identity = std::ptr::from_ref(driver.as_ref()) as *const () as usize;
    let mut spec = request()?;
    spec.reservation_bytes += 1;
    let rejection = super::submission::reserve(
        &rig.client.core_for_test(),
        &rig.scope,
        spec,
        inherited,
        driver,
        event,
    )
    .err()
    .ok_or("Invalid admission succeeded")?;
    assert!(matches!(rejection.error, AdmissionError::InvalidCaptureAdmission));
    assert_eq!(std::ptr::from_ref(rejection.input.lease()) as *const () as usize, input_identity);
    assert_eq!(
        std::ptr::from_ref(rejection.driver.as_ref()) as *const () as usize,
        driver_identity
    );
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn blocked_prepare_keeps_core_available_and_cleanup_vetoes_effect_before_job_cancel() -> TestResult
{
    let rig = Rig::new()?;
    let boundary = Arc::new(Boundary::default());
    let _release = Release(boundary.clone());
    let mut session = super::submission::reserve(
        &rig.client.core_for_test(),
        &rig.scope,
        request()?,
        input()?,
        Box::new(Driver { boundary: boundary.clone(), blocked: true, claims: Vec::new() }),
        event,
    )?;
    boundary.entered.wait()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4_194_304);
    let snapshot = session.control().request_cleanup();
    assert!(matches!(
        snapshot,
        crate::capture_attach::control::CaptureCleanupSnapshot::Latched { .. }
    ));
    assert!(
        !lock(&boundary.context).as_ref().ok_or("Actual context missing")?.cancellation_requested()
    );
    boundary.release.open();
    let (completion, causes) = completion(&rig, &mut session)?;
    assert!(causes.iter().any(|cause| format!("{cause:?}").contains("test_prepare_failure")));
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.attaches.load(Ordering::Acquire), 0);
    boundary.close_entered.wait()?;
    drop(causes);
    drop(completion);
    assert_eq!(boundary.cause_dropped.load(Ordering::Acquire), 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4_194_304);
    boundary.close_release.open();
    drop(session);
    drop(lock(&boundary.context).take());
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.cause_dropped.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn actual_two_output_owner_retires_after_native_failure_preserving_original_terminal() -> TestResult
{
    let rig = Rig::new()?;
    let boundary = Arc::new(Boundary::default());
    let _release = Release(boundary.clone());
    let mut session = super::submission::reserve(
        &rig.client.core_for_test(),
        &rig.scope,
        request()?,
        input()?,
        Box::new(Driver { boundary: boundary.clone(), blocked: false, claims: Vec::new() }),
        event,
    )?;
    let (completion, causes) = completion(&rig, &mut session)?;
    assert!(causes.iter().any(|cause| format!("{cause:?}").contains("capture_native_operation")));
    assert!(matches!(completion.outcome, JobOutcome::Failure(_)));
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 1);
    assert_eq!(boundary.attaches.load(Ordering::Acquire), 1);
    drop(completion);
    drop(session);
    drop(lock(&boundary.context).take());
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.retired.load(Ordering::Acquire), 1);
    Ok(())
}

mod registered;
#[path = "tests/registered_lifetime.rs"]
mod registered_lifetime;
