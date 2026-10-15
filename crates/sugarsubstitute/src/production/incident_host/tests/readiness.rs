// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold actual constructor delivery while an unrelated real completion advances first.
use super::*;

/// Park only genuine constructor metadata while consuming independent real work first.
/// No job outcome, namespace, native proof or readiness flag is manufactured here.
fn reordered_delivery(
    host: &IncidentHost,
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> TestResult<CompletionBatch> {
    let original = host.constructor.as_ref().ok_or("Real constructor not retained")?;
    let identity = (original.scope_id(), original.spec().id);
    let mut specification = spec(jobs.allocate()?);
    specification.reservation_bytes = 4096;
    let mut independent =
        client.submit_blocking(scope, specification, |_| Ok::<_, std::io::Error>(()))?;
    let independent_id = (independent.scope_id(), independent.spec().id);
    let mut parked = Vec::new();
    let mut noticed = false;
    let mut completed = None;
    delivery(client, wake, |batch| {
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == identity {
                parked.push(*notice);
            }
            if (notice.scope_id, notice.job_id) == independent_id {
                noticed = true;
            }
        }
        if noticed && completed.is_none() {
            completed = independent.try_take_completion();
        }
        Ok((completed.is_some() && !parked.is_empty()).then_some(()))
    })?;
    let actual = completed.ok_or("Independent real completion missing")?;
    assert_eq!(actual.outcome.kind(), TerminalKind::Success);
    assert_eq!(parked.len(), 1);
    assert!(host.constructor.is_some());
    assert!(host.preparation_outcome().is_none());
    drop(actual);
    drop(independent);
    Ok(CompletionBatch { notifications: parked, remaining: 0 })
}

#[test]
fn effect_poll_waits_for_actual_constructor_delivery_after_independent_completion() -> TestResult {
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
    let parked = reordered_delivery(&host, &client, &scope, &wake, &jobs)?;
    assert!(!host.prepared());
    assert!(host.failure().is_none());
    assert!(host.next_effect()?.is_none());
    assert_eq!(host.namespace().err(), Some(HostIssue::Unprepared));
    assert_eq!(host.take_effect().err(), Some(HostIssue::Unprepared));
    assert!(!host.capture(&CompletionBatch { notifications: Vec::new(), remaining: 0 })?);
    assert!(host.next_effect()?.is_none());
    assert!(host.capture(&parked)?);
    assert!(host.prepared());
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Success));
    assert!(host.next_effect()?.is_none());
    assert!(host.take_effect()?.is_none());
    assert_eq!(host.namespace()?.execution_owner(), scope.owner_id());
    assert!(host.failure().is_none());
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn pending_poll_does_not_hide_actual_failed_constructor_or_its_original_namespace_cause()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, &client, &scope, &wake, &jobs)?;
    let until = namespace.original_preparation().until();
    entry.close_admission();
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    host.submit_prepare(&client, &scope, &jobs, until, &mut input)?;
    let parked = reordered_delivery(&host, &client, &scope, &wake, &jobs)?;
    assert!(host.next_effect()?.is_none());
    assert_eq!(host.capture(&parked).err(), Some(HostIssue::Preparation));
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Failure));
    assert!(!host.prepared());
    let cause = host.failure().and_then(Error::source).ok_or("Original namespace cause missing")?;
    assert!(cause.is::<EntryError>());
    let original = std::ptr::from_ref(cause);
    assert_eq!(host.next_effect().err(), Some(HostIssue::Unprepared));
    assert_eq!(host.seed_service().err(), Some(HostIssue::Unprepared));
    assert!(std::ptr::eq(
        original,
        std::ptr::from_ref(
            host.failure().and_then(Error::source).ok_or("Original cause replaced")?
        )
    ));
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn never_started_and_closed_constructor_poll_remain_strict_without_forged_readiness() -> TestResult
{
    let mut absent = IncidentHost::new();
    assert_eq!(absent.next_effect().err(), Some(HostIssue::Unprepared));
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
    let parked = reordered_delivery(&host, &client, &scope, &wake, &jobs)?;
    let budget = sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(Moment::from_nanos(0))?;
    host.request_close(budget);
    assert_eq!(host.next_effect().err(), Some(HostIssue::Closed));
    assert!(host.capture(&parked)?);
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Success));
    assert!(!host.prepared());
    assert_eq!(host.next_effect().err(), Some(HostIssue::Closed));
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
