// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual typed delivery, early notifications and shared-mailbox fanout.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;

fn observed_job(
    h: &mut Harness,
    key: ProcessKey,
    identity: Identity,
    correlation: u64,
) -> Result<WatchJob, Box<dyn Error>> {
    let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let exit = ProcessExit {
        correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(correlation)?,
        process_id: identity.pid,
        creation_time_100ns: identity.birth,
        exit_code: 259,
    };
    assert_eq!(key.owner, owner()?);
    // The external native observation is faked; admission, mailbox delivery,
    // terminal transfer and registry identity checking use their real owners.
    Ok(h.client.submit_blocking(&h.scope, spec, move |_| Ok(exit))?)
}

#[test]
fn successful_root_and_member_notices_wait_for_finite_cleanup_and_transfer_actual_outcomes()
-> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    let root = observed_job(&mut h, key, ROOT, key.id.get())?;
    let member = observed_job(&mut h, key, LEAF, key.id.get())?;
    let held = Fake::new();
    held.hold_create.store(true, Ordering::Release);
    let factory = held.clone();
    let cleanup_spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let cleanup = h.client.submit_blocking(&h.scope, cleanup_spec, move |_| {
        let mut guard = lock(&factory.state);
        guard.0 = true;
        factory.changed.notify_all();
        let (_guard, timed) = factory
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |state| !state.1)
            .map_err(|_| RegistryError::invalid("fake_cleanup_latch"))?;
        if timed.timed_out() {
            return Err(RegistryError::invalid("fake_cleanup_latch"));
        }
        Ok(())
    })?;
    held.entered()?;
    let index = h.registry.index(key)?;
    let record = h.registry.records[index].as_mut().ok_or("Missing family")?;
    record.root_spec = (root.scope_id(), root.spec().id);
    record.root_watch = Some(root);
    record.member_spec = Some((member.scope_id(), member.spec().id));
    record.member_watch = Some(member);
    record.watched = Some(LEAF);
    record.cleanup = Some(cleanup);
    h.registry.cleanup_watches = 1;
    h.drain_until(|registry| {
        registry.records[index].as_ref().is_some_and(|r| r.root_notice && r.member_notice)
    })?;
    assert_eq!(h.registry.snapshot(key)?.root_exit, None);
    assert_eq!(h.registry.cleanup_watches, 1);
    held.release();
    h.drain_until(|registry| {
        registry.records[index]
            .as_ref()
            .is_some_and(|r| r.root_watch.is_none() && r.member_watch.is_none())
    })?;
    assert_eq!(h.registry.snapshot(key)?.root_exit, Some(259));
    assert_eq!(h.registry.records[index].as_ref().and_then(|r| r.pending_exit), Some(LEAF));
    assert_eq!(h.registry.cleanup_watches, 0);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}

#[test]
fn actual_typed_watch_rejects_foreign_pid_birth_or_correlation() -> TestResult {
    for field in 0..3 {
        let mut h = Harness::new()?;
        let key = h.launch(1)?.process();
        h.created(key)?;
        let mut identity = ROOT;
        if field == 0 {
            identity.pid += 1;
        } else if field == 1 {
            identity.birth += 1;
        }
        let job = observed_job(&mut h, key, identity, key.id.get() + u64::from(field == 2))?;
        let index = h.registry.index(key)?;
        let record = h.registry.records[index].as_mut().ok_or("Missing family")?;
        record.error = None;
        record.unread[3] = false;
        record.root_spec = (job.scope_id(), job.spec().id);
        record.root_watch = Some(job);
        h.drain_until(|registry| {
            registry.records[index].as_ref().is_some_and(|r| r.root_watch.is_none())
        })?;
        assert_eq!(h.registry.snapshot(key)?.root_exit, None);
        assert_eq!(
            h.registry.records[index]
                .as_ref()
                .and_then(|r| r.error.as_ref())
                .map(RegistryError::stage),
            Some("watch_exit_identity")
        );
    }
    Ok(())
}

#[test]
fn unknown_owner_terminal_remains_available_in_same_borrowed_batch() -> TestResult {
    let mut h = Harness::new()?;
    let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let mut other = h.client.submit_blocking(&h.scope, spec, |_| Ok::<_, RegistryError>(37u32))?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let count = *lock(&h.wake.count);
        let batch = h
            .client
            .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain bound")?)?;
        assert_eq!(h.registry.consume(&batch)?, 0);
        if batch
            .notifications
            .iter()
            .any(|n| n.scope_id == other.scope_id() && n.job_id == other.spec().id)
        {
            assert!(matches!(
                other.try_take_completion().map(|c| c.outcome),
                Some(JobOutcome::Success(37))
            ));
            assert_eq!(batch.notifications.len(), 1);
            break;
        }
        if Instant::now() >= until {
            return Err("Unrelated owner notification not delivered".into());
        }
        h.wake.wait(count)?;
    }
    Ok(())
}
