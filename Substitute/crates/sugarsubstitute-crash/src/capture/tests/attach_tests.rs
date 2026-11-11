// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise the real registered producer, replay source and central rejection ownership.
use super::*;
use crate::capture::attach::commit_admitted;
use sugarsubstitute_contracts::crash::CaptureTarget;
use sugarsubstitute_contracts::{incident::BuildIdentity, lifecycle::*};
use sugarsubstitute_execution::windows_capture_attach::*;

struct ReleaseWatch(Arc<Gate>);
impl Drop for ReleaseWatch {
    fn drop(&mut self) {
        self.0.open();
    }
}
struct Fixture {
    release: ReleaseWatch,
    rig: Rig,
    capture: WindowsCrashCapture,
    port: Arc<FakePort>,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let rig = Rig::new()?;
        let port = Arc::new(FakePort::default());
        port.healthy.store(1, Ordering::Release);
        port.endpoint_unavailable.store(1, Ordering::Release);
        port.release.open();
        let entered = Arc::new(Gate::default());
        let release = Arc::new(Gate::default());
        let release_guard = ReleaseWatch(release.clone());
        let watch_entered = entered.clone();
        let permit = rig.watch(1)?;
        let correlation = NativeCorrelation::new(1)?;
        let mut watch_spec = close_spec(2, Moment::from_nanos(60_000_000_000))?;
        watch_spec.class = JobClass::InfrastructureOrdinary;
        watch_spec.operation = Operation::Infrastructure;
        let watch = rig.client.submit_blocking(&rig.scope, watch_spec, move |_| {
            watch_entered.open();
            if let Err(cause) = release.wait() {
                std::panic::resume_unwind(Box::new(cause.to_string()));
            }
            Ok::<_, sugarsubstitute_execution::windows::NativeObservationError>(
                sugarsubstitute_execution::windows::ProcessExit {
                    correlation,
                    process_id: 17,
                    creation_time_100ns: 23,
                    exit_code: 0,
                },
            )
        })?;
        // This actual typed watcher uses only the fake external process boundary.
        *lock(&port.watch_job) = Some(watch);
        entered.wait()?;
        let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
        let budget = CaptureBudget::new(Moment::from_nanos(0))?;
        capture
            .submit_start(
                &rig.client,
                &rig.scope,
                work_spec(3, budget.until())?,
                budget,
                permit,
                input()?,
            )
            .map_err(|_| "Supervisor fixture start rejected")?;
        rig.observe_until(&mut capture, |capture| {
            capture.snapshot().state == CaptureState::Registered
        })?;
        Ok(Self { release: release_guard, rig, capture, port })
    }
    fn expected(&self) -> TestResult<CaptureAttachExpectation> {
        let owner = coordinator()?;
        let run =
            RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
        Ok(CaptureAttachExpectation::new(
            owner,
            CaptureTarget::Desktop(run),
            &BuildIdentity::new("test-build")?,
            CaptureProcessFacts::new(1, 11)?,
            CaptureProcessFacts::new(17, 23)?,
            CaptureBudget::new(Moment::from_nanos(0))?,
        )?)
    }
    fn finish(mut self) -> TestResult {
        self.release.0.open();
        self.rig.observe_until(&mut self.capture, |capture| capture.watch.is_none())?;
        for job in lock(&self.port.cancelled_permits).iter_mut() {
            assert!(matches!(self.rig.completion(job)?.outcome, JobOutcome::Cancelled));
        }
        self.capture.submit_close(
            &self.rig.client,
            &self.rig.scope,
            close_spec(100, Moment::from_nanos(60_000_000_000))?,
        )?;
        self.rig.observe_until(&mut self.capture, |capture| {
            capture.snapshot().state == CaptureState::Closed
        })?;
        drop(self.capture);
        // Joining the actual execution owner retires the worker's final context
        // guard. Source retirement alone does not prove its byte lease is gone.
        let native_scope = self.rig.join();
        assert_eq!(native_scope.snapshot().reserved_bytes, 0);
        Ok(())
    }
}
fn attach_spec(id: u64) -> TestResult<JobSpec> {
    let mut request = close_spec(id, Moment::from_nanos(60_000_000_000))?;
    request.class = JobClass::NativeObservation;
    request.reservation_bytes = 16_384;
    request.deadline = None;
    Ok(request)
}
#[test]
fn dormant_registration_cannot_mint_a_source_or_admit_capture_work() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let owner = coordinator()?;
    let expected = CaptureAttachExpectation::new(
        owner,
        CaptureTarget::Desktop(RunKey {
            owner,
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        }),
        &BuildIdentity::new("test-build")?,
        CaptureProcessFacts::new(1, 11)?,
        CaptureProcessFacts::new(17, 23)?,
        CaptureBudget::new(Moment::from_nanos(0))?,
    )?;
    let error = capture
        .request_attach_bootstrap(&rig.client, &rig.native_scope, attach_spec(1)?, expected)
        .err()
        .ok_or("Dormant source admitted capture")?;
    assert!(matches!(*error, CaptureBootstrapRejection::Source(_)));
    assert_eq!(port.created.load(Ordering::Acquire), 0);
    assert_eq!(port.endpoint_reads.load(Ordering::Acquire), 0);
    assert_eq!(rig.native_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_registered_owner_validates_identity_and_returns_exact_unadmitted_view() -> TestResult {
    let fixture = Fixture::new()?;
    let before = fixture.rig.native_scope.snapshot().reserved_bytes;
    let expected = fixture.expected()?;
    let owner = expected.owner();
    let mut foreign = owner;
    foreign.epoch = CoordinatorEpoch::new([77; 16])?;
    let mut foreign_run = expected.target().desktop().ok_or("Fixture desktop target absent")?;
    foreign_run.owner = foreign;
    for invalid in [
        CaptureAttachExpectation::new(
            foreign,
            CaptureTarget::Desktop(foreign_run),
            &BuildIdentity::new("test-build")?,
            expected.creator(),
            expected.handler(),
            expected.attach_budget(),
        )?,
        CaptureAttachExpectation::new(
            owner,
            expected.target(),
            &BuildIdentity::new("foreign-build")?,
            expected.creator(),
            expected.handler(),
            expected.attach_budget(),
        )?,
        CaptureAttachExpectation::new(
            owner,
            expected.target(),
            &BuildIdentity::new("test-build")?,
            expected.creator(),
            CaptureProcessFacts::new(19, 23)?,
            expected.attach_budget(),
        )?,
    ] {
        let error = fixture
            .capture
            .request_attach_bootstrap(
                &fixture.rig.client,
                &fixture.rig.native_scope,
                attach_spec(4)?,
                invalid,
            )
            .err()
            .ok_or("Foreign source binding accepted")?;
        assert!(matches!(*error, CaptureBootstrapRejection::Source(_)));
    }
    let alien = Rig::new()?;
    let error = fixture
        .capture
        .request_attach_bootstrap(&alien.client, &alien.native_scope, attach_spec(4)?, expected)
        .err()
        .ok_or("Foreign execution scope accepted")?;
    assert!(matches!(*error, CaptureBootstrapRejection::Source(_)));
    drop(alien);
    let mut request = attach_spec(4)?;
    request.deadline = Some(Moment::from_nanos(1));
    let error = fixture
        .capture
        .request_attach_bootstrap(&fixture.rig.client, &fixture.rig.native_scope, request, expected)
        .err()
        .ok_or("Conflicting deadline accepted")?;
    let CaptureBootstrapRejection::Admission(rejection) = *error else {
        return Err("Original central rejection lost".into());
    };
    let original_source = rejection.source.clone();
    let (cause, returned, source) = (*rejection).into_parts();
    assert!(matches!(cause, CaptureAttachAdmissionError::ConflictingDeadline));
    assert_eq!(returned, expected);
    assert!(Arc::ptr_eq(&original_source, &source));
    assert_eq!(fixture.rig.native_scope.snapshot().reserved_bytes, before);
    assert_eq!(fixture.port.endpoint_reads.load(Ordering::Acquire), 0);
    drop(original_source);
    drop(source);
    fixture.finish()
}
#[test]
fn worker_first_actual_retirement_preserves_late_producer_commit_and_rejects_foreign_actual_job()
-> TestResult {
    let fixture = Fixture::new()?;
    let expected = fixture.expected()?;
    let source = fixture.capture.reserve_attach_source(&fixture.rig.native_scope, expected)?;
    let boundary: Arc<dyn CaptureEndpointSource> = source.clone();
    let mut job = fixture
        .rig
        .client
        .reserve_capture_attach_pin(&fixture.rig.native_scope, attach_spec(4)?, expected, boundary)
        .map_err(|cause| format!("{cause:?}"))?;
    let completion = fixture.rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Failure(CaptureAttachFailure::Source(_))));
    drop(completion);
    assert_eq!(
        source.retired_association(),
        Some((fixture.rig.native_scope.owner_id(), JobId::new(4)?))
    );
    // The real worker has already committed and physically/source retired before
    // this same production finishing step returns to the producer.
    let mut job = commit_admitted(source.clone(), fixture.rig.native_scope.owner_id(), job)?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(fixture.port.endpoint_reads.load(Ordering::Acquire), 1);
    let mut ordinary = close_spec(5, Moment::from_nanos(60_000_000_000))?;
    ordinary.class = JobClass::IoLifecycle;
    ordinary.operation = Operation::Startup;
    let foreign_job: OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure> =
        fixture.rig.client.submit_async(&fixture.rig.scope, ordinary, |_| {
            std::future::pending::<Result<CaptureAttachLaunchPin, CaptureAttachFailure>>()
        })?;
    let error = commit_admitted(source.clone(), fixture.rig.scope.owner_id(), foreign_job)
        .err()
        .ok_or("Foreign actual job rebound a retired source")?;
    let CaptureBootstrapRejection::Association { cause, mut job } = *error else {
        return Err("Admitted job was discarded".into());
    };
    assert_eq!(job.spec().id, JobId::new(5)?);
    assert!(std::error::Error::source(&cause).is_some());
    assert!(matches!(fixture.rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    drop(job);
    drop(source);
    let mut later = expected;
    later = CaptureAttachExpectation::new(
        later.owner(),
        later.target(),
        &BuildIdentity::new("test-build")?,
        later.creator(),
        later.handler(),
        CaptureBudget::new(Moment::from_nanos(1))?,
    )?;
    let error = fixture
        .capture
        .request_attach_bootstrap(
            &fixture.rig.client,
            &fixture.rig.native_scope,
            attach_spec(6)?,
            later,
        )
        .err()
        .ok_or("Retired full run reopened with a renewed budget")?;
    assert!(matches!(*error, CaptureBootstrapRejection::Source(_)));
    assert_eq!(fixture.port.endpoint_reads.load(Ordering::Acquire), 1);
    drop(error);
    fixture.finish()
}

/// Actual endpoint acquisition keeps its retained loan while external work reenters Core.
#[test]
fn registered_endpoint_observation_releases_lifetime_and_source_guards() -> TestResult {
    let fixture = Fixture::new()?;
    let shared = Arc::downgrade(fixture.capture.shared.as_ref().ok_or("Missing actual owner")?);
    let scope = fixture.rig.scope.clone();
    let called = Arc::new(AtomicUsize::new(0));
    let observed = called.clone();
    *lock(&fixture.port.endpoint_probe) = Some(Box::new(move || {
        let shared = shared.upgrade().ok_or_else(|| {
            CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation)
        })?;
        // These exact exclusive owners have no competing caller in this fixture.
        // A locked result detects the old call topology without hanging on reentry.
        let native = shared.native.try_lock().map_err(|_| {
            CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation)
        })?;
        assert!(native.borrowed && native.lease.is_none());
        drop(native);
        let attempts = shared.attempts.try_lock().map_err(|_| {
            CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation)
        })?;
        drop(attempts);
        assert_eq!(scope.snapshot().reserved_bytes, crate::input::STORAGE_BYTES as u64 + 4096);
        observed.fetch_add(1, Ordering::Release);
        Ok(())
    }));
    let expected = fixture.expected()?;
    let mut job = fixture
        .capture
        .request_attach_bootstrap(
            &fixture.rig.client,
            &fixture.rig.native_scope,
            attach_spec(4)?,
            expected,
        )
        .map_err(|cause| format!("{cause:?}"))?;
    assert!(matches!(
        fixture.rig.completion(&mut job)?.outcome,
        JobOutcome::Failure(CaptureAttachFailure::Source(_))
    ));
    assert_eq!(called.load(Ordering::Acquire), 1);
    drop(job);
    fixture.finish()
}
