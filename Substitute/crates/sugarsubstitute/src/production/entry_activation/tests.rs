// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real empty-owner admission and shutdown without native effects.
use super::*;
use sugarsubstitute_contracts::execution::Moment;
fn owner() -> Result<Box<ProductionActivation>> {
    ProductionActivation::new(
        r"E:\application\SugarSubstitute.exe".into(),
        r"E:\application".into(),
        r"D:\captures".into(),
    )
}
#[test]
fn activation_rejects_ambiguous_locations_before_creating_native_owners() {
    assert!(
        ProductionActivation::new(
            "relative.exe".into(),
            r"E:\application".into(),
            r"D:\captures".into(),
        )
        .is_err()
    );
}
#[test]
fn activation_empty_owner_requires_explicit_close_before_finish() -> Result<()> {
    let mut owned: Box<dyn EntryActivation> = owner()?;
    assert!(owned.finish().is_err());
    owned.request_close(None)?;
    assert!(owned.poll_close(
        &CompletionBatch { notifications: Vec::new(), remaining: 0 },
        None,
        ShutdownBudget::new(Moment::from_nanos(0))?
    )?);
    owned.finish()?;
    Ok(())
}
#[test]
fn activation_shutdown_keeps_first_cutoffs_across_later_requests() -> Result<()> {
    let mut owned: Box<dyn EntryActivation> = owner()?;
    let first = ShutdownBudget::new(Moment::from_nanos(20))?;
    let later = ShutdownBudget::new(Moment::from_nanos(30))?;
    owned.request_close(Some(first))?;
    owned.request_close(Some(later))?;
    assert_eq!(owned.shutdown_budget(), Some(first));
    let earlier = ShutdownBudget::new(Moment::from_nanos(10))?;
    owned.request_close(Some(earlier))?;
    assert_eq!(owned.shutdown_budget(), Some(earlier));
    Ok(())
}

#[test]
fn boxed_activation_configuration_and_close_retain_original_owner_and_first_cause() -> Result<()> {
    let owned = owner()?;
    let original_owner = std::ptr::from_ref(owned.as_ref());
    let mut owned = owned.with_qualification(Default::default());
    assert_eq!(std::ptr::from_ref(owned.as_ref()), original_owner);
    let cause: Box<dyn Error> = Box::new(std::io::Error::other("Original activation cause"));
    let original_cause = std::ptr::from_ref(cause.as_ref());
    owned.preserve(cause);
    owned.preserve(Box::new(std::io::Error::other("Later activation cause")));
    let budget = ShutdownBudget::new(Moment::from_nanos(20))?;
    owned.request_close(Some(budget))?;
    assert_eq!(owned.shutdown_budget(), Some(budget));
    assert_eq!(std::ptr::from_ref(owned.as_ref()), original_owner);
    assert!(std::ptr::eq(owned.failure_source().ok_or("First cause absent")?, original_cause));
    assert!(owned.poll_close(
        &CompletionBatch { notifications: Vec::new(), remaining: 0 },
        None,
        budget
    )?);
    owned.finish()?;
    assert_eq!(std::ptr::from_ref(owned.as_ref()), original_owner);
    assert!(std::ptr::eq(owned.failure_source().ok_or("First cause lost")?, original_cause));
    Ok(())
}

/// "Restart to apply": the window tells the coordinator before it hides, then waits for the
/// coordinator's notice to cover the screen. The coordinator owes that notice, and the relaunch
/// after a normal close, from the live window's request, not only once its close completed:
/// owing it only afterwards left the window and the coordinator waiting on each other until the
/// window was cut off, and nothing relaunched.
#[test]
fn a_live_window_that_asked_to_restart_is_a_restart_before_its_close_completes() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::ConnectionId;
    let mut owned = owner()?;
    let retained = super::progress_deadline_tests::retained_record(Moment::from_nanos(1))?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        crate::production::parent_lifecycle::ParentLifecycle::storage_bytes(),
    )
    .map_err(outer)?;
    let mut parent = crate::production::parent_lifecycle::ParentLifecycle::new(
        retained.record.run,
        retained.record.launch,
        sugarsubstitute_contracts::configuration::ProfileId::new("Restart_1".into())?,
        ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?),
    )?;
    owned.records[0] = Some(retained);
    assert!(!EntryActivation::restart_to_apply_requested(owned.as_ref()));
    parent.observe_restart_to_apply();
    owned.records[0].as_mut().ok_or("record absent")?.record.parent = Some(Box::new(parent));
    assert!(
        EntryActivation::restart_to_apply_requested(owned.as_ref()),
        "the live window's request is the restart"
    );
    Ok(())
}
