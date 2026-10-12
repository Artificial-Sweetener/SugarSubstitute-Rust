// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify data rejection and unchanged source custody before any acquisition.
use super::{test_support::*, types::*, *};
use crate::runtime_test_support::{Rig, TestResult};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::*;

#[test]
fn endpoint_shape_and_exact_identity_are_validated_without_native_authority() -> TestResult {
    let expected = expected()?;
    let local: Vec<u16> = r"\\.\pipe\actual_1".encode_utf16().collect();
    let observation = CaptureEndpointObservation::new(
        &expected,
        CaptureEndpointProvenance::CreatorInherited,
        expected.creator(),
        expected.handler(),
        &local,
    )?;
    assert_eq!(observation.endpoint_utf16(), local);
    assert!(!format!("{observation:?}").contains("actual_1"));
    assert!(
        CaptureEndpointObservation::new(
            &expected,
            CaptureEndpointProvenance::CreatorInherited,
            expected.handler(),
            expected.creator(),
            &local
        )
        .is_err()
    );
    for value in
        [r"\\server\pipe\foreign", r"\\.\pipe\", r"\\.\pipe\bad\nested", r"\\.\pipe\bad value"]
    {
        let units: Vec<u16> = value.encode_utf16().collect();
        assert!(
            CaptureEndpointObservation::new(
                &expected,
                CaptureEndpointProvenance::CreatorInherited,
                expected.creator(),
                expected.handler(),
                &units
            )
            .is_err()
        );
    }
    let mut oversized = local;
    oversized.resize(257, 97);
    assert!(
        CaptureEndpointObservation::new(
            &expected,
            CaptureEndpointProvenance::CreatorInherited,
            expected.creator(),
            expected.handler(),
            &oversized
        )
        .is_err()
    );
    assert!(CaptureProcessFacts::new(0, 1).is_err());
    assert!(CaptureProcessFacts::new(1, 0).is_err());
    Ok(())
}
#[test]
fn admission_rejection_returns_exact_source_before_observation_and_native_effects() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let source: Arc<dyn CaptureEndpointSource> = boundary.clone();
    let mut request = attach_spec(1)?;
    request.deadline = Some(Moment::from_nanos(1));
    let rejection = admission::reserve(
        &rig.client.core,
        &scope,
        request,
        expected()?,
        source.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .err()
    .ok_or("Conflicting deadline admitted")?;
    assert!(matches!(rejection.error, admission::CaptureAttachAdmissionError::ConflictingDeadline));
    assert!(Arc::ptr_eq(&rejection.source, &source));
    request.deadline = None;
    request.reservation_bytes = 16_383;
    let rejection = admission::reserve(
        &rig.client.core,
        &scope,
        request,
        expected()?,
        source.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .err()
    .ok_or("Undersized reservation admitted")?;
    assert!(matches!(
        rejection.error,
        admission::CaptureAttachAdmissionError::Admission(
            AdmissionError::NativeReservationTooSmall { minimum_bytes: 16_384, .. }
        )
    ));
    assert!(Arc::ptr_eq(&rejection.source, &source));
    assert_eq!(boundary.observations.load(Ordering::Acquire), 0);
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
