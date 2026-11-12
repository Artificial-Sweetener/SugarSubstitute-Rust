// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve supervised owner panic policy through actual typed delivery and omission.
use super::*;

#[test]
fn native_factory_panic_keeps_adopted_family_and_poisoned_state_until_owned_cleanup() -> TestResult
{
    let mut h = Harness::new()?;
    h.fake.panic_create.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    let index = h.registry.index(key)?;
    assert!(
        h.registry.records[index]
            .as_ref()
            .and_then(|r| r.error.as_ref())
            .is_some_and(RegistryError::contains_owner_panic)
    );
    assert!(!h.registry.snapshot(key)?.started);
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    h.stop(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 37)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert!(
        h.registry.records[index]
            .as_ref()
            .and_then(|r| r.error.as_ref())
            .is_some_and(RegistryError::contains_owner_panic)
    );
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn primary_watch_owner_panic_is_typed_fatal_and_outer_diagnostics_stay_redacted() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    let index = h.registry.index(key)?;
    assert!(
        h.registry.records[index]
            .as_ref()
            .and_then(|r| r.error.as_ref())
            .is_some_and(|error| !error.contains_owner_panic())
    );
    let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let watch: WatchJob = h.client.submit_blocking(&h.scope, spec, |_| {
        std::panic::resume_unwind(Box::new("private E:\\secret supervised watch payload"));
    })?;
    let record = h.registry.records[index].as_mut().ok_or("Missing family")?;
    record.error = None;
    record.unread[3] = false;
    record.root_spec = (watch.scope_id(), watch.spec().id);
    record.root_watch = Some(watch);
    h.drain_until(|registry| {
        registry.records[index].as_ref().is_some_and(|r| r.root_watch.is_none())
    })?;
    let error = h.registry.records[index]
        .as_ref()
        .and_then(|r| r.error.as_ref())
        .ok_or("Missing panic error")?;
    assert!(error.contains_owner_panic());
    assert_eq!(error.stage(), "process_observation");
    assert!(
        error
            .owner_panic_cause()
            .is_some_and(|cause| cause.as_str().contains("supervised watch payload"))
    );
    assert!(!format!("{error:?} {error}").contains("secret"));
    assert!(
        error
            .source()
            .ok_or("Missing retained cause")?
            .to_string()
            .contains("supervised watch payload")
    );
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}

#[test]
fn cleanup_owner_panic_preserves_original_native_failure_before_cause_inventory_fills() -> TestResult
{
    assert_cleanup_panic(0)
}

#[test]
fn owner_panic_remains_fatal_after_all_eight_secondary_cause_slots_are_full() -> TestResult {
    assert_cleanup_panic(8)
}

fn assert_cleanup_panic(secondary_count: usize) -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let job = h.client.submit_blocking(&h.scope, spec, |_| -> Result<(), RegistryError> {
        std::panic::resume_unwind(Box::new("private supervised cleanup payload"));
    })?;
    let index = h.registry.index(key)?;
    let record = h.registry.records[index].as_mut().ok_or("Missing family")?;
    record.error = None;
    record.unread[3] = false;
    if secondary_count > 0 {
        record.fail(RegistryError::invalid("first_native_failure"));
        for _ in 0..secondary_count {
            record.fail(RegistryError::invalid("secondary_native_failure"));
        }
    }
    lock(&record.shared.native).failure = Some(RegistryError::invalid("first_native_failure"));
    record.cleanup = Some(job);
    h.drain_until(|registry| {
        registry.records[index].as_ref().is_some_and(|r| r.cleanup.is_none())
    })?;
    let error = h.registry.records[index]
        .as_ref()
        .and_then(|r| r.error.as_ref())
        .ok_or("Missing native/panic error")?;
    assert_eq!(error.stage(), "first_native_failure");
    assert!(error.contains_owner_panic());
    assert!(error.clone().contains_owner_panic());
    assert_eq!(
        error.owner_panic_cause().map(sugarsubstitute_contracts::execution::PanicCause::as_str),
        Some("private supervised cleanup payload")
    );
    assert_eq!(error.clone().owner_panic_cause(), error.owner_panic_cause());
    assert!(!format!("{error:?} {error}").contains("payload"));
    if secondary_count == 0 {
        assert_eq!(error.cleanup_causes().len(), 1);
        assert!(error.cleanup_causes()[0].contains_owner_panic());
        assert!(
            error.cleanup_causes()[0]
                .source()
                .ok_or("Missing panic cause")?
                .to_string()
                .contains("supervised cleanup payload")
        );
    } else {
        assert_eq!(error.cleanup_causes().len(), 8);
        assert_eq!(error.omitted_causes(), 2);
    }
    Ok(())
}
