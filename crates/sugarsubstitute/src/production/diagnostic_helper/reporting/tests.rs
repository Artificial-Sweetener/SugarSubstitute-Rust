// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact report products, unchanged payload generation and independent finite cleanup.
#[path = "tests/snapshot.rs"]
mod snapshot;
#[path = "tests/support.rs"]
mod support;
use super::*;
#[path = "tests/passive.rs"]
mod passive;
#[path = "tests/restoration.rs"]
mod restoration;
use sugarsubstitute_supervision::diagnostic_storage::fixture::*;

#[test]
fn reporting_presentation_and_dismissal_require_original_authenticated_frame() -> TestResult {
    let binding = binding(&original()?)?;
    assert!(
        super::super::authentication::expected_command(
            &ReportingWork::Attempt { binding, attempt: ReportAttempt::Presented },
            false,
        )
        .is_err()
    );
    for action in [ReportAction::Dismiss, ReportAction::DismissAndRequestRestart] {
        assert!(
            super::super::authentication::expected_command(
                &ReportingWork::Acknowledge {
                    command: ReportCommand::new(binding, action),
                    at: IncidentUtc::from_unix_seconds(110)?,
                },
                false
            )
            .is_err()
        );
    }
    for action in [ReportAction::CopyText, ReportAction::Export(ExportSelection::TextOnly)] {
        assert!(
            super::super::authentication::expected_command(
                &ReportingWork::Acknowledge {
                    command: ReportCommand::new(binding, action),
                    at: IncidentUtc::from_unix_seconds(110)?,
                },
                true
            )
            .is_err()
        );
    }
    Ok(())
}
use support::*;

#[test]
fn reporting_original_controller_admits_source_and_rejects_borrowing_text_credit() -> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        let service = ReportingService::new(&context)?;
        let policy_bytes = crate::production::incident_host::CONTROLLER_BYTES
            - size_of::<crate::production::incident_host::PreparationInput>() as u64
            - crate::production::diagnostic_helper::SEED_SOURCE_BYTES
            - crate::production::diagnostic_helper::collection::COLLECTION_SOURCE_BYTES
            - crate::production::diagnostic_helper::exporting::EXPORT_SOURCE_BYTES
            - crate::production::reporter_role::parent::REPORTER_METADATA_BYTES
            - REPORT_SOURCE_BYTES;
        let required = sugarsubstitute_application::incident::IncidentCoordinator::required_storage_bytes()
            + size_of::<crate::production::incident_host::PolicyState>() as u64;
        assert!(required <= policy_bytes);
        let occupied = context.memory().claim(2 * 1024 * 1024 - REPORT_SOURCE_BYTES)?;
        let rejected = context.memory().claim(ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES)
            .err().ok_or("controller funded unexpected text")?;
        assert_eq!(rejected.available_bytes, 0);
        println!("controller policy required={required}, source-only remainder={policy_bytes}, source={REPORT_SOURCE_BYTES}");
        Ok::<_, TestError>((service, occupied))
    })?;
    let (service, occupied) = success(completed(&client, &wake, &mut controller)?)?;
    drop(controller);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 2 * 1024 * 1024);
    drop(occupied);
    assert_eq!(scope.snapshot().reserved_bytes, 2 * 1024 * 1024);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn reporting_actual_view_uses_separate_finite_credit_and_two_nonclone_disposal_slots() -> TestResult
{
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        ReportingService::new(&context)
    })?;
    let mut service = success(completed(&client, &wake, &mut controller)?)?;
    drop(controller);
    assert!(service.outcome().is_none());
    assert!(service.cleanup_outcome().is_none());
    assert!(service.view_outcome().is_none());
    assert!(!service.cleanup_started());
    assert!(service.take_authentication().is_none());
    assert!(service.take_retiring_authentication().is_none());
    let source = service.source.alias();
    service.work.job = Some(client.submit_blocking(
        &scope,
        spec(2, DIAGNOSTIC_STORAGE_BYTES)?,
        move |context| {
            let fixture = DiagnosticStorageFixture::new().map_err(|_| ReportingIssue::Busy)?;
            let mut loan = source.loan()?;
            let operation = loan.operation()?;
            *operation = prepared(&context, &fixture).map_err(|_| ReportingIssue::Busy)?;
            let original = original().map_err(|_| ReportingIssue::Busy)?;
            let original_binding = binding(&original).map_err(|_| ReportingIssue::Busy)?;
            let payload = DiagnosticObject::Payload(
                original.seed().key(),
                original.revision(),
                AttachmentKind::Fault,
            );
            let before = fixture.snapshot().active_identity(payload).ok_or(ReportingIssue::Busy)?;
            operation
                .record_attempt(original_binding, ReportAttempt::Presented, &context)
                .map_err(|_| ReportingIssue::Busy)?;
            let proof = operation.attempt().ok_or(ReportingIssue::Busy)?;
            assert_eq!(proof.payload_revision(), Some(original.revision()));
            let revised = binding(proof.summary()).map_err(|_| ReportingIssue::Busy)?;
            operation
                .acknowledge(
                    ReportCommand::new(revised, ReportAction::Dismiss),
                    IncidentUtc::from_unix_seconds(110).map_err(|_| ReportingIssue::Busy)?,
                    &context,
                )
                .map_err(|_| ReportingIssue::Busy)?;
            let ack = operation.acknowledgement().ok_or(ReportingIssue::Busy)?;
            assert_eq!(ack.committed().payload_revision(), Some(original.revision()));
            assert_eq!(ack.committed().summary().facts().failure(), original.facts().failure());
            assert_eq!(ack.committed().summary().facts().exit(), original.facts().exit());
            assert_eq!(
                ack.acknowledgement().at(),
                IncidentUtc::from_unix_seconds(110).map_err(|_| ReportingIssue::Busy)?
            );
            let current = binding(ack.committed().summary()).map_err(|_| ReportingIssue::Busy)?;
            operation
                .load_current(current, "ja", true, &context)
                .map_err(|_| ReportingIssue::Busy)?;
            let after = fixture.snapshot().active_identity(payload).ok_or(ReportingIssue::Busy)?;
            assert_eq!(before.file_identity(), after.file_identity());
            assert_eq!(before.sha256(), after.sha256());
            assert!(operation.view.is_none());
            assert!(context.memory().claim(1).is_err());
            Ok(true)
        },
    )?);
    delivered(&client, &wake, &mut service, false)?;
    assert_eq!(service.outcome(), Some(TerminalKind::Success));
    assert!(service.take_authentication().is_none());
    service.requested = Some((scope.owner_id(), scope.id(), Moment::from_nanos(100)));
    let jobs = crate::job_sequence::JobSequence::new();
    jobs.allocate()?;
    jobs.allocate()?;
    assert!(service.take_view().is_none());
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    assert!(service.cleanup_started());
    delivered(&client, &wake, &mut service, true)?;
    assert_eq!(service.cleanup_outcome(), Some(TerminalKind::Success));
    assert!(service.files_retired());
    assert!(service.submit_view(&client, &scope, &jobs, Moment::from_nanos(101)).is_err());
    service.submit_view(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered_view(&client, &wake, &mut service)?;
    assert_eq!(service.view_outcome(), Some(TerminalKind::Success));
    let view = service.take_view().ok_or("actual charged view absent")?;
    let current = service
        .inspect(|operation| {
            let loaded = operation.loaded.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
            Ok::<_, IncidentStoreError>((
                StoredIncidentSummary::new(loaded.summary().clone(), loaded.payload_revision())?,
                loaded.evictions(),
            ))
        })
        .ok_or("actual source unavailable")??;
    snapshot::check(&view, &current.0, current.1)?;
    assert!(view.view().text().ends_with('\n'));
    assert!(!view.view().text().contains('\r'));
    assert!(view.view().text().contains("レポート"));
    assert!(view.view().text().contains(
        sugarsubstitute_contracts::localization::messages("ja").report.recovery_capture_degraded
    ));
    assert!(view.view().text().len() <= ReportView::MAX_BYTES);
    assert_eq!(view.view().binding().revision().get(), 4);
    let second = service.source.reserve_view()?;
    assert!(service.source.reserve_view().is_err());
    drop(second);
    let second = service.source.reserve_view()?;
    drop(second);
    drop(owner);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024 + REPORT_VIEW_JOB_BYTES
    );
    drop(service);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024 + REPORT_VIEW_JOB_BYTES
    );
    drop(view);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn reporting_uncertain_ack_preserves_first_native_cause_and_actual_cleanup_failure() -> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        ReportingService::new(&context)
    })?;
    let mut service = success(completed(&client, &wake, &mut controller)?)?;
    drop(controller);
    let source = service.source.alias();
    service.work.job = Some(client.submit_blocking(
        &scope,
        spec(2, DIAGNOSTIC_STORAGE_BYTES)?,
        move |context| {
            let fixture = DiagnosticStorageFixture::new().map_err(|_| ReportingIssue::Busy)?;
            let mut loan = source.loan()?;
            let operation = loan.operation()?;
            *operation = prepared(&context, &fixture).map_err(|_| ReportingIssue::Busy)?;
            let original = original().map_err(|_| ReportingIssue::Busy)?;
            fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
            operation.first = operation
                .acknowledge(
                    ReportCommand::new(
                        binding(&original).map_err(|_| ReportingIssue::Busy)?,
                        ReportAction::Dismiss,
                    ),
                    IncidentUtc::from_unix_seconds(110).map_err(|_| ReportingIssue::Busy)?,
                    &context,
                )
                .err();
            assert!(operation.failure().and_then(std::error::Error::source).is_some());
            assert!(operation.acknowledgement().is_none());
            fixture.set_failure(DiagnosticFixtureFailure::Close, true);
            assert!(!operation.retire(&context));
            assert!(operation.retirement.is_some());
            assert!(operation.failure().and_then(std::error::Error::source).is_some());
            assert!(context.memory().claim(1).is_err());
            fixture.set_failure(DiagnosticFixtureFailure::Close, false);
            Ok(false)
        },
    )?);
    delivered(&client, &wake, &mut service, false)?;
    assert!(!service.work.succeeded());
    assert_eq!(service.outcome(), Some(TerminalKind::Success));
    assert!(service.take_view().is_none());
    assert!(service.take_failure().is_none());
    assert!(service.take_authentication().is_none());
    assert!(service.take_retiring_authentication().is_none());
    service.requested = Some((scope.owner_id(), scope.id(), Moment::from_nanos(100)));
    let jobs = crate::job_sequence::JobSequence::new();
    jobs.allocate()?;
    jobs.allocate()?;
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered(&client, &wake, &mut service, true)?;
    assert!(service.files_retired());
    assert_eq!(service.inspect(|operation| operation.acknowledgement().is_none()), Some(true));
    assert_eq!(
        service
            .inspect(|operation| operation.failure().and_then(std::error::Error::source).is_some()),
        Some(true)
    );
    assert!(service.release_adopted().is_err());
    let failure = service.take_failure().ok_or("original checked-close cause absent")?;
    assert!(std::error::Error::source(&failure).and_then(std::error::Error::source).is_some());
    assert!(service.take_failure().is_none());
    assert_eq!(service.inspect(ReportingOperation::failed), Some(true));
    assert!(service.take_authentication().is_none());
    assert!(service.release_adopted().is_err());
    drop(owner);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024
    );
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(failure);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn reporting_stale_view_retains_actual_loaded_summary_without_text_or_mutation() -> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut job =
        client.submit_blocking(&scope, spec(1, DIAGNOSTIC_STORAGE_BYTES)?, |context| {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut operation = prepared(&context, &fixture)?;
            let original = original()?;
            let stale = ReportBinding::new(
                original.seed().key(),
                IncidentRevision::new(1)?,
                ReportSessionId::new(7)?,
            );
            let before = fixture.snapshot().mutation_count();
            operation.first = operation.load_current(stale, "en", false, &context).err();
            assert!(matches!(
                operation.failure(),
                Some(ReportingCause::Store(IncidentStoreError::StaleRevision))
            ));
            assert_eq!(
                operation.loaded.as_ref().ok_or("loaded summary absent")?.summary(),
                &original
            );
            assert!(operation.view.is_none());
            assert!(operation.attempt().is_none());
            assert!(operation.acknowledgement().is_none());
            assert_eq!(fixture.snapshot().mutation_count(), before);
            assert!(operation.retire(&context));
            Ok::<_, TestError>(operation)
        })?;
    let operation = success(completed(&client, &wake, &mut job)?)?;
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(operation);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
