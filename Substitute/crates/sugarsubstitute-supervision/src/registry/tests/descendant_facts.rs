// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Typed cached facts use real registry transitions and controlled native effects.
//! The watch fake cancels an unbound reservation; these tests prove identity/cache
//! rules, not actual registered process death.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;

#[test]
fn root_exit_and_force_request_do_not_fabricate_successful_force_or_settlement() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.exit(key, ROOT, true, 0)?;
    let snapshot = h.registry.snapshot(key)?;
    assert_eq!(snapshot.root_exit, Some(0));
    assert!(snapshot.last_member_exit.is_none());
    assert!(!snapshot.forced && !snapshot.settled);
    h.stop(key)?;
    assert!(!h.registry.snapshot(key)?.forced);
    h.fake.fail_terminate.store(true, Ordering::Release);
    h.clock.0.store(10_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.forced);
    h.fake.fail_terminate.store(false, Ordering::Release);
    h.cleanup(key)?;
    let snapshot = h.registry.snapshot(key)?;
    assert!(snapshot.forced);
    assert!(!snapshot.settled);
    assert!(snapshot.last_member_exit.is_none());
    Ok(())
}

#[test]
fn latest_member_exit_retains_only_exact_validated_nonroot_identity() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.exit(key, ROOT, false, 0)?;
    assert!(h.registry.snapshot(key)?.last_member_exit.is_none());
    h.exit(key, LEAF, false, 1)?;
    let accepted = h.registry.snapshot(key)?.last_member_exit.ok_or("Missing member fact")?;
    assert_eq!(
        (accepted.process_id, accepted.creation_time_100ns, accepted.exit_code),
        (LEAF.pid, LEAF.birth, 1)
    );
    for (correlation, pid, birth) in [
        (key.id.get() + 1, LEAF.pid, LEAF.birth),
        (key.id.get(), LEAF.pid, LEAF.birth + 1),
        (key.id.get(), ROOT.pid, LEAF.birth),
    ] {
        let index = h.registry.index(key)?;
        let record = h.registry.records[index].as_mut().ok_or("Missing record")?;
        completion::observe_watch(
            record,
            JobOutcome::Success(ProcessExit {
                correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(
                    correlation,
                )?,
                process_id: pid,
                creation_time_100ns: birth,
                exit_code: 37,
            }),
            Some(LEAF),
            false,
            JobId::new(99)?,
        );
        assert_eq!(h.registry.snapshot(key)?.last_member_exit, Some(accepted));
        assert!(!h.registry.snapshot(key)?.settled);
    }
    Ok(())
}
