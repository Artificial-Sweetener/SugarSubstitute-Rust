// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve recovery correlation through the real presented-report owner without advancing its action.
use super::support::*;

#[test]
fn recovery_origin_uses_unique_original_context_without_ack_and_preserves_durable_restart_order()
-> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let before = ready.coord.snapshot(ready.seed.key()).ok_or("Original report missing")?;
    let highwater = ready.coord.next_operation;
    let first_context = context(3, 102)?;
    let second_context = context(4, 103)?;
    let first = ready.coord.admit_recovery_capture_origin(ready.binding, first_context)?;
    let second = ready.coord.admit_recovery_capture_origin(ready.binding, second_context)?;
    assert_eq!(first.operation().owner(), owner(2)?);
    assert_eq!(first.operation().id().get(), highwater + 1);
    assert_eq!(second.operation().id().get(), highwater + 2);
    assert_eq!(first.incident(), ready.binding.incident());
    assert_eq!(first.expected_revision(), ready.binding.revision());
    assert_eq!(first.original(), first_context.budget());
    assert_eq!(second.original(), second_context.budget());
    assert_eq!(ready.coord.snapshot(ready.seed.key()), Some(before));
    assert_eq!(
        ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::CopyText),
            context(4, 103)?
        )?,
        IncidentReportDecision::CopyText { binding: ready.binding }
    );
    assert_eq!(ready.coord.next_operation, highwater + 2);
    ready.coord.observe(IncidentServiceObservation::SourcesRetired {
        scope: IncidentSourceScope::Live(ready.source),
    })?;
    ready.coord.observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?;
    let effects = acknowledge(&mut ready, true)?;
    assert_eq!(ready.coord.next_operation, highwater + 3);
    let mut ordered = effects.iter();
    let acknowledgement = match ordered.next() {
        Some(IncidentEffect::CloseAfterDurableAck { binding, acknowledgement }) => {
            assert_eq!(*binding, ready.binding);
            *acknowledgement
        }
        _ => return Err("Actual durable close was not first".into()),
    };
    assert!(matches!(ordered.next(), Some(IncidentEffect::ForwardRestartIntent {
        run: forwarded, acknowledgement: actual,
    }) if *forwarded == run(1)? && *actual == acknowledgement));
    assert!(ordered.next().is_none());
    let after = ready.coord.snapshot(ready.seed.key()).ok_or("Acknowledged report missing")?;
    assert_eq!(after.acknowledgement, Some(acknowledgement));
    assert!(ready.coord.record(ready.coord.index(ready.seed.key())?)?.restart_forwarded);
    assert!(after.current_operation.is_none());
    assert_eq!(first.original(), first_context.budget());
    Ok(())
}
#[test]
fn recovery_origin_rejects_unpresented_foreign_and_stale_bindings_without_consuming_operation()
-> TestResult {
    let mut ready = ready(1)?;
    let before = ready.coord.snapshot(ready.seed.key());
    let highwater = ready.coord.next_operation;
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(ready.binding, context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    let old = ready.binding;
    present(&mut ready)?;
    let highwater = ready.coord.next_operation;
    let before = ready.coord.snapshot(ready.seed.key());
    let foreign_key = IncidentKey::new(IncidentSubject::Desktop(run(2)?));
    let foreign_session = ReportBinding::new(
        ready.binding.incident(),
        ready.binding.revision(),
        ReportSessionId::new(ready.binding.session().get() + 1)?,
    );
    let foreign_subject =
        ReportBinding::new(foreign_key, ready.binding.revision(), ready.binding.session());
    for rejected in [old, foreign_session, foreign_subject] {
        assert_eq!(
            ready.coord.admit_recovery_capture_origin(rejected, context(5, 104)?),
            Err(IncidentDecisionError::StaleOperation)
        );
        assert_eq!(ready.coord.next_operation, highwater);
        assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    }
    assert_eq!(
        ready
            .coord
            .admit_recovery_capture_origin(ready.binding, context(6, 105)?)?
            .operation()
            .id()
            .get(),
        highwater + 1
    );
    Ok(())
}
#[test]
fn recovery_origin_waits_for_actual_presented_commit_and_same_session_refresh() -> TestResult {
    let mut ready = ready(1)?;
    let old = ready.binding;
    let effects = ready.coord.observe(IncidentServiceObservation::ReporterPresented {
        binding: old,
        context: context(3, 102)?,
    })?;
    let attempt = match effects.iter().next() {
        Some(IncidentEffect::RecordAttempt { scope, .. }) => *scope,
        _ => return Err("Actual presentation mutation missing".into()),
    };
    let before = ready.coord.snapshot(ready.seed.key());
    let highwater = ready.coord.next_operation;
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(old, context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    let summary =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    let effects = ready.coord.observe(IncidentServiceObservation::AttemptCommitted {
        scope: attempt,
        binding: old,
        summary,
    })?;
    let current = match effects.iter().next() {
        Some(IncidentEffect::ReplaceReportView { current, .. }) => *current,
        _ => return Err("Actual view refresh missing".into()),
    };
    let before = ready.coord.snapshot(ready.seed.key());
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(current, context(5, 104)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    ready.coord.observe(IncidentServiceObservation::ReportViewReplaced { binding: current })?;
    let origin = ready.coord.admit_recovery_capture_origin(current, context(6, 105)?)?;
    assert_eq!(origin.expected_revision(), current.revision());
    assert_eq!(origin.operation().id().get(), highwater + 1);
    Ok(())
}
#[test]
fn recovery_origin_cannot_overtake_export_acknowledgement_or_retired_reporter() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    let IncidentReportDecision::Effects(effects) = ready.coord.report_action(
        ReportCommand::new(ready.binding, ReportAction::Export(ExportSelection::TextOnly)),
        context(4, 103)?,
    )?
    else {
        return Err("Actual export missing".into());
    };
    let export = match effects.iter().next() {
        Some(IncidentEffect::Export { scope, .. }) => *scope,
        _ => return Err("Actual export scope missing".into()),
    };
    let before = ready.coord.snapshot(ready.seed.key());
    let highwater = ready.coord.next_operation;
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(ready.binding, context(5, 104)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    ready.coord.observe(IncidentServiceObservation::ExportFinished {
        scope: export,
        binding: ready.binding,
    })?;
    ready.coord.admit_recovery_capture_origin(ready.binding, context(6, 105)?)?;
    let ctx = context(7, 106)?;
    let IncidentReportDecision::Effects(effects) =
        ready.coord.report_action(ReportCommand::new(ready.binding, ReportAction::Dismiss), ctx)?
    else {
        return Err("Actual acknowledgement mutation missing".into());
    };
    let acknowledgement_scope = match effects.iter().next() {
        Some(IncidentEffect::Acknowledge { scope, .. }) => *scope,
        _ => return Err("Actual acknowledgement scope missing".into()),
    };
    let before = ready.coord.snapshot(ready.seed.key());
    let highwater = ready.coord.next_operation;
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(ready.binding, context(8, 107)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    let summary = changed(&ready.summary, ready.summary.facts().reporting(), Some(ctx.at()))?;
    let acknowledgement =
        DurableAcknowledgement::new(ready.seed.key(), summary.revision(), ctx.at());
    let effects = ready.coord.observe(IncidentServiceObservation::Acknowledged {
        scope: acknowledgement_scope,
        binding: ready.binding,
        acknowledgement,
        summary,
    })?;
    assert!(matches!(effects.iter().next(), Some(IncidentEffect::CloseAfterDurableAck {
        acknowledgement: actual, ..
    }) if *actual == acknowledgement));
    let before = ready.coord.snapshot(ready.seed.key());
    let highwater = ready.coord.next_operation;
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(ready.binding, context(7, 106)?),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert_eq!(ready.coord.next_operation, highwater);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    let mut retired = super::support::ready(2)?;
    present(&mut retired)?;
    retired
        .coord
        .observe(IncidentServiceObservation::ReporterRetired { binding: retired.binding })?;
    let before = retired.coord.snapshot(retired.seed.key());
    let highwater = retired.coord.next_operation;
    assert_eq!(
        retired.coord.admit_recovery_capture_origin(retired.binding, context(8, 107)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(retired.coord.next_operation, highwater);
    assert_eq!(retired.coord.snapshot(retired.seed.key()), before);
    Ok(())
}
#[test]
fn recovery_origin_exhaustion_preserves_report_and_copy_without_acknowledgement() -> TestResult {
    let mut ready = ready(1)?;
    present(&mut ready)?;
    ready.coord.next_operation = u64::MAX;
    let before = ready.coord.snapshot(ready.seed.key());
    assert_eq!(
        ready.coord.admit_recovery_capture_origin(ready.binding, context(4, 103)?),
        Err(IncidentDecisionError::OperationExhausted)
    );
    assert_eq!(ready.coord.next_operation, u64::MAX);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    assert_eq!(
        ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::CopyText),
            context(5, 104)?
        )?,
        IncidentReportDecision::CopyText { binding: ready.binding }
    );
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    assert_eq!(
        ready.coord.report_action(
            ReportCommand::new(ready.binding, ReportAction::Dismiss),
            context(6, 105)?
        ),
        Err(IncidentDecisionError::OperationExhausted)
    );
    assert_eq!(ready.coord.next_operation, u64::MAX);
    assert_eq!(ready.coord.snapshot(ready.seed.key()), before);
    Ok(())
}
