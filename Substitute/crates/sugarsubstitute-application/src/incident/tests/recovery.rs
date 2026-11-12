// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve historical identity without rebasing its authority or monotonic epoch.
use super::support::*;

#[test]
fn recovered_pending_uses_current_recovery_and_inert_original_source() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let summary = historical(1, false)?;
    let original = historical_operation()?;
    let observation = IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(original),
        summary: summary.clone(),
    };
    let effects = coord.observe(observation.clone())?;
    assert_eq!(effects.len(), 2);
    let source = match effects.iter().next() {
        Some(IncidentEffect::RetireSource { scope: IncidentSourceScope::Recovered(source) }) => {
            *source
        }
        _ => return Err("No recovered source".into()),
    };
    assert_eq!(source.recovery(), scope);
    assert_eq!(source.historical_operation(), Some(original));
    assert_eq!(source.incident(), summary.seed().key());
    assert_ne!(scope.operation().owner(), original.owner());
    assert_eq!(source.recovery().original(), context(20, 120)?.budget());
    assert!(coord.observe(observation)?.is_empty());
    Ok(())
}
#[test]
fn acknowledged_recovery_does_not_launch_and_requires_independent_settlement() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let summary = historical(1, true)?;
    let effects = coord.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary: summary.clone(),
    })?;
    assert_eq!(effects.len(), 1);
    assert!(!effects.iter().any(|e| matches!(e, IncidentEffect::LaunchReporter { .. })));
    let snapshot = coord.snapshot(summary.seed().key()).ok_or("No snapshot")?;
    assert!(snapshot.acknowledgement.is_some());
    assert!(!snapshot.sources_retired);
    assert!(!snapshot.family_settled);
    assert!(snapshot.reporter_retired);
    Ok(())
}

#[test]
fn recovery_does_not_reclassify_clean_desktop_exit_as_failed_incident() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let original = historical(1, false)?;
    let facts = original.facts();
    for kind in [DesktopExitKind::Normal, DesktopExitKind::StartupCancelled] {
        let summary = IncidentSummary::new(
            original.seed().clone(),
            original.revision(),
            SummaryFacts::new(
                facts.retention(),
                ExitAttribution::Desktop { kind, status: 0 },
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?;
        assert_eq!(
            coord.observe(IncidentServiceObservation::Recovered {
                scope,
                original_operation: Some(historical_operation()?),
                summary
            }),
            Err(IncidentDecisionError::WrongPhase)
        );
    }
    assert!(coord.snapshot(original.seed().key()).is_none());
    Ok(())
}
#[test]
fn closed_recovery_rejects_new_projection_but_accepts_late_source_retirement() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let summary = historical(1, true)?;
    let observation = IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary: summary.clone(),
    };
    coord.observe(observation.clone())?;
    let source = coord
        .snapshot(summary.seed().key())
        .ok_or("No snapshot")?
        .source_scope
        .ok_or("No source")?;
    coord.observe(IncidentServiceObservation::RecoveryFinished { scope })?;
    assert_eq!(coord.observe(observation), Err(IncidentDecisionError::StaleOperation));
    assert!(
        coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?.is_empty()
    );
    assert!(
        coord.observe(IncidentServiceObservation::SourcesRetired { scope: source })?.is_empty()
    );
    assert!(coord.snapshot(summary.seed().key()).ok_or("No snapshot")?.sources_retired);
    Ok(())
}
#[test]
fn changed_history_or_current_recovery_rejects_before_record_admission() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coord)?;
    let summary = historical(1, false)?;
    let mut changed = scope;
    changed.original = context(21, 120)?.budget();
    assert_eq!(
        coord.observe(IncidentServiceObservation::Recovered {
            scope: changed,
            original_operation: Some(historical_operation()?),
            summary: summary.clone()
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    let foreign = LogicalStoreOperationKey::new(owner(4)?, IncidentOperationId::new(7)?);
    assert_eq!(
        coord.observe(IncidentServiceObservation::Recovered {
            scope,
            original_operation: Some(foreign),
            summary: summary.clone()
        }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    assert!(coord.snapshot(summary.seed().key()).is_none());
    coord.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(historical_operation()?),
        summary: summary.clone(),
    })?;
    assert_eq!(
        coord.observe(IncidentServiceObservation::Recovered {
            scope,
            original_operation: Some(historical_operation()?),
            summary: changed_summary(&summary)?
        }),
        Err(IncidentDecisionError::ChangedDuplicate)
    );
    Ok(())
}
fn changed_summary(summary: &IncidentSummary) -> Result<IncidentSummary, IncidentError> {
    changed(summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)
}
#[test]
fn recovered_attempt_uses_new_current_operation_and_keeps_original_publication() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let recovery = recovery(&mut coord)?;
    let summary = historical(1, false)?;
    let original = historical_operation()?;
    let projection = IncidentServiceObservation::Recovered {
        scope: recovery,
        original_operation: Some(original),
        summary: summary.clone(),
    };
    let effects = coord.observe(projection.clone())?;
    let binding = effects
        .iter()
        .find_map(|e| {
            if let IncidentEffect::LaunchReporter { binding, .. } = e {
                Some(*binding)
            } else {
                None
            }
        })
        .ok_or("No launch")?;
    let before = coord.snapshot(summary.seed().key()).ok_or("No snapshot")?;
    let effects = coord.observe(IncidentServiceObservation::ReporterPresented {
        binding,
        context: context(35, 125)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("No attempt".into()),
    };
    assert_eq!(scope.operation().owner(), owner(2)?);
    assert_ne!(scope.operation(), recovery.operation());
    assert_eq!(scope.original(), context(35, 125)?.budget());
    let updated = changed_summary(&summary)?;
    coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding,
        summary: updated,
    })?;
    assert!(coord.observe(projection)?.is_empty());
    let after = coord.snapshot(summary.seed().key()).ok_or("No snapshot")?;
    assert_eq!(after.original_publication, Some(original));
    assert_eq!(after.source_scope, before.source_scope);
    Ok(())
}
#[test]
fn abandoned_supervisor_collects_under_current_original_recovery_budget() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let recovery = recovery(&mut coord)?;
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner(3)?)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build01")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::SupervisorInitialization,
    )?;
    let observation = IncidentServiceObservation::RecoveredSupervisorSeed {
        scope: recovery,
        original_operation: historical_operation()?,
        seed: seed.clone(),
        actual_status: 77,
        failure: FailureEvidence::Unexplained,
    };
    let effects = coord.observe(observation.clone())?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, attribution, .. }) => {
            assert_eq!(*attribution, ExitAttribution::Supervisor { status: 77 });
            *scope
        }
        _ => return Err("No collection".into()),
    };
    assert_eq!(scope.operation().owner(), owner(2)?);
    assert_eq!(scope.original(), recovery.original());
    assert_ne!(scope.operation(), recovery.operation());
    assert!(coord.observe(observation)?.is_empty());
    let effects = coord
        .observe(IncidentServiceObservation::EvidenceCollected { scope, evidence: evidence()? })?;
    let summary = match effects.iter().next() {
        Some(IncidentEffect::PublishSummary { summary, .. }) => summary.clone(),
        _ => return Err("No summary".into()),
    };
    let effects = coord.observe(IncidentServiceObservation::SummaryCommitted { scope, summary })?;
    assert!(effects.iter().any(|e| matches!(
        e,
        IncidentEffect::RetireSource { scope: IncidentSourceScope::Recovered(_) }
    )));
    assert_eq!(
        coord.snapshot(seed.key()).ok_or("No snapshot")?.original_publication,
        Some(historical_operation()?)
    );
    Ok(())
}
