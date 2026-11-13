// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real retained-family qualification with independent current native inventory and failure custody.
use super::*;
use sugarsubstitute_contracts::execution::{JobCompletion, JobOutcome};
fn completion<T: Send + 'static, E: Send + 'static>(
    h: &mut Harness,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
    loop {
        let revision = *lock(&h.wake.count);
        h.registry
            .consume(&h.client.drain_completions(
                std::num::NonZeroUsize::new(32).ok_or("Drain bound invalid")?,
            )?)?;
        if let Some(completed) = job.try_take_completion() {
            return Ok(completed);
        }
        h.wake.wait(revision)?;
    }
}
fn duplicate(
    h: &mut Harness,
    ticket: LaunchTicket,
    expected: Identity,
) -> Result<bool, Box<dyn Error>> {
    let proof = h.registry.retained_family(ticket)?;
    let mut spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    spec.deadline = Some(Moment::from_nanos(1_000_000_000));
    let mut job = h.client.submit_blocking(&h.scope, spec, move |context| {
        proof.duplicate_member(expected, &context)
    })?;
    Ok(matches!(completion(h, &mut job)?.outcome, JobOutcome::Success(WatchInput::Simulated)))
}
#[test]
fn exact_current_member_is_qualified_before_watch_binding_and_stale_birth_or_unlisted_pid_never_duplicates()
-> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    h.created(ticket.process())?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let before = h.fake.duplicates.load(Ordering::Acquire);
    assert!(duplicate(&mut h, ticket, LEAF)?);
    assert_eq!(h.fake.duplicates.load(Ordering::Acquire), before + 1);
    assert!(!duplicate(&mut h, ticket, Identity { birth: LEAF.birth + 1, ..LEAF })?);
    h.fake.leaf_alive.store(false, Ordering::Release);
    assert!(!duplicate(&mut h, ticket, LEAF)?);
    assert!(!duplicate(&mut h, ticket, Identity { pid: 999, birth: LEAF.birth })?);
    assert_eq!(h.fake.duplicates.load(Ordering::Acquire), before + 1);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn partial_member_duplicate_and_failed_close_retain_original_family_slot_until_paired_cleanup()
-> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    let key = ticket.process();
    h.created(key)?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    h.fake.partial_duplicate_failure.store(true, Ordering::Release);
    assert!(!duplicate(&mut h, ticket, LEAF)?);
    let index = h.registry.index(key)?;
    assert_eq!(
        lock(&h.registry.records[index].as_ref().ok_or("Actual family absent")?.shared.native)
            .family
            .as_ref()
            .ok_or("Actual native family absent")?
            .watch_duplicate,
        Some(14)
    );
    h.stop(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.fake.leaf_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.exit(key, LEAF, false, 0)?;
    h.fake.fail_close.store(true, Ordering::Release);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.settled);
    assert_eq!(
        lock(&h.registry.records[index].as_ref().ok_or("Failed family lost")?.shared.native)
            .family
            .as_ref()
            .ok_or("Failed native family lost")?
            .watch_duplicate,
        Some(14)
    );
    h.fake.fail_close.store(false, Ordering::Release);
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    Ok(())
}
