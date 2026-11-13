// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove actual accepted capture provenance remains with the original session allocation.
use super::*;
use crate::{admission::RequestKind, lifecycle_pipe::captured_admission::AdmissionBinding};
use sugarsubstitute_contracts::crash::CaptureTarget;

#[test]
fn captured_backend_bootstrap_uses_original_allocation_and_preserves_ordinary_pair() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut ordinary = session(&rig, &scope, &boundary, true)?;
    drop(fact(&rig, &mut ordinary, |fact| matches!(fact, Fact::Prepared))?);
    let (ordinary_bootstrap, ordinary_enrollment) =
        ordinary.take_bootstrap()?.ok_or("Original ordinary bootstrap missing")?;
    assert!(!ordinary.control.inner.shared.captured);
    assert!(!ordinary_bootstrap.captures(key()?));
    assert!(ordinary_bootstrap.matches_enrollment(&ordinary_enrollment));

    let target = CaptureTarget::BackendCustody(key()?);
    let (_capture_scope, capture_job, pin, release) =
        crate::capture_attach::test_support::admitted_pin(&rig, target)?;
    let binding = AdmissionBinding::backend(key()?, Moment::from_nanos(1000), &pin);
    let captured_boundary = Boundary::new();
    *lock(&captured_boundary.binding) = Some(PeerBinding::BackendCustody(key()?));
    let mut request = spec(6, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    let mut wrong = request;
    wrong.reservation_bytes = MIN_CAPTURED_LIFECYCLE_PIPE_BYTES;
    let before = scope.snapshot();
    let mut called = false;
    let rejected = crate::lifecycle_pipe::admit_owned(
        &rig.client.core,
        &scope,
        wrong,
        crate::lifecycle_pipe::PipeAdmission {
            binding: Some(PeerBinding::BackendCustody(key()?)),
            handshake: Moment::from_nanos(1000),
            owner: Owner::BackendCustody,
        },
        RequestKind::CapturedLifecycle(&binding),
        (),
        |(), _| {
            called = true;
            Err::<crate::lifecycle_pipe::PreparedPipeLease, _>((
                AdmissionError::InvalidCaptureAdmission,
                (),
            ))
        },
    );
    assert!(matches!(rejected, Err((AdmissionError::InvalidCaptureAdmission, ()))));
    assert!(!called);
    assert_eq!(scope.snapshot(), before);
    let (inner, completion) = crate::lifecycle_pipe::admit_owned(
        &rig.client.core,
        &scope,
        request,
        crate::lifecycle_pipe::PipeAdmission {
            binding: Some(PeerBinding::BackendCustody(key()?)),
            handshake: Moment::from_nanos(1000),
            owner: Owner::BackendCustody,
        },
        RequestKind::CapturedLifecycle(&binding),
        (),
        |(), _| {
            Ok(crate::lifecycle_pipe::PreparedPipeLease {
                lease: Box::new(Lease(captured_boundary.clone())),
                metadata: None,
            })
        },
    )
    .map_err(|(cause, ())| cause)?;
    let mut captured =
        BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion };
    drop(fact(&rig, &mut captured, |fact| matches!(fact, Fact::Prepared))?);
    let (captured_bootstrap, captured_enrollment) =
        captured.take_bootstrap()?.ok_or("Original captured bootstrap missing")?;
    assert!(captured.control.inner.shared.captured);
    assert!(captured_bootstrap.captures(key()?));
    assert!(
        !captured_bootstrap
            .captures(BackendCustodyKey::new(key()?.owner(), BackendCustodySession::new(10)?))
    );
    assert!(captured_bootstrap.matches_enrollment(&captured_enrollment));
    assert!(!captured_bootstrap.matches_enrollment(&ordinary_enrollment));
    assert!(!ordinary_bootstrap.matches_enrollment(&captured_enrollment));
    captured.control.close();
    ordinary.control.close();
    drop((captured_bootstrap, captured_enrollment, ordinary_bootstrap, ordinary_enrollment));
    assert!(matches!(terminal(&rig, &mut captured)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(terminal(&rig, &mut ordinary)?.outcome, JobOutcome::Cancelled));
    drop((captured, ordinary));
    let mut retirement = pin.release();
    drop(release);
    let report =
        crate::capture_attach::test_support::take_retirement_report(&rig, &mut retirement)??;
    assert_eq!(report.target(), target);
    drop(capture_job);
    assert_eq!(scope.snapshot().reserved_bytes, 2 * MIN_LIFECYCLE_PIPE_BYTES);
    drop((boundary, captured_boundary));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
