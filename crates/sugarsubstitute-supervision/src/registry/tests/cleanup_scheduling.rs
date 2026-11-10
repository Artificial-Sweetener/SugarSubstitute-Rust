// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An owned member observation cannot block the finite force step at original grace.
use super::*;
/// Always release the controlled external effect before the actual owner joins.
struct ReleaseCreation(Arc<Fake>);
impl Drop for ReleaseCreation {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn custody_retirement_hint_returns_while_actual_native_creation_holds_its_state() -> TestResult {
    let mut h = Harness::new()?;
    let release = ReleaseCreation(h.fake.clone());
    h.fake.hold_create.store(true, Ordering::Release);
    let ticket = h.launch(1)?;
    h.fake.entered()?;
    assert!(h.registry.custody_retirement_pending(ticket.process())?);
    assert!(h.registry.snapshot(ticket.process())?.creation_pending);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!lock(&h.fake.state).1);
    h.fake.release();
    h.created(ticket.process())?;
    assert!(!h.registry.custody_retirement_pending(ticket.process())?);
    h.stop(ticket.process())?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.cleanup(ticket.process())?;
    drop(release);
    Ok(())
}
#[test]
fn retained_member_watch_allows_finite_force_at_original_grace_without_an_exit_proof() -> TestResult
{
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    let original = h.registry.snapshot(key)?.shutdown.ok_or("Stop transaction absent")?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let inventory = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    h.registry.submit_cleanup_step(&h.client, key, &h.scope, inventory, Some(watch))?;
    // Route the finite job's genuine notice first. The separately admitted watch's
    // actual terminal is deliberately retained for its independent owner boundary.
    let mut retained = Vec::new();
    loop {
        let count = *lock(&h.wake.count);
        let batch = h.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        let mut finite = Vec::new();
        for notice in batch.notifications {
            if notice.job_id == watch.id {
                retained.push(notice);
            } else {
                finite.push(notice);
            }
        }
        h.registry.consume(&CompletionBatch { notifications: finite, remaining: 0 })?;
        if h.registry.snapshot(key)?.cleanup_pending
            && !h.registry.snapshot(key)?.cleanup_step_pending
        {
            break;
        }
        h.wake.wait(count)?;
    }
    let before = h.registry.snapshot(key)?;
    assert!(before.cleanup_pending);
    assert!(!before.cleanup_step_pending);
    assert!(!before.cleanup_step_due(Moment::from_nanos(9_999_999_999)));
    assert!(before.cleanup_step_due(original.1.graceful_until));
    assert!(!before.cleanup_step_due(original.1.settle_until));
    assert!(!before.forced && !before.settled);
    h.clock.0.store(original.1.graceful_until.as_nanos(), Ordering::Release);
    let mut force = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    force.deadline = Some(original.1.settle_until);
    h.registry.submit_cleanup_step(&h.client, key, &h.scope, force, None)?;
    assert!(h.registry.snapshot(key)?.cleanup_step_pending);
    h.drain_until(|registry| registry.snapshot(key).is_ok_and(|s| !s.cleanup_step_pending))?;
    let after = h.registry.snapshot(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    assert!(after.forced && !after.settled);
    assert_eq!(after.shutdown, Some(original));
    assert_eq!(after.retained_members, 2);
    // An actual cancelled watch is no member exit, nor permission to acknowledge settlement.
    h.registry.consume(&CompletionBatch { notifications: retained, remaining: 0 })?;
    assert!(!h.registry.snapshot(key)?.settled);
    assert_eq!(h.registry.snapshot(key)?.shutdown, Some(original));
    Ok(())
}
