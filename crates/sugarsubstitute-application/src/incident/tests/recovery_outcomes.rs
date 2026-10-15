// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe recovery failure and original source custody independently of enumeration.
use super::support::*;

#[test]
fn recovery_failure_retains_original_budget_and_late_actual_completion() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let mut wrong = scope;
    wrong.original = context(21, 120)?.budget();
    assert_eq!(
        coord.observe(IncidentServiceObservation::RecoveryFailed { scope: wrong }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert!(coord.recovery_snapshot().ok_or("No recovery")?.first_failure.is_none());
    assert!(coord.observe(IncidentServiceObservation::RecoveryFailed { scope })?.is_empty());
    assert!(coord.begin_recovery(scope.original())?.is_empty());
    assert_eq!(
        coord.begin_recovery(context(100, 120)?.budget()),
        Err(IncidentDecisionError::StaleOperation)
    );
    coord.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary: historical(1, true)?,
    })?;
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope })?;
    assert!(coord.observe(IncidentServiceObservation::RecoveryFailed { scope })?.is_empty());
    let snapshot = coord.recovery_snapshot().ok_or("No recovery")?;
    assert_eq!(snapshot.scope, scope);
    assert!(snapshot.finished);
    assert_eq!(snapshot.first_failure, Some(IncidentServiceFailure::Recovery));
    assert_eq!(
        coord.observe(IncidentServiceObservation::Recovered {
            scope,
            original_operation: Some(historical_operation()?),
            summary: historical(2, true)?
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    Ok(())
}
#[test]
fn first_recovery_failure_after_completion_rejects_without_reopening() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope })?;
    assert_eq!(
        coord.observe(IncidentServiceObservation::RecoveryFailed { scope }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert!(coord.recovery_snapshot().ok_or("No recovery")?.first_failure.is_none());
    assert_eq!(coord.begin_recovery(scope.original()), Err(IncidentDecisionError::StaleOperation));
    Ok(())
}

#[test]
fn recovered_source_failure_preserves_correlation_after_recovery_closure() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let recovery = recovery(&mut coord)?;
    let summary = historical(1, true)?;
    coord.observe(IncidentServiceObservation::Recovered {
        scope: recovery,
        original_operation: Some(historical_operation()?),
        summary: summary.clone(),
    })?;
    let source = coord
        .snapshot(summary.seed().key())
        .ok_or("No snapshot")?
        .source_scope
        .ok_or("No source")?;
    let IncidentSourceScope::Recovered(mut changed) = source else {
        return Err("Expected recovered scope".into());
    };
    changed.historical_operation =
        Some(LogicalStoreOperationKey::new(owner(3)?, IncidentOperationId::new(8)?));
    assert_eq!(
        coord.observe(IncidentServiceObservation::SourceRetirementFailed {
            scope: IncidentSourceScope::Recovered(changed)
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    changed = match source {
        IncidentSourceScope::Recovered(value) => value,
        _ => return Err("Expected recovered scope".into()),
    };
    changed.recovery.original = context(21, 120)?.budget();
    assert_eq!(
        coord.observe(IncidentServiceObservation::SourceRetirementFailed {
            scope: IncidentSourceScope::Recovered(changed)
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert!(coord.snapshot(summary.seed().key()).ok_or("No snapshot")?.first_failure.is_none());
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope: recovery })?;
    assert!(
        coord
            .observe(IncidentServiceObservation::SourceRetirementFailed { scope: source })?
            .is_empty()
    );
    assert!(!coord.snapshot(summary.seed().key()).ok_or("No snapshot")?.sources_retired);
    coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    assert!(
        coord
            .observe(IncidentServiceObservation::SourceRetirementFailed { scope: source })?
            .is_empty()
    );
    let snapshot = coord.snapshot(summary.seed().key()).ok_or("No snapshot")?;
    assert_eq!(snapshot.source_scope, Some(source));
    assert_eq!(snapshot.first_failure, Some(IncidentServiceFailure::SourceRetirement));
    assert!(!snapshot.family_settled);
    Ok(())
}

#[test]
fn first_source_failure_after_actual_retirement_rejects_without_new_failure() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let summary = historical(1, true)?;
    coord.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary: summary.clone(),
    })?;
    let source = coord
        .snapshot(summary.seed().key())
        .ok_or("No snapshot")?
        .source_scope
        .ok_or("No source")?;
    coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    assert_eq!(
        coord.observe(IncidentServiceObservation::SourceRetirementFailed { scope: source }),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert!(coord.snapshot(summary.seed().key()).ok_or("No snapshot")?.first_failure.is_none());
    Ok(())
}
