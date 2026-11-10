// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reorder real service policy custody and completion delivery without constructing native input.
use super::*;
use crate::completion_wake::Gate;

struct Rig {
    execution: ExecutionOwner,
    client: ExecutionClient,
    scope: Scope,
    wake: Arc<Wake>,
    jobs: JobSequence,
    entry: CoordinatorEntryOwner,
    host: IncidentHost,
}
fn prepared() -> TestResult<Rig> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, &client, &scope, &wake, &jobs)?;
    let until = namespace.original_preparation().until();
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    host.submit_prepare(&client, &scope, &jobs, until, &mut input)?;
    captured(&mut host, &client, &wake)?;
    Ok(Rig { execution, client, scope, wake, jobs, entry, host })
}
fn policy(host: &mut IncidentHost) -> TestResult<&mut FundedPolicy> {
    match host.completion.as_mut().map(|done| &mut done.outcome) {
        Some(JobOutcome::Success(controller)) => Ok(&mut controller.policy),
        _ => Err("Actual admitted constructor missing".into()),
    }
}
/// Park original metadata copied from a genuine central batch; no terminal is manufactured.
fn notice(
    client: &ExecutionClient,
    wake: &Wake,
    scope: ScopeId,
    job: JobId,
) -> TestResult<CompletionBatch> {
    delivery(client, wake, |batch| {
        let notifications: Vec<_> = batch
            .notifications
            .iter()
            .copied()
            .filter(|notice| (notice.scope_id, notice.job_id) == (scope, job))
            .collect();
        Ok((!notifications.is_empty()).then_some(CompletionBatch { notifications, remaining: 0 }))
    })
}
fn reordered_policy_loan(closing: bool) -> TestResult {
    let expected_terminal = if closing { TerminalKind::Cancelled } else { TerminalKind::Success };
    let mut rig = prepared()?;
    let original = policy(&mut rig.host)?
        .state
        .as_ref()
        .map(|funded| std::ptr::from_ref(&*funded.state))
        .ok_or("Original policy absent")?;
    let gate = Arc::new(Gate::default());
    let _release = gate.guard();
    let external = gate.clone();
    let mut probe = spec(rig.jobs.allocate()?);
    probe.reservation_bytes = 4096;
    {
        let policy = policy(&mut rig.host)?;
        policy.collection.submit_policy_probe(
            &rig.client,
            &rig.scope,
            probe,
            &mut policy.state,
            move || external.hold(),
        )?;
        assert!(policy.state.is_none());
        assert!(policy.collection.policy_pending());
    }
    gate.wait_entered();
    let independent = spec(rig.jobs.allocate()?);
    let mut other =
        rig.client.submit_blocking(&rig.scope, independent, |_| Ok::<_, std::io::Error>(()))?;
    let foreign = notice(&rig.client, &rig.wake, rig.scope.id(), independent.id)?;
    let actual = other.try_take_completion().ok_or("Actual independent completion missing")?;
    assert_eq!(actual.outcome.kind(), TerminalKind::Success);
    assert!(!rig.host.capture_collection(&foreign)?);
    if closing {
        rig.host.request_close(ShutdownBudget::new(Moment::from_nanos(0))?);
        assert_eq!(rig.host.next_effect().err(), Some(HostIssue::Closed));
    } else {
        assert!(rig.host.next_effect()?.is_none());
        assert_eq!(rig.host.take_effect().err(), Some(HostIssue::Pending));
        assert_eq!(rig.host.vacant().err(), Some(HostIssue::Pending));
    }
    assert!(rig.host.collection_outcome().is_none());
    assert!(!rig.host.collection_files_retired());
    gate.release();
    let parked = notice(&rig.client, &rig.wake, rig.scope.id(), probe.id)?;
    assert!(policy(&mut rig.host)?.state.is_none());
    assert!(policy(&mut rig.host)?.collection.policy_pending());
    assert!(
        !rig.host
            .capture_collection(&CompletionBatch { notifications: Vec::new(), remaining: 0 })?
    );
    assert!(rig.host.collection_outcome().is_none());
    if !closing {
        assert!(rig.host.next_effect()?.is_none());
    }
    assert_eq!(
        policy(&mut rig.host)?.collection.capture_policy_probe_terminal(&parked)?,
        Some(probe)
    );
    assert_eq!(rig.host.collection_outcome(), Some(expected_terminal));
    assert!(policy(&mut rig.host)?.state.is_none());
    assert!(policy(&mut rig.host)?.collection.policy_pending());
    assert!(
        !rig.host
            .capture_collection(&CompletionBatch { notifications: Vec::new(), remaining: 0 })?
    );
    if closing {
        assert_eq!(rig.host.next_effect().err(), Some(HostIssue::Closed));
    } else {
        assert!(rig.host.next_effect()?.is_none());
        assert_eq!(rig.host.take_effect().err(), Some(HostIssue::Pending));
    }
    {
        let policy = policy(&mut rig.host)?;
        policy.collection.restore_policy_probe(&mut policy.state)?;
    }
    assert_eq!(rig.host.collection_outcome(), Some(expected_terminal));
    assert!(!policy(&mut rig.host)?.collection.policy_pending());
    assert_eq!(
        policy(&mut rig.host)?.state.as_ref().map(|held| std::ptr::from_ref(&*held.state)),
        Some(original)
    );
    assert!(!rig.host.collection_files_retired());
    assert!(!rig.host.capture_collection(&parked)?);
    assert_eq!(probe.deadline, Some(Moment::from_nanos(30_000_000_000)));
    if closing {
        assert_eq!(rig.host.next_effect().err(), Some(HostIssue::Closed));
    } else {
        assert!(rig.host.next_effect()?.is_none());
        assert!(rig.host.take_effect()?.is_none());
    }
    drop(actual);
    drop(other);
    retire_entry(&mut rig.entry, &rig.client, &rig.scope, &rig.wake, &rig.jobs)?;
    drop(rig.entry);
    drop(rig.execution);
    assert_eq!(rig.scope.snapshot().reserved_bytes, CONTROLLER_BYTES + 4096);
    drop(rig.host);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_service_policy_loan_waits_through_unrelated_and_parked_completion_until_same_restore()
-> TestResult {
    reordered_policy_loan(false)
}
#[test]
fn actual_service_policy_loan_does_not_reopen_closed_host_before_or_after_restore() -> TestResult {
    reordered_policy_loan(true)
}
#[test]
fn unaccounted_missing_policy_remains_strict_without_a_real_source_loan() -> TestResult {
    let mut rig = prepared()?;
    assert!(!policy(&mut rig.host)?.collection.policy_pending());
    let original = policy(&mut rig.host)?.state.take();
    assert!(original.is_some());
    assert_eq!(rig.host.next_effect().err(), Some(HostIssue::Pending));
    assert_eq!(rig.host.take_effect().err(), Some(HostIssue::Pending));
    assert_eq!(rig.host.vacant().err(), Some(HostIssue::Pending));
    assert_eq!(
        rig.host
            .capture_collection(&CompletionBatch { notifications: Vec::new(), remaining: 0 })
            .err(),
        Some(HostIssue::Pending)
    );
    policy(&mut rig.host)?.state = original;
    assert!(rig.host.next_effect()?.is_none());
    retire_entry(&mut rig.entry, &rig.client, &rig.scope, &rig.wake, &rig.jobs)?;
    drop(rig.entry);
    drop(rig.execution);
    assert_eq!(rig.scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(rig.host);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
