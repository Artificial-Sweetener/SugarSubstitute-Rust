// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject altered preparation intervals and keep creator versus joiner policy independent.
use super::*;
use sugarsubstitute_application::startup::ProbeBudgetError;
use sugarsubstitute_contracts::{configuration::ProfileId, startup::LaunchId};
fn target() -> Result<BackendTarget> {
    Ok(BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?))
}
fn run(owner: CoordinatorKey, instance: u64) -> Result<RunKey> {
    Ok(RunKey { owner, instance: InstanceId::new(instance)?, generation: RunGeneration::new(1)? })
}
fn registration(run: RunKey) -> Result<ClientRegistration> {
    Ok(ClientRegistration {
        run,
        launch: LaunchId::new(run.instance.get())?,
        profile: ProfileId::new(format!("cutoff{}", run.instance.get()))?,
        connection: ConnectionId::new(run.instance.get())?,
    })
}
fn request(run: RunKey, id: u64, command: ResourceCommand) -> Result<ResourceRequest> {
    Ok(ResourceRequest {
        run,
        connection: ConnectionId::new(run.instance.get())?,
        id: RequestId::new(id)?,
        command,
    })
}
fn submit(
    policy: &mut ResourceCoordinator,
    request: ResourceRequest,
) -> Result<ResourceTransition> {
    Ok(policy
        .request(ClientBinding { run: request.run, connection: request.connection }, request)?)
}
fn lease(transition: &ResourceTransition) -> Result<LeaseKey> {
    match transition.replies.first().map(|r| r.outcome) {
        Some(RequestOutcome::Pending(lease) | RequestOutcome::Granted(lease)) => Ok(lease),
        _ => Err(Box::new(LifecycleRejection::WrongPhase)),
    }
}

#[test]
fn absent_foreign_kind_and_expired_budget_reject_before_policy_mutation() -> Result<()> {
    let now = Moment::from_nanos(10);
    let command = ResourceCommand::Acquire(target()?);
    let cause = CoordinatorBackend::check_request_budget(&command, None, now)
        .err()
        .ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(cause.downcast_ref::<LifecycleRejection>(), Some(&LifecycleRejection::WrongPhase));
    let wrong = ConnectionBudget::new(ConnectionBudgetKind::Managed, now)?;
    assert!(CoordinatorBackend::check_request_budget(&command, Some(wrong), now).is_err());
    let original =
        ConnectionBudget::from_until(ConnectionBudgetKind::Attached, now, Moment::from_nanos(20))?;
    CoordinatorBackend::check_request_budget(&command, Some(original), Moment::from_nanos(19))?;
    let cause =
        CoordinatorBackend::check_request_budget(&command, Some(original), original.until())
            .err()
            .ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(cause.downcast_ref::<ProbeBudgetError>(), Some(&ProbeBudgetError::Expired));
    assert_eq!(original.until(), Moment::from_nanos(20));
    Ok(())
}
/// Represent the real retained creator before any source/native effect; no authority is minted.
pub(super) fn creator(
    key: ResourceKey,
    target: BackendTarget,
    budget: ConnectionBudget,
    scope: Scope,
) -> Result<Backend> {
    let path =
        std::path::PathBuf::from(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source");
    Ok(Backend {
        key,
        target: target.clone(),
        budget,
        resolved: None,
        ready_origin: None,
        scope,
        source: InstalledFrontendOwner::new(FrontendInstallationInput::new(
            path.clone(),
            path.clone(),
            path.clone(),
            path,
        )?),
        source_job: false,
        source_lease: None,
        source_close: None,
        source_terminal: None,
        source_closed: true,
        environment: None,
        resolution: None,
        resolution_pending: false,
        resolution_retiring: false,
        claim_pending: false,
        backend_admitted: false,
        claimed: false,
        release_pending: false,
        released: false,
        launch: None,
        started: false,
        family_settled: false,
        output: None,
        output_terminal: None,
        output_observed: false,
        campaign: ReadinessCampaign::new(target, budget)?,
        readiness: None,
        shutdown: None,
        failure_published: false,
        settled: false,
        cleanup_failure: cleanup_state::CleanupState::new(),
    })
}
#[test]
fn existing_shared_join_and_replay_emit_no_fresh_creator_preparation() -> Result<()> {
    let executor = sugarsubstitute_execution::ExecutionOwner::new(
        std::sync::Arc::new(tests::TestClock::default()),
        std::sync::Arc::new(tests::Wake),
    )?;
    let client = executor.client();
    let scopes = ScopeSequence::new();
    let jobs = JobSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let budget = ConnectionBudget::from_until(
        ConnectionBudgetKind::Attached,
        Moment::from_nanos(10),
        Moment::from_nanos(100),
    )?;
    let scope = client.create_scope(
        ScopeSpec { id: scopes.allocate()?, deadline: Some(budget.until()) },
        Some(&native),
    )?;
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    let mut retained = tests::unadmitted_policy()?;
    let backend = retained.backend.as_mut();
    let owner = tests::key()?;
    let first = run(owner, 1)?;
    let joining = run(owner, 2)?;
    let peer_registration = registration(first)?;
    backend.policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    backend.policy.register_run(peer_registration)?;
    let peer_registration = registration(joining)?;
    backend.policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    backend.policy.register_run(peer_registration)?;
    let creator_transition =
        submit(&mut backend.policy, request(first, 1, ResourceCommand::Acquire(target()?))?)?;
    assert_eq!(creator_transition.effects.len(), 1);
    let held = lease(&creator_transition)?;
    backend.records[0] = Some(creator(held.resource, target()?, budget, scope)?);
    let original_deadline = backend.next_deadline();
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let joining_request = request(joining, 1, ResourceCommand::Acquire(target()?))?;
    for until in [50, 150] {
        let joiner_budget = ConnectionBudget::from_until(
            ConnectionBudgetKind::Attached,
            Moment::from_nanos(20),
            Moment::from_nanos(until),
        )?;
        CoordinatorBackend::check_request_budget(
            &joining_request.command,
            Some(joiner_budget),
            Moment::from_nanos(20),
        )?;
        let mut transition = submit(&mut backend.policy, joining_request.clone())?;
        assert!(transition.effects.is_empty());
        assert_eq!(lease(&transition)?.resource, held.resource);
        backend.effects(&mut transition, &mut None, Some(joiner_budget), None, &mut resources)?;
        assert_eq!(backend.record(held.resource)?.budget, budget);
        assert_eq!(backend.record(held.resource)?.scope.spec().deadline, Some(budget.until()));
        assert_eq!(backend.next_deadline(), original_deadline);
        assert!(backend.record(held.resource)?.launch.is_none());
        assert!(!backend.record(held.resource)?.source_job);
    }
    let mut cancelled = backend.policy.disconnect_run(joining, ConnectionId::new(2)?)?;
    assert!(cancelled.effects.is_empty());
    backend.effects(
        &mut cancelled,
        &mut None,
        None,
        Some(ShutdownBudget::new(Moment::from_nanos(20))?),
        &mut resources,
    )?;
    assert_eq!(backend.record(held.resource)?.budget, budget);
    assert!(backend.record(held.resource)?.shutdown.is_none());
    assert_eq!(backend.policy.snapshot().leases, 1);
    assert_eq!(jobs.allocate()?.get(), 1);
    assert_eq!(scopes.allocate()?.get(), 4);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn reload_keeps_ready_shared_lease_without_a_preparation_budget() -> Result<()> {
    let owner = tests::key()?;
    let run = run(owner, 1)?;
    let mut policy = ResourceCoordinator::new(owner);
    let peer_registration = registration(run)?;
    policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    policy.register_run(peer_registration)?;
    let held = lease(&submit(&mut policy, request(run, 1, ResourceCommand::Acquire(target()?))?)?)?;
    policy
        .observe(ResourceObservation::Ready {
            resource: held.resource,
            origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        })
        .map_err(|rejected| rejected.reason)?;
    let command = ResourceCommand::Reload(held);
    CoordinatorBackend::check_request_budget(&command, None, Moment::from_nanos(1000))?;
    assert!(
        CoordinatorBackend::check_request_budget(
            &command,
            Some(ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?),
            Moment::from_nanos(1000)
        )
        .is_err()
    );
    let transition = submit(&mut policy, request(run, 2, command)?)?;
    assert!(transition.effects.is_empty());
    assert_eq!(transition.replies[0].outcome, RequestOutcome::Reloaded(held));
    assert_eq!(policy.snapshot().leases, 1);
    Ok(())
}
