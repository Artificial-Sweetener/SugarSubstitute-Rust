// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the real finite release, source association and supplied external ACK boundary.
use super::super::*;
use super::support::{Harness, TestResult, run};
use sugarsubstitute_contracts::{backend_custody::*, execution::*};

#[test]
fn release_observer_rejects_acquisition_and_foreign_run_without_consuming_original_job()
-> TestResult {
    let mut h = Harness::new()?;
    let acquire = h.spec(100)?;
    h.lease.submit_acquire(&h.client, &h.scope, acquire)?;
    let acquired = h.terminal_batch()?;
    let rejected = h.lease.consume_release(run()?, &acquired).err().ok_or("Acquisition stolen")?;
    assert_eq!(rejected.operation(), "profile_release_observation_purpose");
    assert!(h.lease.snapshot().pending);
    assert_eq!(h.lease.consume(&acquired)?.ok_or("Acquire lost")?.kind, TerminalKind::Success);
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    let mut foreign = run()?;
    foreign.generation = sugarsubstitute_contracts::lifecycle::RunGeneration::new(2)?;
    let rejected = h.lease.consume_release(foreign, &batch).err().ok_or("Foreign run accepted")?;
    assert_eq!(rejected.operation(), "profile_release_observation_run");
    assert!(h.lease.snapshot().pending);
    let completion = h.lease.consume_release(run()?, &batch)?.ok_or("Release lost")?;
    assert_eq!((completion.job, completion.kind), (release.id, TerminalKind::Success));
    assert!(h.lease.snapshot().retirement_ready());
    h.finish_workers();
    Ok(())
}

#[test]
fn release_observer_ignores_foreign_scope_and_preserves_actual_worker_failure() -> TestResult {
    let mut h = Harness::new()?;
    assert_eq!(h.acquire()?.kind, TerminalKind::Success);
    h.fake.fail_release.store(true, Ordering::Release);
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    let primary = lock(&h.lease.shared.native)
        .as_ref()
        .and_then(|state| state.failure.clone())
        .ok_or("Original worker failure absent")?;
    let mut foreign =
        CompletionBatch { notifications: batch.notifications.clone(), remaining: batch.remaining };
    for notice in &mut foreign.notifications {
        notice.scope_id = ScopeId::new(99)?;
    }
    assert!(h.lease.consume_release(run()?, &foreign)?.is_none());
    assert!(h.lease.snapshot().pending);
    let actual = h.lease.consume_release(run()?, &batch)?.ok_or("Failed release lost")?;
    assert_eq!((actual.job, actual.kind), (release.id, TerminalKind::Failure));
    assert!(actual.failure.ok_or("Original worker cause discarded")?.same_primary(&primary));
    assert!(!h.lease.snapshot().pending);
    assert!(!h.lease.snapshot().retirement_ready());
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    h.fake.fail_release.store(false, Ordering::Release);
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    h.finish_workers();
    Ok(())
}

#[test]
fn release_observer_advances_original_profile_released_after_finite_job_consumption() -> TestResult
{
    release_acknowledgement_order(false, false)
}

/// An earlier exact helper ACK still waits for successful finite completion consumption.
#[test]
fn profile_successor_observes_helper_ack_before_original_finite_release_completion() -> TestResult {
    release_acknowledgement_order(true, false)
}

/// Physical closure and an authentic ACK cannot turn a timed-out finite release into success.
#[test]
fn profile_successor_rejects_late_finite_release_even_after_exact_helper_ack() -> TestResult {
    release_acknowledgement_order(false, true)
}

/// Supply only external native and correlated wire facts through existing profile consumers.
fn release_acknowledgement_order(ack_before_terminal: bool, late_terminal: bool) -> TestResult {
    use crate::backend_custody_parent::observation_fixture;
    let mut h = Harness::new()?;
    assert_eq!(h.acquire()?.kind, TerminalKind::Success);
    let barrier = h
        .lease
        .successor_retirement(std::path::Path::new(r"E:\fixture\profile"))
        .ok_or("Actual acquired profile binding absent")?;
    let scope =
        h.client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut spec = h.spec(100)?;
    spec.class = JobClass::NativeObservation;
    spec.deadline = None;
    spec.reservation_bytes =
        sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;
    let parent = observation_fixture::parent(
        &h.client,
        &scope,
        spec,
        BackendCustodyKey::new(run()?.owner, BackendCustodySession::new(8)?),
        Moment::from_nanos(30_000_000_000),
    )?;
    let profile = ProfileCustodyKey::new(run()?, 2)?;
    let counter = h.client.encode_lifecycle_deadline(Moment::from_nanos(100))?;
    // Supply only external wire/retirement facts at the real correlation boundary.
    // This constructs no helper, remote output cells or native acknowledgement proof.
    {
        let mut native = lock(&h.lease.shared.native);
        let state = native.as_mut().ok_or("Original state absent")?;
        state.custody_record = Some(record::custody::CustodiedRecord::new(
            state.record.ok_or("Original native record absent")?,
            parent.key(),
            profile,
        )?);
        state.custody_release = Some(custody::Release {
            parent: parent.clone(),
            key: profile,
            until: Some(Moment::from_nanos(100)),
            nonce: Some(7),
            counter: Some((counter.ticks(), counter.frequency())),
            ack: None,
        });
    }
    if late_terminal {
        *lock(&h.fake.late_release) = Some(h.clock.clone());
    }
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    let reply = || {
        observation_fixture::reply(
            &parent,
            7,
            BackendCustodyCommand::ProfileStop {
                profile,
                until_ticks: counter.ticks(),
                frequency: counter.frequency(),
            },
            BackendCustodyReceipt::ProfileReleased(profile),
        )
    };
    if ack_before_terminal {
        reply()?;
        assert!(!barrier.released());
    }
    let actual = h.lease.consume_release(run()?, &batch)?.ok_or("Original release absent")?;
    assert_eq!(actual.job, release.id);
    assert_eq!(
        actual.kind,
        if late_terminal { TerminalKind::TimedOut } else { TerminalKind::Success }
    );
    assert!(actual.failure.is_none());
    assert!(!h.lease.snapshot().pending);
    assert_eq!(barrier.released(), ack_before_terminal && !late_terminal);
    if !ack_before_terminal {
        reply()?;
        assert!(h.lease.custody_release_pending());
    }
    let empty = CompletionBatch { notifications: vec![], remaining: 0 };
    assert!(h.lease.consume_release(run()?, &empty)?.is_none());
    assert!(!h.lease.custody_release_pending());
    assert_eq!(barrier.released(), !late_terminal);
    assert!(h.lease.snapshot().retirement_ready());
    let native = lock(&h.lease.shared.native);
    let retained = native
        .as_ref()
        .and_then(|state| state.custody_release.as_ref())
        .and_then(|release| release.ack.as_ref())
        .ok_or("Original ProfileReleased not retained")?;
    assert_eq!(retained.nonce(), 7);
    assert_eq!(retained.receipt(), BackendCustodyReceipt::ProfileReleased(profile));
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    drop(native);
    parent.close()?;
    drop(parent);
    h.finish_workers();
    Ok(())
}
