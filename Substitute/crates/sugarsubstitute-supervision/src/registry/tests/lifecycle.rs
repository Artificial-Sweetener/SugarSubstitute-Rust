// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual cleanup proofs independently of root status, cancellation or timeout.
use super::*;

#[test]
fn root_exit_zero_or259_is_not_family_settlement_and_live_leaf_remains_owned() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 259)?;
    assert_eq!(h.registry.snapshot(key)?.root_exit, Some(259));
    assert!(!h.registry.snapshot(key)?.settled);
    h.stop(key)?;
    h.cleanup(key)?;
    assert_eq!(h.registry.snapshot(key)?.retained_members, 2);
    assert!(h.registry.snapshot(key)?.member_watch_required);
    assert!(!h.registry.snapshot(key)?.settled);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.settled);
    h.fake.leaf_alive.store(false, Ordering::Release);
    h.exit(key, LEAF, false, 0)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    assert!(matches!(h.registry.retire_settled(key), Err(RegistryAdmissionError::Busy)));
    let mut kinds = Vec::new();
    while let Some(observation) = h.registry.take_observation(key)? {
        kinds.push(match observation {
            RegistryObservation::ContainedStarted { .. } => 0,
            RegistryObservation::RootExited { code, .. } => {
                assert_eq!(code, 259);
                1
            }
            RegistryObservation::RootObservationRetired { .. } => 2,
            RegistryObservation::Failed { .. } => 3,
            RegistryObservation::FamilySettled { .. } => 4,
        });
    }
    assert_eq!(kinds, [0, 1, 2, 3, 4]);
    h.registry.retire_settled(key)?;
    assert!(h.registry.is_empty());
    assert!(matches!(h.registry.snapshot(key), Err(RegistryAdmissionError::Stale)));
    let next = h.launch(1)?;
    h.created(next.process())?;
    assert!(next.process().id > key.id);
    Ok(())
}

#[test]
fn cancellation_during_native_creation_keeps_containment_and_original_memory() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.hold_create.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.fake.entered()?;
    h.scope.cancel();
    h.stop(key)?;
    assert!(h.registry.snapshot(key)?.creation_pending);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    h.fake.release();
    h.created(key)?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    let cleanup_scope =
        h.client.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
    let old_scope = std::mem::replace(&mut h.scope, cleanup_scope);
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 37)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(old_scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn stop_linearization_prevents_late_resume_even_without_creation_scope_cancel() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.hold_create.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    h.fake.entered()?;
    h.stop(ticket.process())?;
    h.fake.release();
    h.created(ticket.process())?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(ticket.process())?.started);
    assert!(!h.registry.snapshot(ticket.process())?.settled);
    Ok(())
}

#[test]
fn drop_cancels_and_closes_owned_job_after_inflight_factory_returns_without_settlement()
-> TestResult {
    let mut h = Harness::new()?;
    h.fake.hold_create.store(true, Ordering::Release);
    let _ticket = h.launch(1)?;
    h.fake.entered()?;
    let replacement = h.construct_registry(owner()?)?;
    let old = std::mem::replace(&mut h.registry, replacement);
    drop(old);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 0);
    h.fake.release();
    h.fake.closed()?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    assert!(h.registry.is_empty());
    Ok(())
}

#[test]
fn grace_force_is_once_and_total_deadline_never_releases_uncertain_family() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    h.clock.0.store(10_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    h.clock.0.store(15_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    assert!(h.registry.snapshot(key)?.deadline_expired(h.clock.now()));
    assert!(!h.registry.snapshot(key)?.settled);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn seal_or_complete_inventory_failure_retains_first_cause_and_prevents_false_settlement()
-> TestResult {
    let mut h = Harness::new()?;
    h.fake.fail_seal.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    // A native-fake canceled observation is a separate failure; inspect the real seal cause below.
    let index = h.registry.index(key)?;
    if let Some(record) = &mut h.registry.records[index] {
        record.error = None;
        record.unread[3] = false;
    }
    h.stop(key)?;
    h.clock.0.store(10_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    let record = h.registry.records[index].as_ref().ok_or("Missing family")?;
    let error = record.error.as_ref().ok_or("Missing seal failure")?;
    assert_eq!(error.stage(), "seal_job_limits");
    assert!(!error.contains_owner_panic());
    assert_eq!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    h.fake.fail_seal.store(false, Ordering::Release);
    h.fake.fail_inventory.store(true, Ordering::Release);
    h.cleanup(key)?;
    assert_eq!(
        h.registry.records[index].as_ref().and_then(|r| r.error.as_ref()).map(RegistryError::stage),
        Some("seal_job_limits")
    );
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    Ok(())
}

#[test]
fn close_failure_keeps_native_storage_claim_and_late_success_preserves_original_budget()
-> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.stop(key)?;
    let budget = h.registry.snapshot(key)?.shutdown;
    h.fake.fail_close.store(true, Ordering::Release);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    h.fake.fail_close.store(false, Ordering::Release);
    h.clock.0.store(20_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert!(h.registry.snapshot(key)?.failed);
    assert_eq!(h.registry.snapshot(key)?.shutdown, budget);
    Ok(())
}

#[test]
fn secondary_reserve_overload_keeps_exact_member_and_round_robin_budget() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    h.registry.cleanup_watches = 64;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let step = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    assert!(matches!(
        h.registry.submit_cleanup_step(&h.client, key, &h.scope, step, Some(watch)),
        Err(RegistryAdmissionError::Saturated)
    ));
    assert_eq!(h.registry.snapshot(key)?.retained_members, 2);
    assert!(h.registry.snapshot(key)?.member_watch_required);
    assert_eq!(h.registry.next_cleanup(), Some(key));
    h.registry.cleanup_watches = 0;
    Ok(())
}

#[test]
fn cleanup_clock_regression_rejects_without_force_or_settlement() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    // Supply the registry's budget independently; its cleanup observes 99 while
    // the executor provider remains monotonic rather than failing admission first.
    h.clock.0.store(99, Ordering::Release);
    let original = ShutdownBudget::new(Moment::from_nanos(100))?;
    let shutdown = ShutdownId::new(1)?;
    h.registry.begin_stop(key, shutdown, original)?;
    h.cleanup(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.settled);
    assert_eq!(h.registry.snapshot(key)?.shutdown, Some((shutdown, original)));
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 0);
    let index = h.registry.index(key)?;
    let error = h.registry.records[index]
        .as_ref()
        .and_then(|record| record.error.as_ref())
        .ok_or("Missing retained cleanup failure")?;
    // The fake root watch already reports cancellation; cleanup must preserve
    // that first failure and retain its own exact regression cause separately.
    assert_eq!(error.stage(), "process_observation_not_exit");
    assert!(
        error.cleanup_causes().iter().any(|cause| cause.stage() == "shutdown_clock_regression")
    );
    Ok(())
}

#[test]
fn actual_resume_survives_post_resume_thread_close_failure() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.fail_resume_close.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    assert!(h.registry.snapshot(key)?.started);
    assert!(h.registry.snapshot(key)?.failed);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 1);
    assert!(matches!(
        h.registry.take_observation(key)?,
        Some(RegistryObservation::ContainedStarted { .. })
    ));
    let index = h.registry.index(key)?;
    assert_eq!(
        h.registry.records[index].as_ref().and_then(|r| r.error.as_ref()).map(RegistryError::stage),
        Some("close_primary_thread")
    );
    Ok(())
}

#[test]
fn inventory_or_secondary_duplication_failure_still_attempts_owned_force_after_grace() -> TestResult
{
    for inventory in [true, false] {
        let mut h = Harness::new()?;
        h.fake.include_leaf.store(true, Ordering::Release);
        h.fake.leaf_alive.store(true, Ordering::Release);
        let key = h.launch(1)?.process();
        h.created(key)?;
        h.stop(key)?;
        let index = h.registry.index(key)?;
        if let Some(record) = &mut h.registry.records[index] {
            record.error = None;
            record.unread[3] = false;
        }
        if !inventory {
            h.cleanup(key)?;
            h.fake.fail_duplicate.store(true, Ordering::Release);
        } else {
            h.fake.fail_inventory.store(true, Ordering::Release);
        }
        h.clock.0.store(10_000_000_000, Ordering::Release);
        h.cleanup(key)?;
        assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
        assert!(!h.registry.snapshot(key)?.settled);
        assert_eq!(
            h.registry.records[index]
                .as_ref()
                .and_then(|r| r.error.as_ref())
                .map(RegistryError::stage),
            Some(if inventory { "complete_job_inventory" } else { "duplicate_watch_process" })
        );
        h.cleanup(key)?;
        assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    }
    Ok(())
}
