// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive original summary, action and retirement transitions through the real coordinator.
use super::support::*;

fn queue(
    actual: &mut Ready,
    instance: u64,
) -> Result<(IncidentSummary, IncidentEffects), Box<dyn std::error::Error>> {
    let run = run(instance)?;
    register(&mut actual.resources, run)?;
    let seed = seed(run)?;
    seed_ready(&mut actual.coord, &mut actual.resources, seed)?;
    let effects = actual.coord.failed_desktop(
        &mut actual.resources,
        exit(run, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(7, 104)?,
    )?;
    let source = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("Actual collection missing".into()),
    };
    let effects = actual.coord.observe(IncidentServiceObservation::EvidenceCollected {
        scope: source,
        evidence: evidence()?,
    })?;
    let summary = match effects.iter().next() {
        Some(IncidentEffect::PublishSummary { summary, .. }) => summary.clone(),
        _ => return Err("Actual summary missing".into()),
    };
    let effects = actual.coord.observe(IncidentServiceObservation::SummaryCommitted {
        scope: source,
        summary: summary.clone(),
    })?;
    Ok((summary, effects))
}

#[test]
fn queued_summary_keeps_current_copy_ack_and_only_launches_after_actual_retirement() -> TestResult {
    let mut actual = ready(1)?;
    present(&mut actual)?;
    let (summary, effects) = queue(&mut actual, 2)?;
    assert_eq!(effects.len(), 1);
    assert!(matches!(effects.iter().next(), Some(IncidentEffect::RetireSource { .. })));
    let queued = actual.coord.snapshot(summary.seed().key()).ok_or("Queued snapshot missing")?;
    assert!(queued.report_binding.is_none());
    assert!(!queued.automatic_report_attempted);
    assert!(actual.coord.diagnostic_custody_pending());
    assert!(matches!(actual.coord.report_action(
        ReportCommand::new(actual.binding, ReportAction::CopyText), context(8, 105)?)?,
        IncidentReportDecision::CopyText { binding } if binding == actual.binding));
    let effects = acknowledge(&mut actual, false)?;
    assert!(matches!(effects.iter().next(), Some(IncidentEffect::CloseAfterDurableAck { .. })));
    assert!(!effects.iter().any(|effect| matches!(effect, IncidentEffect::LaunchReporter { .. })));
    let effects = actual
        .coord
        .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?;
    let (binding, retained) = match effects.iter().next() {
        Some(IncidentEffect::LaunchReporter { binding, summary }) => (*binding, summary),
        _ => return Err("Next Reporter missing".into()),
    };
    assert_eq!(retained, &summary);
    assert_ne!(binding.session(), actual.binding.session());
    assert!(
        actual
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?
            .is_empty()
    );
    let stale = ReportBinding::new(
        actual.binding.incident(),
        actual.binding.revision().next()?,
        actual.binding.session(),
    );
    assert_eq!(
        actual.coord.observe(IncidentServiceObservation::ReporterRetired { binding: stale }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert_eq!(
        actual.coord.snapshot(summary.seed().key()).ok_or("Next snapshot missing")?.report_binding,
        Some(binding)
    );
    Ok(())
}

#[test]
fn queued_summaries_survive_failed_report_and_original_attempt_cas() -> TestResult {
    let mut actual = ready(1)?;
    let (second, _) = queue(&mut actual, 2)?;
    let (third, _) = queue(&mut actual, 3)?;
    let failure = ReportFailure::UnexpectedExit;
    let effects = actual.coord.observe(IncidentServiceObservation::ReporterFailed {
        binding: actual.binding,
        failure,
        context: context(9, 106)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("Original failed attempt missing".into()),
    };
    assert!(
        actual
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?
            .is_empty()
    );
    let failed = changed(
        &actual.summary,
        ReportDisposition::new(1, Some(ReportAttempt::Failed(failure)))?,
        None,
    )?;
    let effects = actual.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: actual.binding,
        summary: failed,
    })?;
    let binding = match effects.iter().next() {
        Some(IncidentEffect::LaunchReporter { binding, summary }) if summary == &second => *binding,
        _ => return Err("Original next summary lost".into()),
    };
    assert_eq!(
        actual.coord.record(actual.coord.index(third.seed().key())?)?.summary.as_ref(),
        Some(&third)
    );
    assert!(
        actual
            .coord
            .snapshot(third.seed().key())
            .ok_or("Third snapshot missing")?
            .report_binding
            .is_none()
    );
    let effects = actual.coord.observe(IncidentServiceObservation::ReporterRetired { binding })?;
    assert!(
        matches!(effects.iter().next(), Some(IncidentEffect::LaunchReporter { summary, .. }) if summary == &third)
    );
    Ok(())
}

#[test]
fn deferred_session_exhaustion_preserves_accepted_retirement_and_queued_summary() -> TestResult {
    let mut actual = ready(1)?;
    present(&mut actual)?;
    let (summary, _) = queue(&mut actual, 2)?;
    acknowledge(&mut actual, false)?;
    actual.coord.next_session = u64::MAX;
    let effects = actual
        .coord
        .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?;
    assert!(effects.is_empty());
    assert!(
        actual.coord.snapshot(actual.seed.key()).ok_or("Old snapshot missing")?.reporter_retired
    );
    let queued = actual.coord.record(actual.coord.index(summary.seed().key())?)?;
    assert_eq!(queued.summary.as_ref(), Some(&summary));
    assert!(queued.binding.is_none());
    assert!(!queued.automatic_report_attempted);
    assert_eq!(actual.coord.next_session, u64::MAX);
    assert_eq!(
        actual.coord.report_admission_failure(summary.seed().key()),
        Some(IncidentDecisionError::ReportSessionExhausted)
    );
    let effects =
        actual.coord.observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?;
    assert!(effects.is_empty());
    assert!(actual.coord.snapshot(actual.seed.key()).ok_or("Old snapshot missing")?.family_settled);
    assert_eq!(actual.coord.next_session, u64::MAX);
    assert_eq!(
        actual.coord.record(actual.coord.index(summary.seed().key())?)?.summary.as_ref(),
        Some(&summary)
    );
    Ok(())
}

#[test]
fn full_original_effect_envelope_preserves_every_effect_and_ready_summary_without_session()
-> TestResult {
    let mut actual = ready(1)?;
    actual
        .coord
        .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?;
    let new = run(2)?;
    register(&mut actual.resources, new)?;
    let seeded = seed(new)?;
    seed_ready(&mut actual.coord, &mut actual.resources, seeded)?;
    let collected = actual.coord.failed_desktop(
        &mut actual.resources,
        exit(new, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(10, 110)?,
    )?;
    let scope = match collected.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("Original collection missing".into()),
    };
    let published = actual
        .coord
        .observe(IncidentServiceObservation::EvidenceCollected { scope, evidence: evidence()? })?;
    let summary = match published.iter().next() {
        Some(IncidentEffect::PublishSummary { summary, .. }) => summary.clone(),
        _ => return Err("Original summary missing".into()),
    };
    // Exercise the validated original transition and its fixed envelope before queue dispatch.
    let mut effects = actual.coord.summary_committed(scope, summary.clone())?;
    for instance in 3..10 {
        let run = run(instance)?;
        register(&mut actual.resources, run)?;
        for effect in actual.coord.seed(&mut actual.resources, seed(run)?, context(11, 111)?)? {
            effects.append(effect)?;
        }
    }
    assert_eq!(effects.len(), MAX_INCIDENT_EFFECTS);
    let original = effects.clone();
    let high_water = actual.coord.next_session;
    actual.coord.append_ready_reporter(&mut effects);
    assert_eq!(effects, original);
    assert_eq!(actual.coord.next_session, high_water);
    let queued = actual.coord.record(actual.coord.index(summary.seed().key())?)?;
    assert_eq!(queued.summary.as_ref(), Some(&summary));
    assert!(queued.binding.is_none());
    assert!(!queued.automatic_report_attempted);
    assert_eq!(
        actual.coord.report_admission_failure(summary.seed().key()),
        Some(IncidentDecisionError::Saturated)
    );
    Ok(())
}
