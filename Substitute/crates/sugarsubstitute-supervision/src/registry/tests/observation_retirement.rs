// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish actual unbound watch retirement from fake-boundary family effects.

use super::*;
use sugarsubstitute_contracts::execution::{CompletionNotification, TerminalKind};

/// Forward the actual creation notice while retaining the watch notice at its owner.
pub(super) fn created_with_deferred_watch(
    h: &mut Harness,
    key: ProcessKey,
) -> Result<CompletionBatch, Box<dyn Error>> {
    let index = h.registry.index(key)?;
    let spec = h.registry.records[index].as_ref().ok_or("Missing family")?.root_spec;
    let mut held = Vec::new();
    loop {
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        let mut ready = Vec::new();
        for notice in batch.notifications {
            if (notice.scope_id, notice.job_id) == spec {
                held.push(notice);
                if held.len() > 1 {
                    return Err("Duplicate native watch notification".into());
                }
            } else {
                ready.push(notice);
            }
        }
        h.registry
            .consume(&CompletionBatch { notifications: ready, remaining: batch.remaining })?;
        if !h.registry.snapshot(key)?.creation_pending {
            return Ok(CompletionBatch { notifications: held, remaining: batch.remaining });
        }
        if batch.remaining == 0 {
            h.wake.wait(count)?;
        }
    }
}

#[test]
fn observation_retirement_rejects_foreign_and_pending_creation_without_effects() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.hold_create.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.fake.entered()?;
    let foreign = ProcessKey {
        owner: CoordinatorKey { epoch: CoordinatorEpoch::new([2; 16])?, ..key.owner },
        ..key
    };
    let rejected = matches!(
        h.registry.request_observation_retirement(foreign),
        Err(RegistryAdmissionError::ForeignOwner)
    ) && matches!(
        h.registry.request_observation_retirement(key),
        Err(RegistryAdmissionError::Busy)
    );
    let unchanged = h.registry.snapshot(key)?.shutdown.is_none()
        && h.registry.take_observation(key)?.is_none()
        && h.fake.resumes.load(Ordering::Acquire) == 0
        && h.fake.forces.load(Ordering::Acquire) == 0;
    h.fake.release();
    let deferred = created_with_deferred_watch(&mut h, key)?;
    let index = h.registry.index(key)?;
    let spec = h.registry.records[index].as_ref().ok_or("Missing family")?.root_spec;
    let before = h.registry.snapshot(key)?;
    let first = h.registry.request_observation_retirement(key)?;
    assert_eq!(h.registry.request_observation_retirement(key)?, first);
    assert_eq!(first, spec.1);
    assert_eq!(h.registry.snapshot(key)?, before);
    let unrelated = CompletionNotification {
        scope_id: ScopeId::new(999)?,
        job_id: first,
        kind: TerminalKind::Cancelled,
        replacement: None,
    };
    assert_eq!(
        h.registry.consume(&CompletionBatch { notifications: vec![unrelated], remaining: 0 })?,
        0
    );
    assert!(
        h.registry.records[index].as_ref().is_some_and(|record| record.root_retirement.is_none())
    );
    h.registry.consume(&deferred)?;
    h.drain_until(|registry| {
        registry.records[index].as_ref().is_some_and(|record| record.root_watch.is_none())
    })?;
    assert!(rejected);
    assert!(unchanged);
    Ok(())
}

#[test]
fn observation_retirement_fact_requires_actual_typed_completion_and_preserves_family() -> TestResult
{
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    assert!(h.registry.take_observation(key)?.is_none());
    // The native boundary supplies an unbound permit cancellation. Its real
    // execution terminal must return before the registry exposes this fact.
    h.created(key)?;
    let snapshot = h.registry.snapshot(key)?;
    assert_eq!(snapshot.root_exit, None);
    assert!(!snapshot.settled);
    assert!(snapshot.shutdown.is_none());
    assert_eq!(snapshot.retained_members, 1);
    assert!(h.fake.root_alive.load(Ordering::Acquire));
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 0);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    let mut retired = 0;
    while let Some(observation) = h.registry.take_observation(key)? {
        match observation {
            RegistryObservation::RootObservationRetired { process, job, outcome } => {
                assert_eq!(process, key);
                assert_eq!(job.get(), 2);
                assert_eq!(outcome, RootObservationOutcome::Cancelled);
                retired += 1;
            }
            RegistryObservation::ContainedStarted { .. } | RegistryObservation::Failed { .. } => {}
            _ => return Err("Cancellation fabricated process/family exit".into()),
        }
    }
    assert_eq!(retired, 1);
    assert!(matches!(
        h.registry.request_observation_retirement(key),
        Err(RegistryAdmissionError::Closed)
    ));
    Ok(())
}

#[test]
fn observation_retirement_does_not_prevent_later_actual_root_reobservation() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.member_watch_required);
    assert_eq!(h.registry.snapshot(key)?.root_exit, None);
    assert!(!h.registry.snapshot(key)?.settled);
    h.cleanup(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, false, 259)?;
    assert_eq!(h.registry.snapshot(key)?.root_exit, Some(259));
    assert!(!h.registry.snapshot(key)?.settled);
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    while h.registry.take_observation(key)?.is_some() {}
    h.registry.retire_settled(key)?;
    assert!(h.registry.is_empty());
    Ok(())
}

#[test]
fn observation_retirement_request_does_not_replace_already_returned_success() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    while h.registry.take_observation(key)?.is_some() {}
    let observed = super::completions::observed_job(&mut h, key, ROOT, key.id.get())?;
    let spec = (observed.scope_id(), observed.spec().id);
    let index = h.registry.index(key)?;
    let record = h.registry.records[index].as_mut().ok_or("Missing family")?;
    record.root_spec = spec;
    record.root_watch = Some(observed);
    let returned = loop {
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if batch.notifications.iter().any(|notice| (notice.scope_id, notice.job_id) == spec) {
            break batch;
        }
        h.registry.consume(&batch)?;
        if batch.remaining == 0 {
            h.wake.wait(count)?;
        }
    };
    assert_eq!(h.registry.request_observation_retirement(key)?, spec.1);
    h.registry.consume(&returned)?;
    assert_eq!(h.registry.snapshot(key)?.root_exit, Some(259));
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(matches!(
        h.registry.take_observation(key)?,
        Some(RegistryObservation::RootExited { code: 259, .. })
    ));
    assert!(h.registry.take_observation(key)?.is_none());
    Ok(())
}
