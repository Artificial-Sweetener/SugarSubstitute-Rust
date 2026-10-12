// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect exact report CAS, actual view consumption and explicit acknowledgement.
use super::support::*;

#[test]
fn presented_cas_fences_old_commands_until_actual_same_session_refresh() -> TestResult {
    let mut ready = ready(1)?;
    let old = ready.binding;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: old,
        context: context(3, 102)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("No attempt".into()),
    };
    assert_eq!(
        ready
            .coord
            .report_action(ReportCommand::new(old, ReportAction::CopyText), context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    let summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let observation = IncidentServiceObservation::AttemptCommitted { scope, binding: old, summary };
    let effects = ready.coord.observe(observation.clone())?;
    let current = match effects.iter().next() {
        Some(IncidentEffect::ReplaceReportView { current, .. }) => *current,
        _ => return Err("No refresh".into()),
    };
    assert_eq!(current.session(), old.session());
    assert_ne!(current.revision(), old.revision());
    assert!(ready.coord.observe(observation)?.is_empty());
    assert_eq!(
        ready.coord.report_action(ReportCommand::new(old, ReportAction::Dismiss), context(4, 103)?),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert_eq!(
        ready
            .coord
            .report_action(ReportCommand::new(current, ReportAction::Dismiss), context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::ReportViewReplaced { binding: old }),
        Err(IncidentDecisionError::StaleOperation)
    );
    ready.coord.observe(IncidentServiceObservation::ReportViewReplaced { binding: current })?;
    assert_eq!(
        ready
            .coord
            .report_action(ReportCommand::new(current, ReportAction::CopyText), context(4, 103)?)?,
        IncidentReportDecision::CopyText { binding: current }
    );
    assert!(
        ready
            .coord
            .observe(IncidentServiceObservation::ReporterPresented {
                binding: old,
                context: context(3, 102)?
            })?
            .is_empty()
    );
    Ok(())
}
#[test]
fn actual_attempt_commit_rejects_modified_original_facts() -> TestResult {
    let mut ready = ready(1)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: ready.binding,
        context: context(3, 102)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("No attempt".into()),
    };
    let wrong =
        changed(&ready.summary, ReportDisposition::new(2, Some(ReportAttempt::Presented))?, None)?;
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
            scope,
            binding: ready.binding,
            summary: wrong
        }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
        Some(scope)
    );
    Ok(())
}
#[test]
fn copy_is_local_only_and_export_never_acknowledges() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let operation = ready.coord.next_operation;
    assert!(matches!(
        ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::CopyText),
            context(99, 103)?
        )?,
        IncidentReportDecision::CopyText { .. }
    ));
    assert_eq!(ready.coord.next_operation, operation);
    let IncidentReportDecision::Effects(effects) = ready.coord.report_action(
        ReportCommand::new(ready.binding, ReportAction::Export(ExportSelection::TextAndNativeDump)),
        context(400, 103)?,
    )?
    else {
        return Err("No export".into());
    };
    let scope = match effects.iter().next() {
        Some(IncidentEffect::Export { scope, selection, .. }) => {
            assert_eq!(*selection, ExportSelection::TextAndNativeDump);
            *scope
        }
        _ => return Err("No export".into()),
    };
    assert_eq!(scope.original(), context(400, 103)?.budget());
    ready
        .coord
        .observe(IncidentServiceObservation::ExportFinished { scope, binding: ready.binding })?;
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.revision,
        ready.summary.revision()
    );
    Ok(())
}
#[test]
fn export_failure_is_not_ack_and_latches_original_cause() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let IncidentReportDecision::Effects(effects) = ready.coord.report_action(
        ReportCommand::new(ready.binding, ReportAction::Export(ExportSelection::TextOnly)),
        context(4, 103)?,
    )?
    else {
        return Err("No export".into());
    };
    let scope = match effects.iter().next() {
        Some(IncidentEffect::Export { scope, .. }) => *scope,
        _ => return Err("No export".into()),
    };
    ready.coord.observe(IncidentServiceObservation::Failed {
        scope,
        failure: IncidentServiceFailure::Export,
    })?;
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    assert!(matches!(
        ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::CopyText),
            context(4, 103)?
        )?,
        IncidentReportDecision::CopyText { .. }
    ));
    Ok(())
}
#[test]
fn actual_ack_exact_utc_and_content_are_required_before_close() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let ctx = context(4, 103)?;
    let IncidentReportDecision::Effects(effects) =
        ready.coord.report_action(ReportCommand::new(ready.binding, ReportAction::Dismiss), ctx)?
    else {
        return Err("No ack".into());
    };
    assert_eq!(effects.len(), 1);
    let scope = match effects.iter().next() {
        Some(IncidentEffect::Acknowledge { scope, .. }) => *scope,
        _ => return Err("No ack".into()),
    };
    let summary = changed(&ready.summary, ready.summary.facts().reporting(), Some(ctx.at()))?;
    let wrong = DurableAcknowledgement::new(
        ready.seed.key(),
        summary.revision(),
        IncidentUtc::from_unix_seconds(104)?,
    );
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::Acknowledged {
            scope,
            binding: ready.binding,
            acknowledgement: wrong,
            summary: summary.clone()
        }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    let ack = DurableAcknowledgement::new(ready.seed.key(), summary.revision(), ctx.at());
    let observation = IncidentServiceObservation::Acknowledged {
        scope,
        binding: ready.binding,
        acknowledgement: ack,
        summary,
    };
    let effects = ready.coord.observe(observation.clone())?;
    assert!(matches!(effects.iter().next(), Some(IncidentEffect::CloseAfterDurableAck { .. })));
    assert!(ready.coord.observe(observation)?.is_empty());
    Ok(())
}
#[test]
fn reporter_exit_zero_is_not_ack_and_does_not_launch_recursive_report() -> TestResult {
    let mut ready = ready(1)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterFailed {
        binding: ready.binding,
        failure: ReportFailure::UnexpectedExit,
        context: context(3, 102)?,
    })?;
    assert!(matches!(
        effects.iter().next(),
        Some(IncidentEffect::RecordAttempt {
            attempt: ReportAttempt::Failed(ReportFailure::UnexpectedExit),
            ..
        })
    ));
    assert!(!effects.iter().any(|e| matches!(
        e,
        IncidentEffect::LaunchReporter { .. } | IncidentEffect::PublishSeed { .. }
    )));
    ready.coord.observe(IncidentServiceObservation::ReporterRetired { binding: ready.binding })?;
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    Ok(())
}
#[test]
fn failure_after_presented_counts_same_session_once() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterFailed {
        binding: ready.binding,
        failure: ReportFailure::Presentation,
        context: context(4, 103)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("No attempt".into()),
    };
    let summary = changed(
        &ready.summary,
        ReportDisposition::new(1, Some(ReportAttempt::Failed(ReportFailure::Presentation)))?,
        None,
    )?;
    ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: ready.binding,
        summary: summary.clone(),
    })?;
    assert_eq!(summary.facts().reporting().attempts(), 1);
    assert!(ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.acknowledgement.is_none());
    Ok(())
}
#[test]
fn failure_during_presented_cas_waits_actual_commit_refresh_and_preserves_budget() -> TestResult {
    let mut ready = ready(1)?;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: ready.binding,
        context: context(3, 102)?,
    })?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("No attempt".into()),
    };
    let failure = IncidentServiceObservation::ReporterFailed {
        binding: ready.binding,
        failure: ReportFailure::UnexpectedExit,
        context: context(17, 103)?,
    };
    assert!(ready.coord.observe(failure.clone())?.is_empty());
    assert!(ready.coord.observe(failure)?.is_empty());
    let summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding: ready.binding,
        summary,
    })?;
    let binding = match effects.iter().next() {
        Some(IncidentEffect::ReplaceReportView { current, .. }) => *current,
        _ => return Err("No refresh".into()),
    };
    let effects =
        ready.coord.observe(IncidentServiceObservation::ReportViewReplaced { binding })?;
    match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt {
            scope: next,
            attempt: ReportAttempt::Failed(ReportFailure::UnexpectedExit),
            ..
        }) => {
            assert_eq!(next.original(), context(17, 103)?.budget());
            assert_ne!(next.operation(), scope.operation());
        }
        _ => return Err("No failed attempt".into()),
    };
    Ok(())
}
