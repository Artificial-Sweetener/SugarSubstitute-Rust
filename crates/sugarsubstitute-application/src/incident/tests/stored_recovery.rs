// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize canonical stored-summary recovery without manufacturing a historical operation.
use super::support::*;
#[test]
fn stored_pending_recovery_keeps_original_subject_and_forwards_only_fresh_intent() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let recovery = recovery(&mut coordinator)?;
    let summary = historical(1, false)?;
    let effects = coordinator.observe(IncidentServiceObservation::Recovered {
        scope: recovery,
        original_operation: None,
        summary: summary.clone(),
    })?;
    let mut binding = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::LaunchReporter { binding, .. } => Some(*binding),
            _ => None,
        })
        .ok_or("Pending incident did not reopen")?;
    let snapshot = coordinator.snapshot(summary.seed().key()).ok_or("Recovered incident absent")?;
    assert_eq!(snapshot.original_publication, None);
    assert_eq!(binding.incident(), summary.seed().key());
    assert_eq!(binding.revision(), summary.revision());
    assert!(!snapshot.family_settled);
    let effects = coordinator.observe(IncidentServiceObservation::ReporterPresented {
        binding,
        context: context(30, 120)?,
    })?;
    let scope = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::RecordAttempt { scope, .. } => Some(*scope),
            _ => None,
        })
        .ok_or("Actual presentation attempt absent")?;
    let presented =
        changed(&summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = coordinator.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding,
        summary: presented,
    })?;
    binding = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::ReplaceReportView { current, .. } => Some(*current),
            _ => None,
        })
        .ok_or("Actual same-session replacement absent")?;
    coordinator.observe(IncidentServiceObservation::ReportViewReplaced { binding })?;
    assert!(matches!(
        coordinator.report_action(
            ReportCommand::new(binding, ReportAction::CopyText),
            context(40, 130)?
        )?,
        IncidentReportDecision::CopyText { .. }
    ));
    let effects = match coordinator.report_action(
        ReportCommand::new(binding, ReportAction::DismissAndRequestRestart),
        context(40, 130)?,
    )? {
        IncidentReportDecision::Effects(effects) => effects,
        _ => return Err("No acknowledgement request".into()),
    };
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        IncidentEffect::ForwardRestartIntent { .. }
            | IncidentEffect::ForwardRecoveredRestartIntent { .. }
    )));
    assert_eq!(
        coordinator.snapshot(binding.incident()).ok_or("Incident lost")?.acknowledgement,
        None
    );
    let scope = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::Acknowledge { scope, .. } => Some(*scope),
            _ => None,
        })
        .ok_or("Actual acknowledgement scope missing")?;
    let summary = changed(
        &changed(&summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?,
        ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
        Some(context(40, 130)?.at()),
    )?;
    let ack =
        DurableAcknowledgement::new(binding.incident(), summary.revision(), context(40, 130)?.at());
    let effects = coordinator.observe(IncidentServiceObservation::Acknowledged {
        scope,
        binding,
        acknowledgement: ack,
        summary,
    })?;
    assert!(
        !effects.iter().any(|effect| matches!(effect, IncidentEffect::ForwardRestartIntent { .. }))
    );
    // Current reader closure is independent; no historical family settlement is invented.
    let source = coordinator
        .snapshot(binding.incident())
        .ok_or("Incident lost")?
        .source_scope
        .ok_or("Source absent")?;
    let effects =
        coordinator.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    assert!(effects.iter().any(|effect| matches!(effect,
        IncidentEffect::ForwardRecoveredRestartIntent { incident, acknowledgement }
            if *incident == binding.incident() && *acknowledgement == ack)));
    assert!(!coordinator.snapshot(binding.incident()).ok_or("Incident lost")?.family_settled);
    Ok(())
}
#[test]
fn same_pending_summary_recovery_is_idempotent_and_changed_provenance_rejects() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coordinator)?;
    let summary = historical(1, false)?;
    let observation = IncidentServiceObservation::Recovered {
        scope,
        original_operation: None,
        summary: summary.clone(),
    };
    coordinator.observe(observation.clone())?;
    assert!(coordinator.observe(observation)?.is_empty());
    assert!(
        coordinator
            .observe(IncidentServiceObservation::Recovered {
                scope,
                original_operation: Some(historical_operation()?),
                summary
            })
            .is_err()
    );
    Ok(())
}

#[test]
fn open_stored_recovery_reuses_only_actually_acknowledged_and_retired_slots() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let scope = recovery(&mut coordinator)?;
    for instance in 1..=MAX_INCIDENT_RECORDS as u64 {
        coordinator.observe(IncidentServiceObservation::Recovered {
            scope,
            original_operation: None,
            summary: historical(instance, true)?,
        })?;
    }
    let next = historical(MAX_INCIDENT_RECORDS as u64 + 1, true)?;
    assert_eq!(
        coordinator
            .observe(IncidentServiceObservation::Recovered {
                scope,
                original_operation: None,
                summary: next.clone(),
            })
            .err(),
        Some(IncidentDecisionError::Saturated)
    );
    let first = historical(1, true)?.seed().key();
    let before = coordinator.snapshot(first).ok_or("First incident missing")?;
    assert!(!before.sources_retired && !before.family_settled);
    let source = before.source_scope.ok_or("First source missing")?;
    coordinator.observe(IncidentServiceObservation::SourcesRetired { scope: source })?;
    let settled = coordinator.snapshot(first).ok_or("First incident lost")?;
    assert!(settled.sources_retired && settled.reporter_retired);
    assert!(!settled.family_settled);
    coordinator.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: None,
        summary: next.clone(),
    })?;
    assert!(coordinator.snapshot(first).is_none());
    assert!(coordinator.snapshot(next.seed().key()).is_some());
    assert_eq!(coordinator.records.len(), MAX_INCIDENT_RECORDS);
    assert_eq!(coordinator.records.iter().flatten().count(), MAX_INCIDENT_RECORDS);
    assert!(!coordinator.recovery_snapshot().ok_or("Recovery missing")?.finished);
    Ok(())
}
