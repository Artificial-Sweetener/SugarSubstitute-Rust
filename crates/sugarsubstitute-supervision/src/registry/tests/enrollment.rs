// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify finite creation ordering through real execution and a fake native boundary.
//! These tests establish no native bootstrap, credential or pipe authentication proof.
use super::*;

#[test]
fn desktop_creation_binds_watch_then_enrolls_before_resume() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    assert_eq!(*lock(&h.fake.creation_order), [1, 2, 3, 4]);
    assert!(h.registry.snapshot(key)?.started);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}

#[test]
fn enrollment_rejection_retains_contained_family_without_resume_or_settlement() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.fail_enrollment.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    assert_eq!(*lock(&h.fake.creation_order), [1, 2, 3, 0]);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.started);
    assert!(!h.registry.snapshot(key)?.settled);
    let index = h.registry.index(key)?;
    assert_eq!(
        h.registry.records[index]
            .as_ref()
            .and_then(|record| record.error.as_ref())
            .map(RegistryError::stage),
        Some("lifecycle_enrollment_rejected")
    );
    assert!(
        lock(&h.registry.records[index].as_ref().ok_or("Missing family")?.shared.native)
            .family
            .is_some()
    );
    Ok(())
}

#[test]
fn cancellation_during_enrollment_prevents_resume_after_actual_boundary_return() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.hold_enrollment.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.fake.entered_enrollment()?;
    h.scope.cancel();
    h.fake.release();
    h.created(key)?;
    assert_eq!(*lock(&h.fake.creation_order), [1, 2, 3, 0]);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.started);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}

#[test]
fn failed_root_duplicate_never_reaches_enrollment_or_resume() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.fail_duplicate.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    assert_eq!(*lock(&h.fake.creation_order), [1, 0, 0, 0]);
    assert!(!h.fake.enrollment_entered.load(Ordering::Acquire));
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}
