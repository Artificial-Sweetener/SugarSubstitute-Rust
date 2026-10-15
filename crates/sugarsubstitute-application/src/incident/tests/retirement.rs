// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require independent actual completion before reclamation or restart forwarding.
use super::support::*;

#[test]
fn clean_completion_requires_accepted_exit_and_actual_resource_settlement() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let run = run(1)?;
    register(&mut resources, run)?;
    let seed = seed(run)?;
    let scope = seed_ready(&mut coord, &mut resources, seed.clone())?;
    let mut tx = exit(run, Some(CompletionIntent::Normal))?;
    assert_eq!(
        coord.completed_desktop(&tx),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::WrongPhase))
    );
    tx.observe(DesktopObservation::ResourcesSettled { run, shutdown: ShutdownId::new(1)? })
        .map_err(|e| e.reason)?;
    let effects = coord.completed_desktop(&tx)?;
    assert!(
        matches!(effects.iter().next(),Some(IncidentEffect::RetireSource{scope:IncidentSourceScope::Live(actual)}) if *actual==scope)
    );
    assert!(!coord.snapshot(seed.key()).ok_or("No snapshot")?.sources_retired);
    assert_eq!(
        coord.observe(IncidentServiceObservation::SourcesRetired {
            scope: IncidentSourceScope::Live(scope)
        }),
        Err(IncidentDecisionError::WrongPhase)
    );
    coord.observe(IncidentServiceObservation::SeedRetired { scope })?;
    assert_eq!(
        coord.snapshot(seed.key()).ok_or("No snapshot")?.phase,
        IncidentServicePhase::Settled
    );
    Ok(())
}
#[test]
fn cancelled_startup_can_retire_clean_seed_but_unexpected_exit_cannot() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let run = run(1)?;
    register(&mut resources, run)?;
    seed_ready(&mut coord, &mut resources, seed(run)?)?;
    assert!(coord.completed_desktop(&exit(run, None)?).is_err());
    let mut tx = exit(run, Some(CompletionIntent::StartupCancelled))?;
    tx.observe(DesktopObservation::ResourcesSettled { run, shutdown: ShutdownId::new(1)? })
        .map_err(|e| e.reason)?;
    assert_eq!(coord.completed_desktop(&tx)?.len(), 1);
    Ok(())
}
#[test]
fn accepted_clean_exit_with_actual_cleanup_failure_cannot_retire_seed() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let run = run(1)?;
    register(&mut resources, run)?;
    seed_ready(&mut coord, &mut resources, seed(run)?)?;
    let mut tx = exit(run, Some(CompletionIntent::Normal))?;
    tx.observe(DesktopObservation::ResourcesSettled { run, shutdown: ShutdownId::new(1)? })
        .map_err(|e| e.reason)?;
    tx.observe(DesktopObservation::Failed {
        run,
        shutdown: ShutdownId::new(1)?,
        failure: LifecycleFailure::new(
            LifecycleFailureCode::Cleanup,
            Box::new(std::io::Error::other("actual cleanup boundary failure")),
        ),
    })
    .map_err(|e| e.reason)?;
    assert_eq!(
        coord.completed_desktop(&tx),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::WrongPhase))
    );
    assert!(!coord.snapshot(seed(run)?.key()).ok_or("No snapshot")?.sources_retired);
    Ok(())
}
#[test]
fn failed_incident_cannot_be_removed_with_clean_seed_observation() -> TestResult {
    let mut ready = ready(1)?;
    let seed_scope = ready
        .coord
        .record(ready.coord.index(ready.seed.key())?)?
        .seed_scope
        .ok_or("No seed scope")?;
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SeedRetired { scope: seed_scope }),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SourcesRetired {
            scope: IncidentSourceScope::Live(seed_scope)
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    Ok(())
}
#[test]
fn all_ack_source_family_permutations_forward_exactly_one_restart() -> TestResult {
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let mut ready = ready(1)?;
        present(&mut ready)?;
        let mut forwards = 0;
        for (step, item) in order.into_iter().enumerate() {
            let effects = match item {
                0 => acknowledge(&mut ready, true)?,
                1 => ready.coord.observe(IncidentServiceObservation::SourcesRetired {
                    scope: IncidentSourceScope::Live(ready.source),
                })?,
                _ => ready
                    .coord
                    .observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?,
            };
            let count = effects
                .iter()
                .filter(|e| matches!(e, IncidentEffect::ForwardRestartIntent { .. }))
                .count();
            assert_eq!(count, usize::from(step == 2));
            forwards += count;
        }
        assert_eq!(forwards, 1);
        assert!(!ready.coord.record(ready.coord.index(ready.seed.key())?)?.reclaimable());
        ready
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?;
        assert!(ready.coord.record(ready.coord.index(ready.seed.key())?)?.reclaimable());
        assert!(
            ready
                .coord
                .observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?
                .is_empty()
        );
    }
    Ok(())
}
#[test]
fn source_failure_latches_but_actual_late_settlement_does_not_erase_it() -> TestResult {
    let mut ready = ready(1)?;
    ready.coord.observe(IncidentServiceObservation::Failed {
        scope: ready.source,
        failure: IncidentServiceFailure::SourceRetirement,
    })?;
    ready.coord.observe(IncidentServiceObservation::SourcesRetired {
        scope: IncidentSourceScope::Live(ready.source),
    })?;
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.first_failure,
        Some(IncidentServiceFailure::SourceRetirement)
    );
    Ok(())
}
#[test]
fn more_than_128_clean_runs_reclaim_and_m1_eviction_replay_stays_rejected() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    for instance in 1..=200 {
        let run = run(instance)?;
        register(&mut resources, run)?;
        let scope = seed_ready(&mut coord, &mut resources, seed(run)?)?;
        let mut tx = exit(run, Some(CompletionIntent::Normal))?;
        tx.observe(DesktopObservation::ResourcesSettled { run, shutdown: ShutdownId::new(1)? })
            .map_err(|e| e.reason)?;
        coord.completed_desktop(&tx)?;
        coord.observe(IncidentServiceObservation::SeedRetired { scope })?;
        resources.disconnect_run(run, ConnectionId::new(instance)?)?;
        resources.settle_run(run)?;
    }
    assert!(coord.records.iter().flatten().count() <= 128);
    assert_eq!(resources.snapshot().runs, 64);
    assert_eq!(
        coord.seed(&mut resources, seed(run(1)?)?, context(1, 100)?),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::StaleIdentity))
    );
    assert!(register(&mut resources, run(1)?).is_err());
    Ok(())
}
#[test]
fn more_than_128_full_failed_incidents_reclaim_only_after_all_actual_obligations() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    for instance in 1..=200 {
        let run = run(instance)?;
        register(&mut resources, run)?;
        let seed = seed(run)?;
        seed_ready(&mut coord, &mut resources, seed.clone())?;
        let effects = coord.failed_desktop(
            &mut resources,
            exit(run, None)?.failed_incident()?,
            FailureEvidence::Unexplained,
            context(2, 101)?,
        )?;
        let source = match effects.iter().next() {
            Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
            _ => return Err("No collection".into()),
        };
        let effects = coord.observe(IncidentServiceObservation::EvidenceCollected {
            scope: source,
            evidence: evidence()?,
        })?;
        let summary = match effects.iter().next() {
            Some(IncidentEffect::PublishSummary { summary, .. }) => summary.clone(),
            _ => return Err("No summary".into()),
        };
        let effects = coord.observe(IncidentServiceObservation::SummaryCommitted {
            scope: source,
            summary: summary.clone(),
        })?;
        let binding = effects
            .iter()
            .find_map(|e| {
                if let IncidentEffect::LaunchReporter { binding, .. } = e {
                    Some(*binding)
                } else {
                    None
                }
            })
            .ok_or("No reporter")?;
        let mut current = Ready { coord, resources, seed, source, binding, summary };
        present(&mut current)?;
        acknowledge(&mut current, false)?;
        current.coord.observe(IncidentServiceObservation::SourcesRetired {
            scope: IncidentSourceScope::Live(source),
        })?;
        current.coord.observe(IncidentServiceObservation::FamilySettled { run })?;
        assert!(!current.coord.record(current.coord.index(current.seed.key())?)?.reclaimable());
        current
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: current.binding })?;
        current.resources.disconnect_run(run, ConnectionId::new(instance)?)?;
        current.resources.settle_run(run)?;
        coord = current.coord;
        resources = current.resources;
    }
    assert_eq!(coord.records.iter().flatten().count(), 128);
    assert_eq!(
        coord.seed(&mut resources, seed(run(1)?)?, context(1, 100)?),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::StaleIdentity))
    );
    Ok(())
}
#[test]
fn supervisor_family_settlement_matches_full_historical_key_and_is_independent() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner(3)?)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build01")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::SupervisorInitialization,
    )?;
    let summary = IncidentSummary::new(
        seed.clone(),
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(
                IncidentUtc::from_unix_seconds(101)?,
                Some(IncidentUtc::from_unix_seconds(102)?),
            )?,
            ExitAttribution::Supervisor { status: 7 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Unavailable,
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?;
    coord.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary,
    })?;
    let source =
        coord.snapshot(seed.key()).ok_or("No snapshot")?.source_scope.ok_or("No source")?;
    coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope })?;
    assert!(!coord.record(coord.index(seed.key())?)?.reclaimable());
    assert_eq!(
        coord.observe(IncidentServiceObservation::SupervisorFamilySettled { owner: owner(4)? }),
        Err(IncidentDecisionError::StaleOperation)
    );
    let mut foreign = owner(3)?;
    foreign.namespace = NamespaceId::new([8; 32]);
    assert_eq!(
        coord.observe(IncidentServiceObservation::SupervisorFamilySettled { owner: foreign }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert!(
        coord
            .observe(IncidentServiceObservation::SupervisorFamilySettled { owner: owner(3)? })?
            .is_empty()
    );
    assert!(coord.record(coord.index(seed.key())?)?.reclaimable());
    assert!(
        coord
            .observe(IncidentServiceObservation::SupervisorFamilySettled { owner: owner(3)? })?
            .is_empty()
    );
    Ok(())
}
