// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A failed native duplication does not turn cleanup activity into an exit or retry proof.
use super::*;
use sugarsubstitute_contracts::execution::TerminalKind;

/// Capture exact production notifications and let the registry transfer its typed jobs.
/// The harness wake condition bounds waiting; no fabricated completion or sleep is used.
fn consume_exact(
    h: &mut Harness,
    key: ProcessKey,
    expected: &[(ScopeId, JobId, TerminalKind)],
) -> TestResult {
    assert!(!expected.is_empty() && expected.len() <= 2);
    let mut seen = [false; 2];
    loop {
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        for notice in &batch.notifications {
            for (index, &(scope, job, kind)) in expected.iter().enumerate() {
                if (notice.scope_id, notice.job_id) == (scope, job) {
                    assert!(!seen[index], "An exact terminal was notified twice");
                    assert_eq!(notice.kind, kind);
                    seen[index] = true;
                }
            }
        }
        h.registry.consume(&batch)?;
        if seen[..expected.len()].iter().all(|seen| *seen)
            && !h.registry.snapshot(key)?.cleanup_pending
        {
            return Ok(());
        }
        h.wake.wait(count)?;
    }
}

#[test]
fn failed_member_duplication_yields_failure_and_unbound_cancel_without_exit_or_settlement()
-> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.stop(key)?;
    let budget = h.registry.snapshot(key)?.shutdown;
    let inventory = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    assert_eq!(
        h.registry.submit_cleanup_step(&h.client, key, &h.scope, inventory, None)?,
        inventory.id
    );
    let inventory_scope = h.scope.id();
    consume_exact(&mut h, key, &[(inventory_scope, inventory.id, TerminalKind::Success)])?;
    let before = h.registry.snapshot(key)?;
    assert_eq!(before.retained_members, 2);
    assert!(before.member_watch_required);
    assert!(!before.cleanup_pending);
    assert_eq!(before.root_exit, None);
    let index = h.registry.index(key)?;
    let (native_scope, candidate) = {
        let record = h.registry.records[index].as_ref().ok_or("Missing family")?;
        (record.native_scope.id(), record.next)
    };
    assert!(candidate.is_some());
    h.fake.fail_duplicate.store(true, Ordering::Release);
    let member = h.spec(JobClass::NativeObservation, 4096)?;
    let cleanup = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    assert!(member.id < cleanup.id);
    assert_eq!(
        h.registry.submit_cleanup_step(&h.client, key, &h.scope, cleanup, Some(member))?,
        cleanup.id
    );
    let cleanup_scope = h.scope.id();
    consume_exact(
        &mut h,
        key,
        &[
            (cleanup_scope, cleanup.id, TerminalKind::Failure),
            (native_scope, member.id, TerminalKind::Cancelled),
        ],
    )?;
    let after = h.registry.snapshot(key)?;
    assert!(after.member_watch_required);
    assert!(!after.cleanup_pending);
    assert!(!after.settled);
    assert_eq!(after.retained_members, before.retained_members);
    assert_eq!(after.root_exit, None);
    assert_eq!(after.shutdown, budget);
    assert_eq!(h.clock.now(), Moment::from_nanos(0));
    assert_eq!(h.registry.cleanup_watches, 0);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 0);
    assert!(h.fake.fail_duplicate.load(Ordering::Acquire));
    let record = h.registry.records[index].as_ref().ok_or("Missing retained family")?;
    assert_eq!(record.next, candidate);
    assert!(record.cleanup.is_none() && record.member_watch.is_none());
    assert!(record.member_spec.is_none() && record.pending_exit.is_none());
    let error = record.error.as_ref().ok_or("Missing retained native failure")?;
    // The simulated original root-watch cancellation stays primary. The actual
    // finite duplicate failure is retained separately, with its original OS cause.
    assert_eq!(error.stage(), "process_observation_not_exit");
    let duplicate = error
        .cleanup_causes()
        .iter()
        .find(|cause| cause.stage() == "duplicate_watch_process")
        .ok_or("Missing duplicate-handle cause")?;
    assert_eq!(
        duplicate
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(6)
    );
    while let Some(observation) = h.registry.take_observation(key)? {
        assert!(!matches!(
            observation,
            RegistryObservation::RootExited { .. } | RegistryObservation::FamilySettled { .. }
        ));
    }
    assert!(!h.registry.is_empty());
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    Ok(())
}
