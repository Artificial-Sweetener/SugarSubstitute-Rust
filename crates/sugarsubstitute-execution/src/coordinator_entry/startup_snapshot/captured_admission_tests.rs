// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence genuine capture and Entry owners before a new captured session is constructed.
use super::kernel::*;
use crate::{
    admission::RequestKind,
    coordinator_entry::*,
    lifecycle_pipe::captured_admission::AdmissionBinding,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
    task::QueuedJob,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobOutcome, Moment, Operation, ScopeId, ScopeSpec,
};

fn snapshot(
    rig: &Rig,
    boundary: &Arc<Boundary>,
    session: &CoordinatorEntryServerSession,
) -> TestResult<PreparedStartupSnapshot> {
    activate(rig, session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { request, .. } =
        fact(rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Missing genuine authenticated Entry request".into());
    };
    request.into_startup_snapshot().map_err(|failure| failure.error.into())
}

#[test]
fn captured_parent_rejects_closed_receiving_ancestor_before_constructor_and_id_use() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_entry_scope, mut session) = server(&rig, &boundary)?;
    let startup = snapshot(&rig, &boundary, &session)?;
    let mut retirement = None;
    let (_capture_scope, _capture_job, pin, release) =
        crate::capture_attach::test_support::admitted_pin(&rig, run()?)?;
    retirement.replace(release);
    let binding = AdmissionBinding::project(
        run()?,
        Moment::from_nanos(25_000_000_000),
        &pin,
        startup.admission_binding()?,
    );
    let core = &rig.client.core;
    let parent = core.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
    let receiving = core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(5)?, deadline: None },
        Some(&parent),
        true,
    )?;
    let open =
        core.create_scope_kind(ScopeSpec { id: ScopeId::new(6)?, deadline: None }, None, true)?;
    core.close_scope(parent.id());
    assert!(!parent.snapshot().cancellation_requested);
    assert!(!receiving.snapshot().closed);
    assert!(!receiving.snapshot().cancellation_requested);
    let calls = AtomicUsize::new(0);
    let mut request = spec(9, JobClass::NativeObservation, 262_144)?;
    request.operation = Operation::NativeLifecycle;
    let rejected = core.admit_owned(
        &receiving,
        request,
        RequestKind::CapturedLifecycle(&binding),
        (),
        |input, _, _, _| {
            calls.fetch_add(1, Ordering::AcqRel);
            Err::<(QueuedJob, ()), _>((AdmissionError::InvalidCaptureAdmission, input))
        },
    );
    assert!(
        matches!(rejected, Err((AdmissionError::ScopeClosed { scope_id }, ())) if scope_id == parent.id())
    );
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(receiving.snapshot().reserved_bytes, 0);
    assert_eq!(receiving.snapshot().queued, 0);
    let unchanged_id = core.admit_owned(
        &open,
        request,
        RequestKind::CapturedLifecycle(&binding),
        (),
        |input, context, _, timing| {
            calls.fetch_add(1, Ordering::AcqRel);
            assert_eq!(context.deadline(), None);
            assert_eq!(timing.native_handshake, Some(Moment::from_nanos(25_000_000_000)));
            Err::<(QueuedJob, ()), _>((AdmissionError::InvalidCaptureAdmission, input))
        },
    );
    assert!(matches!(unchanged_id, Err((AdmissionError::InvalidCaptureAdmission, ()))));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(open.snapshot().reserved_bytes, 0);
    drop(startup);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    drop(pin);
    drop(retirement);
    Ok(())
}

#[test]
fn captured_parent_constructor_gets_narrowed_receiving_cutoff_with_persistent_deadline_none()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_entry_scope, mut session) = server(&rig, &boundary)?;
    let startup = snapshot(&rig, &boundary, &session)?;
    let mut retirement = None;
    let (_capture_scope, _capture_job, pin, release) =
        crate::capture_attach::test_support::admitted_pin(&rig, run()?)?;
    retirement.replace(release);
    let binding = AdmissionBinding::project(
        run()?,
        Moment::from_nanos(25_000_000_000),
        &pin,
        startup.admission_binding()?,
    );
    let core = &rig.client.core;
    let parent = core.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
    let receiving = core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(5)?, deadline: None },
        Some(&parent),
        true,
    )?;
    let cutoff = Moment::from_nanos(4_000_000_000);
    lock(&core.state).scopes.get_mut(&parent.id()).ok_or("Missing actual parent scope")?.deadline =
        Some(cutoff);
    let calls = AtomicUsize::new(0);
    let mut request = spec(9, JobClass::NativeObservation, 262_144)?;
    request.operation = Operation::NativeLifecycle;
    let returned = core.admit_owned(
        &receiving,
        request,
        RequestKind::CapturedLifecycle(&binding),
        (),
        |input, context, _, timing| {
            calls.fetch_add(1, Ordering::AcqRel);
            assert_eq!(context.deadline(), None);
            assert_eq!(context.spec().deadline, None);
            assert_eq!(timing.native_handshake, Some(cutoff));
            Err::<(QueuedJob, ()), _>((AdmissionError::InvalidCaptureAdmission, input))
        },
    );
    assert!(matches!(returned, Err((AdmissionError::InvalidCaptureAdmission, ()))));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(receiving.spec().deadline, None);
    assert_eq!(receiving.snapshot().reserved_bytes, 0);
    drop(startup);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    drop(pin);
    drop(retirement);
    Ok(())
}
