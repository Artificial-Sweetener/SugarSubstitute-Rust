// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real recovery policy and inert cleanup owners with external registry facts.
//! No process, native watch, launch ticket, readiness source or family proof is fabricated.
use super::{recovery::RecoverableBackendFailure, *};
use std::sync::Arc;
use sugarsubstitute_application::supervision::InvalidatedLeaseState;

pub(super) struct Fixture {
    pub(super) held: RetainedBackend,
    client: ExecutionClient,
    native: Scope,
    cleanup: Scope,
    jobs: JobSequence,
    scopes: ScopeSequence,
    authority: CoordinatorAuthority,
    pub(super) leases: [LeaseKey; 2],
    pub(super) process: ProcessKey,
    _executor: sugarsubstitute_execution::ExecutionOwner,
}
impl Fixture {
    pub(super) fn new(managed: bool) -> Result<Self> {
        let executor = sugarsubstitute_execution::ExecutionOwner::new(
            Arc::new(tests::TestClock::default()),
            Arc::new(tests::Wake),
        )?;
        let client = executor.client();
        let scopes = ScopeSequence::new();
        let native = client
            .create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
        let scope = client
            .create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, Some(&native))?;
        let mut held = tests::unadmitted_policy()?;
        let backend = held.backend.as_mut();
        let target = if managed {
            BackendTarget::Managed(
                sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
                    r"E:\unopened-recovery".into(),
                    r"E:\unopened-recovery\python.exe".into(),
                    vec!["--original".into()],
                    8188,
                )?,
            )
        } else {
            BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?)
        };
        let mut leases = Vec::new();
        for instance in 1..=2 {
            let run = RunKey {
                owner: tests::key()?,
                instance: InstanceId::new(instance)?,
                generation: RunGeneration::new(1)?,
            };
            let connection = ConnectionId::new(instance)?;
            let launch = sugarsubstitute_contracts::startup::LaunchId::new(instance)?;
            let profile = sugarsubstitute_contracts::configuration::ProfileId::new(format!(
                "recovery{instance}"
            ))?;
            backend.policy.enroll_run(RunEnrollment { run, launch, profile: profile.clone() })?;
            backend.policy.register_run(ClientRegistration { run, launch, profile, connection })?;
            let produced = backend.policy.request(
                ClientBinding { run, connection },
                ResourceRequest {
                    run,
                    connection,
                    id: RequestId::new(1)?,
                    command: ResourceCommand::Acquire(target.clone()),
                },
            )?;
            let Some(RequestOutcome::Pending(lease)) = produced.replies.first().map(|r| r.outcome)
            else {
                return Err("Original policy association absent".into());
            };
            leases.push(lease);
        }
        let leases: [LeaseKey; 2] = leases.try_into().map_err(|_| "Original cohort count")?;
        let key = leases[0].resource;
        let process = ProcessKey { owner: tests::key()?, id: ProcessId::new(7)? };
        if managed {
            backend.observe(ResourceObservation::ClaimAcquired(key))?;
            backend.observe(ResourceObservation::OwnedStarted { resource: key, process })?;
        }
        backend.observe(ResourceObservation::Ready {
            resource: key,
            origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        })?;
        let kind =
            if managed { ConnectionBudgetKind::Managed } else { ConnectionBudgetKind::Attached };
        backend.records[0] = Some(cutoff_tests::creator(
            key,
            target,
            ConnectionBudget::new(kind, Moment::from_nanos(0))?,
            scope,
        )?);
        Ok(Self {
            held,
            client,
            native,
            cleanup,
            jobs: JobSequence::new(),
            scopes,
            authority: CoordinatorAuthority::new(
                sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
                    Moment::from_nanos(0),
                )?,
            )?,
            leases,
            process,
            _executor: executor,
        })
    }
    /// Install an actual unopened source/finite scope for a genuinely App-admitted
    /// successor. This supplies no launch ticket, watch or VerifiedReadiness.
    pub(super) fn install_inert_successor(&mut self, key: ResourceKey) -> Result<()> {
        let target = self.held.backend.record(self.leases[0].resource)?.target.clone();
        let budget = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
        let scope = self.client.create_scope(
            ScopeSpec { id: self.scopes.allocate()?, deadline: Some(budget.until()) },
            Some(&self.native),
        )?;
        self.held.backend.records[1] = Some(cutoff_tests::creator(key, target, budget, scope)?);
        Ok(())
    }
    pub(super) fn binding(&self) -> ClientBinding {
        ClientBinding { run: self.leases[0].run, connection: self.leases[0].connection }
    }
    pub(super) fn exit(&mut self) -> Result<ResourceTransition> {
        let backend = self.held.backend.as_mut();
        backend.begin_delivery()?;
        backend.managed_exit_observation(self.leases[0].resource, self.process, self.process, 0)
    }
    pub(super) fn retire_inert(&mut self) -> Result<()> {
        // These admitted capsules never opened any native/source/output. Their actual
        // empty cleanup is useful owner coverage, not live managed-family qualification.
        let mut resources = BackendResources {
            client: &self.client,
            native: &self.native,
            cleanup: &self.cleanup,
            jobs: &self.jobs,
            scopes: &self.scopes,
            authority: &mut self.authority,
            capture: None,
        };
        let backend = self.held.backend.as_mut();
        backend.begin_cleanup(
            self.leases[0].resource,
            Some(ShutdownBudget::new(Moment::from_nanos(0))?),
            &mut resources,
        )?;
        backend.cleanup(0, &mut resources)?;
        Ok(())
    }
}

#[test]
fn same_process_exit_preserves_full_cohort_and_unexpected_exit_cause_before_cleanup() -> Result<()>
{
    let mut fixture = Fixture::new(true)?;
    let key = fixture.leases[0].resource;
    let other = ProcessKey { id: ProcessId::new(8)?, ..fixture.process };
    assert!(fixture.held.backend.managed_exit_observation(key, fixture.process, other, 3).is_err());
    assert!(fixture.held.backend.failure(key).is_none());
    let transition = fixture.exit()?;
    assert_eq!(transition.invalidations.len(), 2);
    for (notice, lease) in transition.invalidations.iter().zip(fixture.leases) {
        assert_eq!(notice.lease, lease);
        assert_eq!(notice.status, 0);
    }
    assert_eq!(
        fixture.held.backend.failure(key).ok_or("Original exit absent")?.code(),
        LifecycleFailureCode::UnexpectedExit
    );
    assert!(matches!(fixture.held.backend.recoverable_failure(key)?,
        Some(RecoverableBackendFailure::ManagedExited { process, status: 0 }) if process == fixture.process));
    assert!(!fixture.held.backend.attempt_settled(key));
    assert_eq!(
        fixture.held.backend.invalidated_lease(fixture.binding(), fixture.leases[0])?,
        InvalidatedLeaseState::Retiring
    );
    let pending =
        fixture.held.backend.take_pending_transition().ok_or("Original return dropped")?;
    assert_eq!(pending.invalidations, transition.invalidations);
    Ok(())
}

#[test]
fn original_cause_transfer_waits_for_actual_inert_cleanup_and_never_overwrites_slot() -> Result<()>
{
    let mut fixture = Fixture::new(true)?;
    fixture.exit()?;
    let key = fixture.leases[0].resource;
    let pointer = fixture
        .held
        .backend
        .failure(key)
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<RootExit>())
        .map(std::ptr::from_ref)
        .ok_or("Original typed cause absent")?;
    let mut destination = None;
    assert!(!fixture.held.backend.transfer_recovery_failure(key, &mut destination)?);
    assert!(destination.is_none());
    fixture.retire_inert()?;
    assert!(fixture.held.backend.attempt_settled(key));
    destination = Some(LifecycleFailure::new(
        LifecycleFailureCode::Cleanup,
        Box::new(std::io::Error::other("previous retained cause")),
    ));
    assert!(fixture.held.backend.transfer_recovery_failure(key, &mut destination).is_err());
    assert_eq!(
        fixture.held.backend.invalidated_lease(fixture.binding(), fixture.leases[0])?,
        InvalidatedLeaseState::Retiring
    );
    let previous = destination.take().ok_or("Destination was overwritten")?;
    assert_eq!(
        previous.source().ok_or("Previous cause dropped")?.to_string(),
        "previous retained cause"
    );
    assert!(fixture.held.backend.transfer_recovery_failure(key, &mut destination)?);
    let transferred = destination.as_ref().ok_or("Original cause not moved")?;
    assert_eq!(
        transferred
            .source()
            .and_then(|cause| cause.downcast_ref::<RootExit>())
            .map(std::ptr::from_ref),
        Some(pointer)
    );
    assert_eq!(
        fixture.held.backend.invalidated_lease(fixture.binding(), fixture.leases[0])?,
        InvalidatedLeaseState::RetryReady
    );
    assert_eq!(
        fixture.held.backend.record(key)?.shutdown.ok_or("Original cleanup absent")?.1,
        ShutdownBudget::new(Moment::from_nanos(0))?
    );
    Ok(())
}

#[test]
fn cleanup_failure_remains_fatal_and_keeps_original_outage_cause_untransferred() -> Result<()> {
    let mut fixture = Fixture::new(true)?;
    fixture.exit()?;
    let key = fixture.leases[0].resource;
    let failure = Box::new(std::io::Error::from_raw_os_error(5));
    let pointer = std::ptr::from_ref(failure.as_ref());
    fixture.held.backend.records[0]
        .as_mut()
        .ok_or("Original record absent")?
        .cleanup_failure
        .retain(failure);
    assert_eq!(fixture.held.backend.recoverable_failure(key)?, None);
    fixture.retire_inert()?;
    let mut destination = None;
    assert!(fixture.held.backend.transfer_recovery_failure(key, &mut destination).is_err());
    assert!(fixture.held.backend.take_attempt_failure(key).is_err());
    assert!(destination.is_none());
    assert!(fixture.held.backend.failure(key).is_some());
    assert_eq!(
        fixture
            .held
            .backend
            .cleanup_failure(key)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .map(std::ptr::from_ref),
        Some(pointer)
    );
    Ok(())
}

#[test]
fn attached_records_and_wrong_processes_never_authorize_managed_retry() -> Result<()> {
    let mut fixture = Fixture::new(false)?;
    assert!(
        fixture
            .held
            .backend
            .managed_exit_observation(
                fixture.leases[0].resource,
                fixture.process,
                fixture.process,
                1
            )
            .is_err()
    );
    assert!(fixture.held.backend.failure(fixture.leases[0].resource).is_none());
    assert!(fixture.held.backend.invalidated_lease(fixture.binding(), fixture.leases[0]).is_err());
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?;
    assert!(
        CoordinatorBackend::check_request_budget(
            &ResourceCommand::RetryDead(fixture.leases[0]),
            Some(budget),
            Moment::from_nanos(0)
        )
        .is_err()
    );
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
    CoordinatorBackend::check_request_budget(
        &ResourceCommand::RetryDead(fixture.leases[0]),
        Some(budget),
        Moment::from_nanos(0),
    )?;
    assert!(
        CoordinatorBackend::check_request_budget(
            &ResourceCommand::RetryDead(fixture.leases[0]),
            Some(budget),
            budget.until()
        )
        .is_err()
    );
    Ok(())
}
