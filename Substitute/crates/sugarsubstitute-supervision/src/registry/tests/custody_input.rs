// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve genuine admitted bootstrap owners when captured-helper composition rejects.
use super::*;
use crate::RegistryInputError;
use std::os::windows::io::AsRawHandle;
use sugarsubstitute_contracts::backend_custody::{BackendCustodyKey, BackendCustodySession};
use sugarsubstitute_execution::windows_backend_custody::{
    BackendCustodyBootstrap, BackendCustodyEnrollment, BackendCustodyPipeObservation,
    BackendCustodyPipeSession, MIN_LIFECYCLE_PIPE_BYTES,
};
/// Reserve an actual isolated native transport and take its once-only prepared pair.
fn prepared(
    h: &mut Harness,
) -> Result<
    (BackendCustodyPipeSession, Scope, BackendCustodyBootstrap, BackendCustodyEnrollment),
    Box<dyn Error>,
> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| value.checked_add(1))
        .map_err(|_| "Custody fixture identities exhausted")?;
    let key = BackendCustodyKey::new(
        owner()?,
        BackendCustodySession::new((u64::from(std::process::id()) << 32) | sequence)?,
    );
    let scope = h
        .client
        .create_native_scope(ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None }, None)?;
    h.next_scope += 1;
    let spec = h.spec(JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    let mut session = h.client.reserve_backend_custody_pipe_server(
        &scope,
        spec,
        key,
        Moment::from_nanos(60_000_000_000),
    )?;
    loop {
        let revision = *lock(&h.wake.count);
        h.registry.consume(&h.client.drain_completions(std::num::NonZeroUsize::MIN)?)?;
        if session
            .drain(std::num::NonZeroUsize::MIN)
            .into_iter()
            .any(|fact| matches!(fact, BackendCustodyPipeObservation::Prepared))
        {
            let (bootstrap, enrollment) =
                session.take_bootstrap()?.ok_or("Prepared custody pair missing")?;
            return Ok((session, scope, bootstrap, enrollment));
        }
        if session.take_terminal().is_some() {
            return Err("Custody transport retired before prepared pins".into());
        }
        h.wake.wait(revision)?;
    }
}
#[test]
fn captured_constructor_rejects_real_ordinary_pair_without_consuming_either_owner() -> TestResult {
    let mut h = Harness::new()?;
    let (_session, _scope, bootstrap, enrollment) = prepared(&mut h)?;
    let key = bootstrap.key();
    let inherited = bootstrap.borrow_inheritance().section.as_raw_handle();
    let (mut bootstrap, mut enrollment) = (Some(bootstrap), Some(enrollment));
    let mut launch = input()?;
    assert!(matches!(
        launch.with_backend_custody_capture(&mut bootstrap, &mut enrollment),
        Err(RegistryInputError::BackendCustodyProvenance)
    ));
    assert!(launch.backend_custody.is_none());
    assert_eq!(
        bootstrap.as_ref().ok_or("Bootstrap moved")?.borrow_inheritance().section.as_raw_handle(),
        inherited
    );
    assert_eq!(enrollment.as_ref().ok_or("Enrollment moved")?.key(), key);
    launch.with_backend_custody_pipe(&mut bootstrap, &mut enrollment)?;
    assert!(bootstrap.is_none() && enrollment.is_none());
    assert!(launch.backend_custody.as_ref().is_some_and(|owner| owner.validates(key)));
    Ok(())
}

#[test]
fn actual_backend_bootstrap_projects_original_section_through_family_inheritance() -> TestResult {
    let mut h = Harness::new()?;
    let (_session, _scope, bootstrap, enrollment) = prepared(&mut h)?;
    let key = bootstrap.key();
    let section = bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
    let mut launch = input()?;
    launch.with_backend_custody_pipe(&mut Some(bootstrap), &mut Some(enrollment))?;
    let ticket = submit(&mut h, key, launch)?;
    h.created(ticket.process())?;
    let inherited = *lock(&h.fake.inherited);
    assert_eq!(inherited.0, [section, 0, 0, 0, 0, 0]);
    assert_eq!(inherited.1, 1);
    assert_eq!(inherited.2, [0; 3]);
    assert!(!inherited.3);
    Ok(())
}
#[test]
fn foreign_real_enrollment_rejection_preserves_both_original_session_allocations() -> TestResult {
    let mut h = Harness::new()?;
    let (_first, _first_scope, bootstrap, original_enrollment) = prepared(&mut h)?;
    let (_second, _second_scope, second_bootstrap, enrollment) = prepared(&mut h)?;
    let (mut bootstrap, mut enrollment) = (Some(bootstrap), Some(enrollment));
    let mut launch = input()?;
    assert!(matches!(
        launch.with_backend_custody_capture(&mut bootstrap, &mut enrollment),
        Err(RegistryInputError::BackendCustodyProvenance)
    ));
    assert!(matches!(
        launch.with_backend_custody_pipe(&mut bootstrap, &mut enrollment),
        Err(RegistryInputError::BackendCustodyProvenance)
    ));
    assert!(bootstrap.as_ref().is_some_and(|owner| owner.matches_enrollment(&original_enrollment)));
    assert!(enrollment.as_ref().is_some_and(|owner| second_bootstrap.matches_enrollment(owner)));
    assert!(launch.backend_custody.is_none());
    Ok(())
}

#[test]
fn original_helper_exit_rejects_pending_establishment_without_clock_or_retirement() -> TestResult {
    let mut h = Harness::new()?;
    let (_session, _scope, bootstrap, enrollment) = prepared(&mut h)?;
    let key = bootstrap.key();
    let mut launch = input()?;
    launch.with_backend_custody_pipe(&mut Some(bootstrap), &mut Some(enrollment))?;
    let ticket = submit(&mut h, key, launch)?;
    h.created(ticket.process())?;
    let index = h.registry.index(ticket.process())?;
    super::super::backend_custody::helper_root_alive(
        h.registry.records[index].as_ref().ok_or("Original record absent")?.root_exit,
    )?;
    let alien =
        BackendCustodyKey::new(key.owner(), BackendCustodySession::new(key.session().get() + 1)?);
    assert!(h.registry.backend_custody_launch_pending(ticket.process(), alien).is_err());
    h.exit(ticket.process(), ROOT, true, 41)?;
    let before = h.registry.snapshot(ticket.process())?;
    let failure = super::super::backend_custody::helper_root_alive(
        h.registry.records[index].as_ref().ok_or("Original record absent")?.root_exit,
    )
    .err()
    .ok_or("Exited helper remained healthy pending")?;
    assert_eq!(failure.stage(), "custody_helper_exited");
    assert_eq!(
        failure.source().ok_or("Original exit fact absent")?.to_string(),
        "Actual owned custody helper exited with code 41"
    );
    assert_eq!(h.registry.snapshot(ticket.process())?, before);
    assert_eq!(h.clock.now(), Moment::from_nanos(0));
    assert!(before.shutdown.is_none());
    assert!(!before.settled);
    let retained = h
        .registry
        .backend_custody_launch_pending(ticket.process(), key)
        .err()
        .ok_or("Faulted/exited helper remained healthy pending")?;
    let original = h.registry.records[index]
        .as_ref()
        .and_then(|record| record.error.as_ref())
        .ok_or("Original simulated-watch cancellation failure missing")?;
    assert_eq!(retained.stage(), original.stage());
    assert!(std::ptr::eq(
        retained.source().ok_or("Original cause missing")?,
        original.source().ok_or("Original cause missing")?
    ));
    Ok(())
}

/// Admit the original helper family through the real registry and controlled native boundary.
fn submit(
    h: &mut Harness,
    key: BackendCustodyKey,
    launch: WindowsLaunchInput,
) -> Result<LaunchTicket, Box<dyn Error>> {
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let creation =
        h.spec(JobClass::InfrastructureLifecycle, launch.required_reservation_bytes())?;
    let scope = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    h.next_scope += 1;
    Ok(h.registry.submit_backend_custody_launch(
        &h.client,
        &h.scope,
        scope,
        creation,
        watch,
        key,
        &mut Some(launch),
    )?)
}

#[test]
fn pending_helper_launch_preserves_original_native_creation_failure() -> TestResult {
    let mut h = Harness::new()?;
    let (_session, _scope, bootstrap, enrollment) = prepared(&mut h)?;
    let key = bootstrap.key();
    let mut launch = input()?;
    launch.with_backend_custody_pipe(&mut Some(bootstrap), &mut Some(enrollment))?;
    h.fake.fail_duplicate.store(true, Ordering::Release);
    let ticket = submit(&mut h, key, launch)?;
    h.created(ticket.process())?;
    let index = h.registry.index(ticket.process())?;
    let original = h.registry.records[index]
        .as_ref()
        .and_then(|record| record.error.as_ref())
        .ok_or("Original native failure absent")?;
    let failure = h
        .registry
        .backend_custody_launch_pending(ticket.process(), key)
        .err()
        .ok_or("Failed helper remained healthy pending")?;
    assert_eq!(failure.stage(), original.stage());
    assert!(std::ptr::eq(
        failure.source().ok_or("Original cause absent")?,
        original.source().ok_or("Original cause absent")?
    ));
    assert_eq!(failure.win32_code(), Some(6));
    assert_eq!(h.clock.now(), Moment::from_nanos(0));
    Ok(())
}
