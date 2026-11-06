// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drivers that re-evaluate cleanup on every activity turn admit steps only for new facts.
use super::*;
use sugarsubstitute_contracts::execution::{CompletionNotification, TerminalKind};

/// One driver turn: submit a step only when due, then route its completion while
/// root and member watch notices stay held, as if those live processes had not exited.
fn turn(
    h: &mut Harness,
    key: ProcessKey,
    held: &mut Vec<CompletionNotification>,
) -> Result<bool, Box<dyn Error>> {
    let snapshot = h.registry.snapshot(key)?;
    if !snapshot.cleanup_step_due(h.clock.now()) {
        return Ok(false);
    }
    let watch = if snapshot.member_watch_required {
        Some(h.spec(JobClass::NativeObservation, 4096)?)
    } else {
        None
    };
    let step = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    h.registry.submit_cleanup_step(&h.client, key, &h.scope, step, watch)?;
    let until = Instant::now() + Duration::from_secs(10);
    while h.registry.snapshot(key)?.cleanup_step_pending {
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        let ready = defer_watch_notices(h, key, batch, held)?;
        h.registry.consume(&ready)?;
        if !h.registry.snapshot(key)?.cleanup_step_pending {
            break;
        }
        if Instant::now() >= until {
            return Err("Admitted cleanup step did not complete".into());
        }
        h.wake.wait(count)?;
    }
    Ok(true)
}

/// Retain watch metadata while allowing finite completions and foreign owners to progress.
fn defer_watch_notices(
    h: &Harness,
    key: ProcessKey,
    batch: CompletionBatch,
    held: &mut Vec<CompletionNotification>,
) -> Result<CompletionBatch, Box<dyn Error>> {
    let index = h.registry.index(key)?;
    let record = h.registry.records[index].as_ref().ok_or("Missing family")?;
    let root_spec = record.root_spec;
    let member_spec = record.member_spec;
    let mut ready = Vec::new();
    for notice in batch.notifications {
        let identity = (notice.scope_id, notice.job_id);
        if identity == root_spec || Some(identity) == member_spec {
            held.push(notice);
        } else {
            ready.push(notice);
        }
    }
    Ok(CompletionBatch { notifications: ready, remaining: batch.remaining })
}

/// Retain a real execution terminal until its deliberately chosen delivery turn.
fn take_watch_notice(
    h: &mut Harness,
    expected: (ScopeId, JobId),
    held: &mut Vec<CompletionNotification>,
) -> Result<CompletionNotification, Box<dyn Error>> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(index) =
            held.iter().position(|notice| (notice.scope_id, notice.job_id) == expected)
        {
            return Ok(held.remove(index));
        }
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        let remaining = batch.remaining;
        held.extend(batch.notifications);
        if held.iter().any(|notice| (notice.scope_id, notice.job_id) == expected) {
            continue;
        }
        if Instant::now() >= until {
            return Err("Actual member-watch terminal did not arrive".into());
        }
        if remaining == 0 {
            h.wake.wait(count)?;
        }
    }
}

/// Count the steps admitted over many driver turns at one clock reading.
fn turns(
    h: &mut Harness,
    key: ProcessKey,
    held: &mut Vec<CompletionNotification>,
    count: usize,
) -> Result<usize, Box<dyn Error>> {
    let mut steps = 0;
    for _ in 0..count {
        steps += usize::from(turn(h, key, held)?);
    }
    Ok(steps)
}

#[test]
fn watched_live_root_receives_one_step_per_fact_across_repeated_driver_turns() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    let mut held = observation_retirement::created_with_deferred_watch(&mut h, key)?.notifications;
    h.stop(key)?;
    let (_, budget) = h.registry.snapshot(key)?.shutdown.ok_or("Stop transaction absent")?;

    // Seal and inventory once; the live root's own watch supplies the next wake.
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    assert_eq!(h.fake.seals.load(Ordering::Acquire), 1);
    let waiting = h.registry.snapshot(key)?;
    assert!(!waiting.cleanup_pending && !waiting.member_watch_required);
    assert!(!waiting.cleanup_step_due(h.clock.now()));

    // Grace admits exactly the force; later turns wait for the actual exit.
    h.clock.0.store(budget.graceful_until.as_nanos(), Ordering::Release);
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    assert!(h.registry.snapshot(key)?.forced);
    h.clock.0.store(budget.graceful_until.as_nanos() + 3_000_000_000, Ordering::Release);
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 0);

    // The observed root exit is one new fact: one step records it and closes the Job.
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 1)?;
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    h.registry
        .consume(&CompletionBatch { notifications: std::mem::take(&mut held), remaining: 0 })?;
    // The fake's unbound root watch retires as cancelled; settlement still follows it.
    let settled = h.registry.snapshot(key)?;
    assert!(settled.settled && settled.forced);
    assert_eq!(settled.root_exit, Some(1));
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 0);
    Ok(())
}

#[test]
fn empty_job_awaiting_a_member_exit_is_not_restepped_after_grace() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    let mut held = observation_retirement::created_with_deferred_watch(&mut h, key)?.notifications;
    h.stop(key)?;
    let (_, budget) = h.registry.snapshot(key)?.shutdown.ok_or("Stop transaction absent")?;
    // Inventory adopts the leaf, which then needs its own watch; that owned watch
    // and the root's supply the next wakes.
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 2);
    assert!(h.registry.snapshot(key)?.cleanup_pending);
    // Both members leave the Job before grace without an exit proof for the leaf.
    h.fake.root_alive.store(false, Ordering::Release);
    h.fake.leaf_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    let empty = h.registry.snapshot(key)?;
    assert!(empty.inventory_empty && !empty.settled);
    // Nothing remains to terminate, so grace admits no force and no repeated inventory.
    h.clock.0.store(budget.graceful_until.as_nanos(), Ordering::Release);
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 0);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    Ok(())
}

/// Keep a prior genuine watch terminal held until its explicit delivery boundary.
#[test]
fn late_owned_member_notice_stays_deferred_without_a_new_watch() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    let mut held = observation_retirement::created_with_deferred_watch(&mut h, key)?.notifications;
    h.stop(key)?;
    let (_, budget) = h.registry.snapshot(key)?.shutdown.ok_or("Stop transaction absent")?;
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 2);
    let index = h.registry.index(key)?;
    let member = h.registry.records[index]
        .as_ref()
        .and_then(|record| record.member_spec)
        .ok_or("Member watch absent")?;
    let late = take_watch_notice(&mut h, member, &mut held)?;
    assert_eq!(late.kind, TerminalKind::Cancelled);
    assert!(!h.registry.snapshot(key)?.member_watch_required);

    // The genuine earlier terminal arrives during a turn that admits no new watch.
    let foreign = CompletionNotification { scope_id: ScopeId::new(999)?, ..late };
    assert_ne!(foreign.scope_id, late.scope_id);
    let ready = defer_watch_notices(
        &h,
        key,
        CompletionBatch { notifications: vec![foreign, late], remaining: 0 },
        &mut held,
    )?;
    assert_eq!(ready.notifications, vec![foreign]);
    assert!(held.contains(&late));
    assert_eq!(h.registry.consume(&ready)?, 0);
    assert!(!h.registry.snapshot(key)?.failed);

    h.fake.root_alive.store(false, Ordering::Release);
    h.fake.leaf_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    assert!(h.registry.snapshot(key)?.inventory_empty);
    h.clock.0.store(budget.graceful_until.as_nanos(), Ordering::Release);
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 0);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);

    // Deliberate delivery still creates the production fact; cancellation is no exit.
    let released = take_watch_notice(&mut h, member, &mut held)?;
    assert_eq!(
        h.registry.consume(&CompletionBatch { notifications: vec![released], remaining: 0 })?,
        1
    );
    let observed = h.registry.snapshot(key)?;
    assert!(observed.cleanup_fact_pending && observed.failed && !observed.settled);
    assert_eq!(observed.last_member_exit, None);
    assert_eq!(observed.root_exit, Some(0));
    assert_eq!(turns(&mut h, key, &mut held, 40)?, 1);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}
