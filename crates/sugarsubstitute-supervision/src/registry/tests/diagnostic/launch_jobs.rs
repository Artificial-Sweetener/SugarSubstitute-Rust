// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Genuine Reporter bootstrap/enrollment survive the exact former Host job-order rejection.
use super::*;

#[test]
fn reporter_creation_before_watch_rejects_without_effects_and_same_pins_admit_in_real_order()
-> TestResult {
    let mut h = Harness::new()?;
    let scope = pipe_scope(&mut h)?;
    let key = key(DiagnosticRole::Reporter)?;
    let (session, bootstrap, enrollment) = pins(&mut h, key, &scope)?;
    let original = bootstrap.borrow_inheritance().section.as_raw_handle();
    let mut input = assembled(bootstrap, enrollment)?;
    let required = input.as_ref().ok_or("Original input absent")?.required_reservation_bytes();
    let creation = h.spec(JobClass::InfrastructureLifecycle, required)?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    h.next_scope += 1;
    assert!(matches!(
        h.registry.submit_diagnostic_launch(
            &h.client, &h.scope, native, creation, watch, key, &mut input
        ),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let retained = input
        .as_ref()
        .and_then(|value| value.diagnostic.as_ref())
        .ok_or("Original diagnostic input lost")?;
    assert_eq!(retained.bootstrap.borrow_inheritance().section.as_raw_handle(), original);
    assert!(retained.validates(key));
    assert!(h.registry.is_empty());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    let release = Release(h.fake.clone());
    h.fake.hold_enrollment.store(true, Ordering::Release);
    let process = submit(&mut h, key, &mut input)?.process();
    assert!(input.is_none());
    h.fake.entered_enrollment()?;
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 1);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert_eq!(*lock(&h.fake.creation_order), [1, 2, 3, 0]);
    let inherited = *lock(&h.fake.inherited);
    assert_eq!(inherited.1, 1);
    assert_eq!(inherited.0[0], original as usize);
    assert!(!inherited.3);
    h.stop(process)?;
    h.fake.release();
    h.created(process)?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(process, ROOT, true, 0)?;
    h.cleanup(process)?;
    session.session.control().close();
    drop(session);
    assert!(h.registry.snapshot(process)?.settled);
    drop(release);
    Ok(())
}
