// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unavailable actual authority without minting a coordinator key or lease.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{Clock, CompletionWake, ExecutionError, WaitFuture};
use sugarsubstitute_execution::ExecutionOwner;
#[path = "storage_tests.rs"]
pub(crate) mod storage_tests;
#[derive(Default)]
pub(super) struct TestClock(AtomicU64);
impl TestClock {
    /// Advance the injected external clock without changing any owner's transaction.
    pub(super) fn set(&self, now: Moment) {
        self.0.store(now.as_nanos(), Ordering::Release);
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
pub(super) struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        Ok(())
    }
}
pub(super) fn key() -> Result<CoordinatorKey> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
/// Matching data populates pure policy only; every tested native boundary receives
/// an actual inert authority and must reject it before admission.
pub(super) fn unadmitted_policy() -> Result<RetainedBackend> {
    let owner = key()?;
    let storage = storage_tests::claim(RetainedBackend::required_storage_bytes())?;
    let mut records = Vec::new();
    records.try_reserve_exact(64)?;
    records.resize_with(64, || None);
    let backend = CoordinatorBackend {
        custody_options: None,
        custody: None,
        owner,
        policy: ResourceCoordinator::new(owner),
        records: records.into_boxed_slice(),
        next_shutdown: 0,
        undelivered: None,
    };
    Ok(RetainedBackend { backend: Box::new(backend), _storage: storage })
}
#[test]
fn inert_authority_cannot_construct_backend_driver() -> Result<()> {
    let original =
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?;
    let authority = CoordinatorAuthority::new(original)?;
    let storage = storage_tests::claim(RetainedBackend::required_storage_bytes())?;
    assert!(RetainedBackend::new(&authority, storage).is_err());
    assert!(authority.key().is_none());
    assert!(authority.registry().is_none());
    assert!(!authority.serving());
    Ok(())
}

#[test]
fn failed_resource_diagnostic_borrows_the_original_source_cause_after_policy_adoption() -> Result<()>
{
    use super::failure::{FailureKind, FailureObservation};
    let mut retained = unadmitted_policy()?;
    let owner = retained.backend.as_mut();
    let run =
        RunKey { owner: key()?, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let connection = ConnectionId::new(1)?;
    let peer_registration = ClientRegistration {
        run,
        connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("Diagnostic_1".into())?,
    };
    owner.policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    owner.register_run(peer_registration)?;
    let transition = owner.policy.request(
        ClientBinding { run, connection },
        ResourceRequest {
            run,
            connection,
            id: RequestId::new(1)?,
            command: ResourceCommand::Acquire(BackendTarget::Attached(LoopbackOrigin::parse(
                "http://127.0.0.1:8188",
                8188,
            )?)),
        },
    )?;
    let Some(ResourceEffect::Prepare { resource, .. }) = transition.effects.first() else {
        return Err(Box::new(LifecycleRejection::WrongPhase));
    };
    let resource = *resource;
    let relative = std::path::PathBuf::from("private-relative-source");
    let cause = match FrontendInstallationInput::new(
        relative.clone(),
        relative.clone(),
        relative.clone(),
        relative,
    ) {
        Err(cause) => Box::new(cause),
        Ok(_) => return Err(Box::new(LifecycleRejection::WrongPhase)),
    };
    let original = std::ptr::from_ref(cause.as_ref());
    let failure = LifecycleFailure::new(LifecycleFailureCode::Preparation, cause);
    owner.begin_delivery()?;
    owner.observe(ResourceObservation::Failed { resource, failure })?;
    let retained_cause = owner.failure(resource).ok_or(LifecycleRejection::WrongPhase)?;
    let observation = FailureObservation::retained(retained_cause);
    assert_eq!(observation.kind, FailureKind::Source);
    assert_eq!(observation.stage, "frontend_location_shape");
    assert_eq!(
        retained_cause
            .source()
            .and_then(|cause| cause.downcast_ref::<FrontendSourceError>())
            .map(std::ptr::from_ref),
        Some(original)
    );
    let pending = owner.take_pending_transition().ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(pending.replies.len(), 1);
    assert_eq!(pending.replies[0].outcome, RequestOutcome::Failed);
    assert_eq!(
        FailureObservation::retained(
            owner.failure(resource).ok_or(LifecycleRejection::WrongPhase)?
        ),
        observation
    );
    Ok(())
}

#[test]
fn backend_failure_diagnostics_classify_campaign_without_formatting_unknown_causes() {
    use super::failure::{FailureKind, FailureObservation};
    use sugarsubstitute_comfy::readiness_campaign::{CampaignError, CampaignRejection};
    let campaign = LifecycleFailure::new(
        LifecycleFailureCode::Preparation,
        Box::new(CampaignError::Rejected(CampaignRejection::WrongPhase)),
    );
    assert_eq!(
        FailureObservation::retained(&campaign),
        FailureObservation::classified(FailureKind::Campaign, "campaign_rejected")
    );
    let unknown = LifecycleFailure::new(
        LifecycleFailureCode::Preparation,
        Box::new(std::io::Error::other("private/path/response/secret")),
    );
    let observation = FailureObservation::retained(&unknown);
    assert_eq!(observation, FailureObservation::classified(FailureKind::Other, "unclassified"));
    assert!(!format!("{observation:?}").contains("secret"));
}

#[test]
fn unavailable_authority_preserves_input_and_both_identity_sequences() -> Result<()> {
    let executor = ExecutionOwner::new(Arc::new(TestClock::default()), Arc::new(Wake))?;
    let client = executor.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    let mut retained = unadmitted_policy()?;
    let owner = retained.backend.as_mut();
    let run =
        RunKey { owner: key()?, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let connection = ConnectionId::new(1)?;
    let peer_registration = ClientRegistration {
        run,
        connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("Backend_1".into())?,
    };
    owner.policy.enroll_run(sugarsubstitute_contracts::lifecycle::RunEnrollment {
        run: peer_registration.run,
        launch: peer_registration.launch,
        profile: peer_registration.profile.clone(),
    })?;
    owner.register_run(peer_registration)?;
    let target = BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?);
    let path = std::path::PathBuf::from(r"D:\SugarSubstitute-Rust\artifacts\unopened-input");
    let mut input = Some(BackendPreparation {
        source: FrontendInstallationInput::new(path.clone(), path.clone(), path.clone(), path)?,
        environment: None,
    });
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    assert!(
        owner
            .request(
                ClientBinding { run, connection },
                ResourceRequest {
                    run,
                    connection,
                    id: RequestId::new(1)?,
                    command: ResourceCommand::Acquire(target),
                },
                Some(ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?),
                &mut input,
                &mut resources
            )
            .is_err()
    );
    assert!(input.is_some());
    assert!(
        owner
            .disconnect(
                run,
                connection,
                ShutdownBudget::new(Moment::from_nanos(0))?,
                &mut resources
            )
            .is_err()
    );
    assert_eq!(owner.policy.snapshot().resources, 0);
    assert_eq!(owner.policy.snapshot().leases, 0);
    assert_eq!(jobs.allocate()?.get(), 1);
    assert_eq!(scopes.allocate()?.get(), 3);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert!(owner.take_pending_transition().is_none());
    let stale = ResourceKey {
        owner: key()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    assert!(owner.readiness(stale).is_none());
    assert!(owner.ready_origin(stale)?.is_none());
    assert!(owner.take_output(stale).is_err());
    assert!(owner.cleanup_failure(stale).is_none());
    assert!(owner.failure(stale).is_none());
    assert!(owner.next_deadline().is_none());
    assert!(owner.resource_keys().iter().all(Option::is_none));
    assert!(owner.settled());
    assert!(
        owner
            .activity(
                stale,
                &CompletionBatch { notifications: vec![], remaining: 0 },
                &mut resources
            )
            .is_err()
    );
    let (transition, forwarded) =
        owner.claim_observation(ClaimObservation::NativeClaimsClosed, &mut resources)?;
    assert!(transition.replies.is_empty());
    assert!(transition.effects.is_empty());
    assert!(matches!(forwarded, Some(ClaimObservation::NativeClaimsClosed)));
    drop(retained);
    drop(authority);
    drop(cleanup);
    drop(native);
    drop(executor);
    Ok(())
}

#[test]
fn pending_actual_transition_blocks_mutation_until_original_replies_transfer() -> Result<()> {
    let mut retained = unadmitted_policy()?;
    let owner = retained.backend.as_mut();
    owner.begin_delivery()?;
    let run =
        RunKey { owner: key()?, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let reply = ResourceReply { run, request: RequestId::new(3)?, outcome: RequestOutcome::Failed };
    owner.retain_transition(&ResourceTransition { replies: vec![reply], effects: vec![] });
    assert!(owner.begin_delivery().is_err());
    let actual = owner.take_pending_transition().ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(actual.replies, vec![reply]);
    assert!(owner.take_pending_transition().is_none());
    owner.begin_delivery()?;
    assert!(owner.take_pending_transition().is_some());
    Ok(())
}

#[test]
fn partial_backend_cleanup_keeps_actual_parent_budget_across_later_clock_and_repeat() -> Result<()>
{
    let clock = Arc::new(TestClock::default());
    clock.set(Moment::from_nanos(2_000_000_000));
    let executor = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let client = executor.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let scope =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, Some(&native))?;
    let jobs = JobSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    let mut retained = unadmitted_policy()?;
    let owner = retained.backend.as_mut();
    let key = ResourceKey {
        owner: key()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let target = BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?);
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?;
    let path =
        std::path::PathBuf::from(r"D:\SugarSubstitute-Rust\artifacts\unopened-partial-source");
    // Actual inert owners represent admitted construction before any native job/output.
    owner.records[0] = Some(Backend {
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
    });
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let original = ShutdownBudget::new(Moment::from_nanos(1_000_000_000))?;
    let transition = owner.begin_cleanup(key, Some(original), &mut resources)?;
    assert!(transition.replies.is_empty() && transition.effects.is_empty());
    let actual = owner.record(key)?.shutdown.ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(actual.1, original);
    assert!(owner.record(key)?.scope.snapshot().cancellation_requested);
    clock.set(Moment::from_nanos(40_000_000_000));
    let transition = owner.begin_cleanup(
        key,
        Some(ShutdownBudget::new(Moment::from_nanos(40_000_000_000))?),
        &mut resources,
    )?;
    assert!(transition.replies.is_empty() && transition.effects.is_empty());
    assert_eq!(owner.record(key)?.shutdown, Some(actual));
    assert_eq!(owner.next_deadline(), Some(original.graceful_until));
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(jobs.allocate()?.get(), 1);
    Ok(())
}
