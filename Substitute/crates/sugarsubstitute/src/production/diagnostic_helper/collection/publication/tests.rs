// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual collection publication after an unavailable external capture observation.
use super::*;
use sugarsubstitute_application::supervision::{DesktopExitTransaction, ResourceCoordinator};
use sugarsubstitute_contracts::{configuration::ProfileId, lifecycle::*, startup::LaunchId};
use sugarsubstitute_supervision::diagnostic_storage::fixture::DiagnosticStorageFixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Admit collection using the real policy and an external failed-process observation.
fn collecting() -> TestResult<(IncidentCoordinator, IncidentSeed, IncidentOperationScope)> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let mut resources = ResourceCoordinator::new(owner);
    let profile = ProfileId::new("collection-host-failure".into())?;
    resources.enroll_run(RunEnrollment {
        run,
        launch: LaunchId::new(1)?,
        profile: profile.clone(),
    })?;
    resources.register_run(ClientRegistration {
        run,
        launch: LaunchId::new(1)?,
        profile,
        connection: ConnectionId::new(1)?,
    })?;
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("collection-host-failure")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::DesktopInitialization,
    )?;
    let mut coordinator = IncidentCoordinator::new(owner);
    let request = IncidentRequestContext::new(
        IncidentOperationBudget::new(Moment::from_nanos(0))?,
        seed.created(),
    );
    let effects = coordinator.seed(&mut resources, seed.clone(), request)?;
    let Some(IncidentEffect::PublishSeed { scope, .. }) = effects.iter().next() else {
        return Err("Seed publication missing".into());
    };
    coordinator
        .observe(IncidentServiceObservation::SeedCommitted { scope: *scope, seed: seed.clone() })?;
    let process = ProcessKey { owner, id: ProcessId::new(1)? };
    let mut exit = DesktopExitTransaction::new(run, process)?;
    exit.observe(DesktopObservation::Exited { process, code: 1 })
        .map_err(|rejected| rejected.reason)?;
    let effects = coordinator.failed_desktop(
        &mut resources,
        exit.failed_incident()?,
        FailureEvidence::Unexplained,
        IncidentRequestContext::new(
            IncidentOperationBudget::new(Moment::from_nanos(0))?,
            IncidentUtc::from_unix_seconds(101)?,
        ),
    )?;
    let Some(IncidentEffect::CollectEvidence { scope, .. }) = effects.iter().next() else {
        return Err("Collection admission missing".into());
    };
    Ok((coordinator, seed, *scope))
}

#[test]
fn unavailable_capture_with_host_failure_commits_without_entering_native_dump_disposition()
-> TestResult {
    crate::production::incident_host::tests::admitted_value(DIAGNOSTIC_STORAGE_BYTES, |context| {
        let (mut coordinator, seed, scope) = collecting()?;
        let fixture = DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(scope.operation(), &context)?;
        let opening = fixture.resolve_opening(&mut storage, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut repository = IncidentRepository::new(storage, operation, &context)?;
        let DiagnosticStoreOpening::Created(mut genesis) = opening else {
            return Err("Fresh fixture namespace was not created".into());
        };
        repository.initialize(&mut genesis, &context)?;
        repository.publish_seed(seed.clone(), &context)?;
        let host_failure = HostFailureFact::new(
            "main_frontend_effect",
            "None",
            HostFailureCategory::Io,
            Some("Io"),
            None,
            1,
            1,
        )?;
        let mut collection = CollectionOperation::new();
        collection.repository = Some(repository);
        collection.scope = Some(scope);
        let mut effects = std::array::from_fn(|_| None);
        collection.publish_evidence(
            CollectionEvidence {
                seed: seed.clone(),
                scope,
                occurred: IncidentUtc::from_unix_seconds(101)?,
                capture: CaptureEvidence::Unavailable(CaptureAbsence::Missing),
                dump_bytes: 0,
                host_failure: Some(host_failure.clone()),
            },
            &mut coordinator,
            &mut effects,
            &context,
        )?;
        assert_eq!(collection.stage, CollectionStage::Committed);
        assert!(collection.failure().is_none());
        assert!(collection.report.is_none());
        assert!(!collection.source_settled);
        assert!(
            effects
                .iter()
                .flatten()
                .any(|effect| matches!(effect, IncidentEffect::RetireSource { scope: IncidentSourceScope::Live(actual) } if *actual == scope))
        );
        let committed = collection.committed.as_ref().ok_or("Collection commit missing")?;
        assert_eq!(
            committed.summary().facts().capture(),
            CaptureEvidence::Unavailable(CaptureAbsence::Missing)
        );
        let PayloadDisposition::Retained(payload) = committed.summary().facts().payload() else {
            return Err("Host fault was not retained".into());
        };
        assert_eq!(
            payload.entries().map(|fact| fact.kind()).collect::<Vec<_>>(),
            [AttachmentKind::Fault]
        );
        let repository = collection.repository.as_mut().ok_or("Repository missing")?;
        let current = repository.load(seed.key(), &context)?;
        assert_eq!(repository.read_host_failure(&current, &context)?, Some(host_failure));
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(collection)
    })
}
