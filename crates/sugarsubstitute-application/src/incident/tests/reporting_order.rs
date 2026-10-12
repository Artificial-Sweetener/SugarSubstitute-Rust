// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual reporter failure ordering against each separately durable mutation.
use super::support::*;

fn failed(
    binding: ReportBinding,
) -> Result<IncidentServiceObservation, Box<dyn std::error::Error>> {
    Ok(IncidentServiceObservation::ReporterFailed {
        binding,
        failure: ReportFailure::UnexpectedExit,
        context: context(17, 103)?,
    })
}
fn scope(effects: &IncidentEffects) -> Result<IncidentOperationScope, Box<dyn std::error::Error>> {
    effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::RecordAttempt { scope, .. } => Some(*scope),
            _ => None,
        })
        .ok_or_else(|| "No queued attempt effect".into())
}
fn failed_summary(summary: &IncidentSummary) -> Result<IncidentSummary, IncidentError> {
    changed(
        summary,
        ReportDisposition::new(1, Some(ReportAttempt::Failed(ReportFailure::UnexpectedExit)))?,
        summary.facts().retention().acknowledged(),
    )
}

#[test]
fn failure_during_export_waits_actual_success_or_failure_and_uses_prepared_operation() -> TestResult
{
    for succeeds in [true, false] {
        let mut ready = ready(1)?;
        present(&mut ready)?;
        let IncidentReportDecision::Effects(effects) = ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::Export(ExportSelection::TextOnly)),
            context(4, 103)?,
        )?
        else {
            return Err("No export".into());
        };
        let export = match effects.iter().next() {
            Some(IncidentEffect::Export { scope, .. }) => *scope,
            _ => return Err("No export scope".into()),
        };
        let previous = ready.coord.next_operation;
        assert!(ready.coord.observe(failed(ready.binding)?)?.is_empty());
        let reserved = ready.coord.next_operation;
        assert_eq!(reserved, previous + 1);
        assert!(ready.coord.observe(failed(ready.binding)?)?.is_empty());
        assert_eq!(ready.coord.next_operation, reserved);
        assert_eq!(
            ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
            Some(export)
        );
        let effects = if succeeds {
            ready.coord.observe(IncidentServiceObservation::ExportFinished {
                scope: export,
                binding: ready.binding,
            })?
        } else {
            ready.coord.observe(IncidentServiceObservation::Failed {
                scope: export,
                failure: IncidentServiceFailure::Export,
            })?
        };
        let scope = scope(&effects)?;
        assert_eq!(scope.operation().id().get(), reserved);
        assert_eq!(scope.original(), context(17, 103)?.budget());
        assert!(
            ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none()
        );
        ready
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?;
        let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
            scope,
            binding: ready.binding,
            summary: failed_summary(&ready.summary)?,
        })?;
        assert!(effects.is_empty());
        if !succeeds {
            assert_eq!(
                ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.first_failure,
                Some(IncidentServiceFailure::Export)
            );
        }
    }
    Ok(())
}

#[test]
fn failure_during_ack_preserves_actual_durable_close_restart_and_original_failure_budget()
-> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    ready.coord.observe(IncidentServiceObservation::SourcesRetired {
        scope: IncidentSourceScope::Live(ready.source),
    })?;
    ready.coord.observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?;
    let ctx = context(4, 103)?;
    let IncidentReportDecision::Effects(effects) = ready.coord.report_action(
        ReportCommand::new(ready.binding, ReportAction::DismissAndRequestRestart),
        ctx,
    )?
    else {
        return Err("No ack".into());
    };
    let ack_scope = match effects.iter().next() {
        Some(IncidentEffect::Acknowledge { scope, .. }) => *scope,
        _ => return Err("No ack scope".into()),
    };
    assert!(ready.coord.observe(failed(ready.binding)?)?.is_empty());
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
        Some(ack_scope)
    );
    let summary = changed(&ready.summary, ready.summary.facts().reporting(), Some(ctx.at()))?;
    let acknowledgement =
        DurableAcknowledgement::new(ready.seed.key(), summary.revision(), ctx.at());
    let effects = ready.coord.observe(IncidentServiceObservation::Acknowledged {
        scope: ack_scope,
        binding: ready.binding,
        acknowledgement,
        summary: summary.clone(),
    })?;
    assert_eq!(effects.len(), 3);
    assert!(matches!(effects.iter().next(), Some(IncidentEffect::CloseAfterDurableAck { .. })));
    assert_eq!(
        effects.iter().filter(|e| matches!(e, IncidentEffect::ForwardRestartIntent { .. })).count(),
        1
    );
    let scope = scope(&effects)?;
    assert_eq!(scope.expected_revision(), summary.revision());
    assert_eq!(scope.original(), context(17, 103)?.budget());
    ready.coord.observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?;
    assert!(!ready.coord.record(ready.coord.index(ready.seed.key())?)?.reclaimable());
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: ready.binding,
        summary: failed_summary(&summary)?,
    })?;
    assert!(effects.is_empty());
    let snapshot = ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?;
    assert_eq!(snapshot.acknowledgement, Some(acknowledgement));
    assert!(snapshot.reporter_retired);
    assert!(ready.coord.record(ready.coord.index(ready.seed.key())?)?.reclaimable());
    Ok(())
}

#[test]
fn actual_retirement_discharges_unreachable_refresh_and_emits_original_queued_cas() -> TestResult {
    let mut ready = ready(1)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: ready.binding,
        context: context(3, 102)?,
    })?;
    let first = scope(&effects)?;
    ready.summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope: first,
        binding: ready.binding,
        summary: ready.summary.clone(),
    })?;
    let current = match effects.iter().next() {
        Some(IncidentEffect::ReplaceReportView { current, .. }) => *current,
        _ => return Err("No refresh".into()),
    };
    assert!(ready.coord.observe(failed(current)?)?.is_empty());
    let reserved = ready.coord.next_operation;
    let effects =
        ready.coord.observe(IncidentServiceObservation::ReporterRetired { binding: current })?;
    let scope = scope(&effects)?;
    assert_eq!(scope.operation().id().get(), reserved);
    assert_eq!(scope.original(), context(17, 103)?.budget());
    assert!(!ready.coord.record(ready.coord.index(ready.seed.key())?)?.refresh_pending);
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: current,
        summary: failed_summary(&ready.summary)?,
    })?;
    assert!(effects.is_empty());
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    Ok(())
}

#[test]
fn retirement_before_presented_commit_does_not_start_concurrent_failure_cas() -> TestResult {
    let mut ready = ready(1)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: ready.binding,
        context: context(3, 102)?,
    })?;
    let first = scope(&effects)?;
    ready.coord.observe(failed(ready.binding)?)?;
    assert!(
        ready
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?
            .is_empty()
    );
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
        Some(first)
    );
    let summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope: first,
        binding: ready.binding,
        summary: summary.clone(),
    })?;
    let queued = scope(&effects)?;
    assert_eq!(effects.len(), 1);
    let binding = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { binding, .. }) => *binding,
        _ => return Err("No binding".into()),
    };
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope: queued,
        binding,
        summary: failed_summary(&summary)?,
    })?;
    assert!(effects.is_empty());
    Ok(())
}

#[test]
fn queued_failure_exhaustion_preflights_without_poisoning_pending_ack_or_report_state() -> TestResult
{
    let mut ready = ready(1)?;
    present(&mut ready)?;
    ready.coord.report_action(
        ReportCommand::new(ready.binding, ReportAction::Dismiss),
        context(4, 103)?,
    )?;
    let before = ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?;
    let highwater = ready.coord.next_operation;
    ready.coord.next_operation = u64::MAX;
    assert_eq!(
        ready.coord.observe(failed(ready.binding)?),
        Err(IncidentDecisionError::OperationExhausted)
    );
    assert_eq!(ready.coord.snapshot(ready.seed.key()), Some(before));
    assert!(ready.coord.record(ready.coord.index(ready.seed.key())?)?.queued_failure.is_none());
    ready.coord.next_operation = highwater;
    ready.coord.observe(failed(ready.binding)?)?;
    let reserved = ready.coord.next_operation;
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::ReporterFailed {
            binding: ready.binding,
            failure: ReportFailure::Load,
            context: context(17, 103)?
        }),
        Err(IncidentDecisionError::ChangedDuplicate)
    );
    assert_eq!(ready.coord.next_operation, reserved);
    Ok(())
}

#[test]
fn failure_after_ack_or_actual_retirement_records_once_without_reopening_view() -> TestResult {
    for acknowledged in [true, false] {
        let mut ready = ready(1)?;
        present(&mut ready)?;
        if acknowledged {
            acknowledge(&mut ready, false)?;
        }
        ready
            .coord
            .observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?;
        let effects = ready.coord.observe(failed(ready.binding)?)?;
        let scope = scope(&effects)?;
        let summary = failed_summary(&ready.summary)?;
        let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
            scope,
            binding: ready.binding,
            summary: summary.clone(),
        })?;
        assert!(effects.is_empty());
        let snapshot = ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?;
        assert_eq!(snapshot.acknowledgement.is_some(), acknowledged);
        assert!(snapshot.reporter_retired);
        let current = snapshot.report_binding.ok_or("No current binding")?;
        assert!(
            ready
                .coord
                .report_action(
                    ReportCommand::new(current, ReportAction::CopyText),
                    context(20, 104)?
                )
                .is_err()
        );
        assert_eq!(summary.facts().reporting().attempts(), 1);
    }
    Ok(())
}
