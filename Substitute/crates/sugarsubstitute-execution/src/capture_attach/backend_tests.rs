// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind genuine backend targets into the existing original capture/Core ownership.
use super::{test_support::*, *};
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    crash::CaptureTarget,
    execution::*,
    incident::BuildIdentity,
};
fn expected_backend(session: u64) -> TestResult<types::CaptureAttachExpectation> {
    let base = expected()?;
    Ok(types::CaptureAttachExpectation::new(
        base.owner(),
        CaptureTarget::BackendCustody(BackendCustodyKey::new(
            base.owner(),
            BackendCustodySession::new(session)?,
        )),
        &BuildIdentity::new("test-build")?,
        base.creator(),
        base.handler(),
        base.attach_budget(),
    )?)
}
fn submit_backend(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    id: u64,
    session: u64,
) -> TestResult<OwnedJob<CaptureAttachLaunchPin, types::CaptureAttachFailure>> {
    admission::reserve(
        &rig.client.core_for_test(),
        scope,
        attach_spec(id)?,
        expected_backend(session)?,
        boundary.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .map_err(|rejected| Box::new(rejected.error) as Box<dyn std::error::Error>)
}
#[test]
fn backend_target_uses_same_core_pin_custody_and_blocks_duplicates_after_ack() -> TestResult {
    let rig = Rig::new()?;
    let first = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core_for_test(), &boundary);
    let mut job = submit_backend(&rig, &first, &boundary, 1, 9)?;
    boundary.started.wait()?;
    boundary.ready.store(true, Ordering::Release);
    boundary.signal()?;
    let pin = take_pin(&rig, &mut job)?;
    assert_eq!(pin.target(), expected_backend(9)?.target());
    assert_eq!(first.snapshot().reserved_bytes, 16384);
    let rejected = admission::reserve(
        &rig.client.core_for_test(),
        &first,
        attach_spec(2)?,
        expected_backend(9)?,
        boundary.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .err()
    .ok_or("Duplicate backend admitted")?;
    assert!(
        matches!(rejected.error,admission::CaptureAttachAdmissionError::TargetAlreadyReserved(target) if target==pin.target())
    );
    let mut retirement = pin.release();
    boundary.settle()?;
    assert_eq!(
        take_retirement_report(&rig, &mut retirement)??.target(),
        expected_backend(9)?.target()
    );
    Ok(())
}
#[test]
fn backend_start_borrows_actual_target_before_move_and_preserves_pin_on_rejection() -> TestResult {
    use crate::windows_backend_custody::CapturedBackendCustodyStart;
    let rig = Rig::new()?;
    let first = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core_for_test(), &boundary);
    let mut job = submit_backend(&rig, &first, &boundary, 1, 9)?;
    boundary.started.wait()?;
    boundary.ready.store(true, Ordering::Release);
    boundary.signal()?;
    let mut pin = Some(take_pin(&rig, &mut job)?);
    let key = expected_backend(9)?.target().backend_custody().ok_or("Backend key absent")?;
    let wrong = BackendCustodyKey::new(key.owner(), BackendCustodySession::new(10)?);
    assert!(CapturedBackendCustodyStart::new(wrong, &mut pin).is_err());
    assert!(pin.is_some());
    let mut start = CapturedBackendCustodyStart::new(key, &mut pin)?;
    assert!(pin.is_none());
    assert!(!start.is_consumed());
    let native =
        rig.client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut request = spec(2, JobClass::NativeObservation, 196608)?;
    request.operation = Operation::NativeLifecycle;
    let before = native.snapshot();
    let mut invalid = request;
    invalid.reservation_bytes += 1;
    assert!(matches!(
        rig.client.reserve_captured_backend_custody_pipe_server(
            &native,
            invalid,
            key,
            Moment::from_nanos(1_000_000_000),
            &mut start
        ),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert!(!start.is_consumed());
    assert_eq!(before, native.snapshot());
    assert!(
        rig.client
            .reserve_captured_backend_custody_pipe_server(
                &native,
                request,
                wrong,
                Moment::from_nanos(1_000_000_000),
                &mut start
            )
            .is_err()
    );
    assert!(!start.is_consumed());
    assert_eq!(before, native.snapshot());
    rig.clock.advance(1_000_000_000);
    assert!(matches!(
        rig.client.reserve_captured_backend_custody_pipe_server(
            &native,
            request,
            key,
            Moment::from_nanos(1_000_000_000),
            &mut start
        ),
        Err(AdmissionError::DeadlineExpired { .. })
    ));
    assert!(!start.is_consumed());
    assert_eq!(before, native.snapshot());
    let mut retirement = start.release().ok_or("Original backend pin was lost")?;
    assert!(start.is_consumed());
    assert!(start.release().is_none());
    assert_eq!(first.snapshot().reserved_bytes, 16384);
    boundary.settle()?;
    let actual = take_retirement_report(&rig, &mut retirement)??;
    assert_eq!(actual.target(), CaptureTarget::BackendCustody(key));
    assert_eq!(actual.execution_id(), first.owner_id());
    assert_eq!(actual.job_id(), JobId::new(1)?);
    Ok(())
}
