// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject both captured purpose substitutions using genuine original capture and Entry owners.
use super::*;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    crash::CaptureTarget,
};
fn reject_substitution(backend_pin: bool) -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_entry_scope, mut session) = server(&rig, &boundary)?;
    let startup = snapshot(&rig, &boundary, &session)?;
    let key = BackendCustodyKey::new(run()?.owner, BackendCustodySession::new(9)?);
    let actual = if backend_pin {
        CaptureTarget::BackendCustody(key)
    } else {
        CaptureTarget::Desktop(run()?)
    };
    let (_capture_scope, capture_job, pin, release) =
        crate::capture_attach::test_support::admitted_pin(&rig, actual)?;
    let original = Moment::from_nanos(25_000_000_000);
    let binding = if backend_pin {
        AdmissionBinding::project(run()?, original, &pin, startup.admission_binding()?)
    } else {
        AdmissionBinding::backend(key, original, &pin)
    };
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(4)?, deadline: None },
        None,
        true,
    )?;
    let mut request = spec(9, JobClass::NativeObservation, binding.reservation_bytes())?;
    request.operation = Operation::NativeLifecycle;
    let before = scope.snapshot();
    let calls = AtomicUsize::new(0);
    let rejected = rig.client.core.admit_owned(
        &scope,
        request,
        RequestKind::CapturedLifecycle(&binding),
        (),
        |(), _, _, _| {
            calls.fetch_add(1, Ordering::AcqRel);
            Err::<(QueuedJob, ()), _>((AdmissionError::InvalidCaptureAdmission, ()))
        },
    );
    assert!(matches!(rejected, Err((AdmissionError::InvalidCaptureAdmission, ()))));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot(), before);
    assert_eq!(pin.target(), actual);
    let mut retirement = pin.release();
    drop(release);
    assert_eq!(
        crate::capture_attach::test_support::take_retirement_report(&rig, &mut retirement)??
            .target(),
        actual
    );
    drop(capture_job);
    drop(startup);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn backend_capture_cannot_be_presented_as_desktop_even_with_its_exact_desktop_size() -> TestResult {
    reject_substitution(true)
}
#[test]
fn desktop_capture_cannot_be_presented_as_backend_even_with_its_exact_backend_size() -> TestResult {
    reject_substitution(false)
}
