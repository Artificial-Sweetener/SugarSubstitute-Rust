// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect bounded cross-owner admission from replay and partial marker mutation.
use super::support::*;

#[test]
fn enrolled_seed_commits_before_connection_and_accepts_genuine_early_failed_fact() -> TestResult {
    let run = run(1)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    resources.enroll_run(RunEnrollment {
        run,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("prelaunch".into())?,
    })?;
    let original = seed(run)?;
    let effects = coord.seed(&mut resources, original.clone(), context(1, 100)?)?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::PublishSeed { scope, .. }) => *scope,
        _ => return Err("Missing actual seed publication".into()),
    };
    let fact = exit(run, None)?.failed_incident()?;
    assert_eq!(
        coord.failed_desktop(&mut resources, fact, FailureEvidence::Unexplained, context(2, 101)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert!(exit(run, Some(CompletionIntent::Normal))?.failed_incident().is_err());
    assert!(exit(run, Some(CompletionIntent::StartupCancelled))?.failed_incident().is_err());
    let mut foreign = run;
    foreign.owner = owner(3)?;
    assert_eq!(
        coord.failed_desktop(
            &mut resources,
            exit(foreign, None)?.failed_incident()?,
            FailureEvidence::Unexplained,
            context(2, 101)?
        ),
        Err(IncidentDecisionError::ForeignNamespace)
    );
    coord.observe(IncidentServiceObservation::SeedCommitted { scope, seed: original })?;
    let effects = coord.failed_desktop(
        &mut resources,
        fact,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    assert!(matches!(
        effects.iter().next(),
        Some(IncidentEffect::CollectEvidence {
            attribution: ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            ..
        })
    ));
    assert!(
        coord
            .failed_desktop(&mut resources, fact, FailureEvidence::Unexplained, context(2, 101)?)?
            .is_empty()
    );
    assert_eq!((resources.snapshot().resources, resources.snapshot().leases), (0, 0));
    Ok(())
}
#[test]
fn disconnected_or_settled_enrollment_cannot_admit_a_new_seed() -> TestResult {
    let run = run(1)?;
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run)?;
    resources.disconnect_run(run, ConnectionId::new(1)?)?;
    for settled in [false, true] {
        if settled {
            resources.settle_run(run)?;
        }
        let mut coord = IncidentCoordinator::new(owner(2)?);
        assert_eq!(
            coord.seed(&mut resources, seed(run)?, context(1, 100)?),
            Err(IncidentDecisionError::Lifecycle(LifecycleRejection::WrongPhase))
        );
        assert!(coord.snapshot(seed(run)?.key()).is_none());
    }
    Ok(())
}

#[test]
fn exact_seed_repeats_without_second_effect_changed_content_rejects() -> TestResult {
    let run = run(1)?;
    let seed = seed(run)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run)?;
    let original = coord.seed(&mut resources, seed.clone(), context(1, 100)?)?;
    assert_eq!(original.len(), 1);
    assert!(coord.seed(&mut resources, seed.clone(), context(1, 100)?)?.is_empty());
    assert_eq!(
        coord.seed(&mut resources, seed, context(2, 100)?),
        Err(IncidentDecisionError::ChangedDuplicate)
    );
    Ok(())
}
#[test]
fn foreign_and_unregistered_seed_leave_owner_markers_unchanged() -> TestResult {
    let run = run(1)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    assert!(matches!(
        coord.seed(&mut resources, seed(run)?, context(1, 100)?),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::StaleIdentity))
    ));
    register(&mut resources, run)?;
    assert_eq!(coord.seed(&mut resources, seed(run)?, context(1, 100)?)?.len(), 1);
    let mut foreign = run;
    foreign.owner = owner(3)?;
    assert_eq!(
        coord.seed(&mut resources, seed(foreign)?, context(1, 100)?),
        Err(IncidentDecisionError::ForeignNamespace)
    );
    Ok(())
}
#[test]
fn operation_exhaustion_precedes_m1_marker_and_record_admission() -> TestResult {
    let run = run(1)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run)?;
    coord.next_operation = u64::MAX;
    assert_eq!(
        coord.seed(&mut resources, seed(run)?, context(1, 100)?),
        Err(IncidentDecisionError::OperationExhausted)
    );
    assert!(coord.snapshot(seed(run)?.key()).is_none());
    coord.next_operation = 0;
    assert_eq!(coord.seed(&mut resources, seed(run)?, context(1, 100)?)?.len(), 1);
    Ok(())
}
#[test]
fn failure_exhaustion_does_not_poison_authoritative_failure_marker() -> TestResult {
    let mut ready = ready(1)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    seed_ready(&mut coord, &mut resources, ready.seed.clone())?;
    let fact = exit(run(1)?, None)?.failed_incident()?;
    coord.next_operation = u64::MAX;
    assert_eq!(
        coord.failed_desktop(&mut resources, fact, FailureEvidence::Unexplained, context(2, 101)?),
        Err(IncidentDecisionError::OperationExhausted)
    );
    coord.next_operation = 1;
    assert_eq!(
        coord
            .failed_desktop(&mut resources, fact, FailureEvidence::Unexplained, context(2, 101)?)?
            .len(),
        1
    );
    assert!(
        ready
            .coord
            .failed_desktop(
                &mut ready.resources,
                fact,
                FailureEvidence::Unexplained,
                context(2, 101)?
            )?
            .is_empty()
    );
    Ok(())
}
#[test]
fn changed_actual_failure_rejects_without_reclassification() -> TestResult {
    let mut ready = ready(1)?;
    let fact = exit(run(1)?, None)?.failed_incident()?;
    assert_eq!(
        ready.coord.failed_desktop(
            &mut ready.resources,
            fact,
            FailureEvidence::NativeFault { code: 1, address: 2 },
            context(2, 101)?
        ),
        Err(IncidentDecisionError::ChangedDuplicate)
    );
    assert_eq!(
        ready.summary.facts().exit(),
        ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 }
    );
    assert!(exit(run(1)?, Some(CompletionIntent::Normal))?.failed_incident().is_err());
    assert!(exit(run(1)?, Some(CompletionIntent::StartupCancelled))?.failed_incident().is_err());
    Ok(())
}
#[test]
fn capacity_preflight_preserves_m1_marker_until_a_slot_can_reclaim() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let recovery = recovery(&mut coord)?;
    for instance in 1..=128 {
        coord.observe(IncidentServiceObservation::Recovered {
            scope: recovery,
            original_operation: historical_operation()?,
            summary: historical(instance, true)?,
        })?;
    }
    let run = run(200)?;
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run)?;
    assert_eq!(
        coord.seed(&mut resources, seed(run)?, context(1, 100)?),
        Err(IncidentDecisionError::Saturated)
    );
    let historical = historical(1, true)?;
    let source = coord
        .snapshot(historical.seed().key())
        .ok_or("No historical record")?
        .source_scope
        .ok_or("No source")?;
    coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    if let IncidentSubject::Desktop(old) = historical.seed().key().subject() {
        coord.observe(IncidentServiceObservation::FamilySettled { run: old })?;
    }
    assert_eq!(
        coord.seed(&mut resources, seed(run)?, context(1, 100)?),
        Err(IncidentDecisionError::Saturated)
    );
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope: recovery })?;
    assert_eq!(coord.seed(&mut resources, seed(run)?, context(1, 100)?)?.len(), 1);
    Ok(())
}
#[test]
fn sixty_second_budget_overflow_is_checked_and_utc_is_independent() -> TestResult {
    assert_eq!(
        IncidentOperationBudget::new(Moment::from_nanos(u64::MAX)),
        Err(sugarsubstitute_contracts::execution::TimeError::Overflow)
    );
    let context = context(17, 100)?;
    assert_eq!(context.budget().until().as_nanos(), 60_000_000_017);
    assert_eq!(context.at().unix_seconds(), 100);
    Ok(())
}
#[test]
fn current_supervisor_seed_is_once_only_and_never_uses_desktop_failure_path() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner(2)?)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build01")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::SupervisorInitialization,
    )?;
    let effects = coord.seed(&mut resources, seed.clone(), context(1, 100)?)?;
    assert_eq!(effects.len(), 1);
    assert!(coord.seed(&mut resources, seed, context(1, 100)?)?.is_empty());
    assert_eq!(resources.snapshot().runs, 0);
    assert_eq!(
        coord.failed_desktop(
            &mut resources,
            exit(run(1)?, None)?.failed_incident()?,
            FailureEvidence::Unexplained,
            context(2, 101)?
        ),
        Err(IncidentDecisionError::StaleOperation)
    );
    Ok(())
}
#[test]
fn different_incident_owner_cannot_replay_m1_seed_marker() -> TestResult {
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    let seed = seed(run(1)?)?;
    let mut first = IncidentCoordinator::new(owner(2)?);
    first.seed(&mut resources, seed.clone(), context(1, 100)?)?;
    let mut second = IncidentCoordinator::new(owner(2)?);
    assert_eq!(
        second.seed(&mut resources, seed.clone(), context(1, 100)?),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::StaleIdentity))
    );
    assert!(second.snapshot(seed.key()).is_none());
    Ok(())
}
#[test]
fn higher_generation_resets_markers_only_after_actual_m1_settlement() -> TestResult {
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let first = run(1)?;
    register(&mut resources, first)?;
    let mut coord = IncidentCoordinator::new(owner(2)?);
    seed_ready(&mut coord, &mut resources, seed(first)?)?;
    let mut next = first;
    next.generation = first.generation.next()?;
    assert!(register(&mut resources, next).is_err());
    resources.disconnect_run(first, ConnectionId::new(1)?)?;
    assert!(register(&mut resources, next).is_err());
    resources.settle_run(first)?;
    register(&mut resources, next)?;
    assert_eq!(coord.seed(&mut resources, seed(next)?, context(1, 100)?)?.len(), 1);
    assert_eq!(
        coord.failed_desktop(
            &mut resources,
            exit(first, None)?.failed_incident()?,
            FailureEvidence::Unexplained,
            context(2, 101)?
        ),
        Err(IncidentDecisionError::Lifecycle(LifecycleRejection::StaleIdentity))
    );
    Ok(())
}
