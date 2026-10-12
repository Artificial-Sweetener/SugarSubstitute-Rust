// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise durable report metadata through actual protected storage and execution owners.
use super::{
    publication::{admitted, setup},
    reporting_fixture::*,
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
#[test]
fn report_attempt_and_ack_preserve_actual_payload_inode_at_original_generation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let bytes = b"bounded redacted failure";
        let summary = retained(bytes)?;
        let payload = DiagnosticObject::Payload(
            summary.seed().key(),
            summary.revision(),
            AttachmentKind::Fault,
        );
        let old = fixture.put(payload, bytes)?;
        installed(&fixture, summary.clone(), Some(summary.revision()))?;
        let presented =
            repository.record_attempt(binding(&summary)?, ReportAttempt::Presented, &context)?;
        assert_eq!(presented.revision().get(), 3);
        assert_eq!(presented.payload_revision(), Some(summary.revision()));
        assert_eq!(presented.summary().facts().reporting().attempts(), 1);
        assert_eq!(presented.summary().facts().retention().acknowledged(), None);
        let ack = repository.acknowledge(
            ReportCommand::new(
                binding(presented.summary())?,
                ReportAction::DismissAndRequestRestart,
            ),
            IncidentUtc::from_unix_seconds(110)?,
            &context,
        )?;
        assert_eq!(ack.acknowledgement().revision().get(), 4);
        assert_eq!(ack.committed().payload_revision(), Some(summary.revision()));
        let actual = fixture.snapshot().active_identity(payload).ok_or("old payload absent")?;
        assert_eq!(actual.file_identity(), old.file_identity());
        assert_eq!(actual.sha256(), old.sha256());
        for revision in [3, 4] {
            assert!(
                fixture
                    .snapshot()
                    .active_identity(DiagnosticObject::Payload(
                        summary.seed().key(),
                        IncidentRevision::new(revision)?,
                        AttachmentKind::Fault,
                    ))
                    .is_none()
            );
        }
        let loaded = repository.load(summary.seed().key(), &context)?;
        assert_eq!(loaded.summary(), ack.committed().summary());
        assert_eq!(loaded.summary().facts().exit(), summary.facts().exit());
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn acknowledgement_repeat_reconciles_exact_current_revision_and_first_time() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        let first = repository.acknowledge(
            ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
            IncidentUtc::from_unix_seconds(110)?,
            &context,
        )?;
        let effects = fixture.snapshot().mutation_count();
        let repeat = repository.acknowledge(
            ReportCommand::new(binding(first.committed().summary())?, ReportAction::Dismiss),
            IncidentUtc::from_unix_seconds(90)?,
            &context,
        )?;
        assert_eq!(repeat.acknowledgement(), first.acknowledgement());
        assert_eq!(repeat.committed().content_sha256(), first.committed().content_sha256());
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        let failed = repository.record_attempt(
            binding(repeat.committed().summary())?,
            ReportAttempt::Failed(ReportFailure::UnexpectedExit),
            &context,
        )?;
        assert_eq!(
            failed.summary().facts().retention().acknowledged(),
            Some(first.acknowledgement().at())
        );
        assert_eq!(failed.summary().facts().reporting().attempts(), 1);
        assert_eq!(failed.revision().get(), 4);
        assert!(matches!(
            repository.acknowledge(
                ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
                IncidentUtc::from_unix_seconds(120)?,
                &context,
            ),
            Err(IncidentStoreError::StaleRevision)
        ));
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn report_stale_foreign_and_non_dismissal_requests_cannot_mutate() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        let before = fixture.snapshot().mutation_count();
        let stale = ReportBinding::new(
            summary.seed().key(),
            IncidentRevision::new(1)?,
            ReportSessionId::new(1)?,
        );
        assert!(matches!(
            repository.record_attempt(stale, ReportAttempt::Presented, &context),
            Err(IncidentStoreError::StaleRevision)
        ));
        let foreign = IncidentKey::new(IncidentSubject::Supervisor(CoordinatorKey {
            namespace: NamespaceId::new([8; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        }));
        assert!(matches!(
            repository.record_attempt(
                ReportBinding::new(foreign, summary.revision(), ReportSessionId::new(1)?),
                ReportAttempt::Presented,
                &context
            ),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        for action in
            [ReportAction::CopyText, ReportAction::Export(ExportSelection::TextAndNativeDump)]
        {
            assert!(matches!(
                repository.acknowledge(
                    ReportCommand::new(binding(&summary)?, action),
                    IncidentUtc::from_unix_seconds(110)?,
                    &context
                ),
                Err(IncidentStoreError::RecoveryConflict)
            ));
        }
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn report_counter_and_revision_exhaustion_reject_before_native_staging() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let base = original()?;
        for (revision, attempts) in [(2, u64::MAX), (u64::MAX, 0)] {
            let facts = base.facts();
            let summary = IncidentSummary::new(
                base.seed().clone(),
                IncidentRevision::new(revision)?,
                SummaryFacts::new(
                    facts.retention(),
                    facts.exit(),
                    facts.failure(),
                    facts.capture(),
                    facts.payload().clone(),
                    ReportDisposition::new(
                        attempts,
                        (attempts != 0).then_some(ReportAttempt::Presented),
                    )?,
                    facts.symbols().clone(),
                ),
            )?;
            installed(&fixture, summary.clone(), None)?;
            let before = fixture.snapshot().mutation_count();
            assert!(
                repository
                    .record_attempt(binding(&summary)?, ReportAttempt::Presented, &context)
                    .is_err()
            );
            assert_eq!(fixture.snapshot().mutation_count(), before);
        }
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn first_acknowledgement_rejects_time_before_occurrence_without_publication() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.acknowledge(
                ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
                IncidentUtc::from_unix_seconds(100)?,
                &context
            ),
            Err(IncidentStoreError::Facts(IncidentError::InvalidTime))
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn failed_ack_activation_never_returns_proof_and_retains_uncertain_stage() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        let failure = repository
            .acknowledge(
                ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
                IncidentUtc::from_unix_seconds(110)?,
                &context,
            )
            .err()
            .ok_or("unexpected acknowledgement proof")?;
        assert!(failure.source().is_some());
        assert_eq!(repository.snapshot().stages(), 1);
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        assert!(context.memory().claim(1).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(repository.retire(&context).is_err());
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn repeat_ack_close_failure_retains_read_and_cannot_claim_reconciliation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        let first = repository.acknowledge(
            ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
            IncidentUtc::from_unix_seconds(110)?,
            &context,
        )?;
        let before = fixture.snapshot().mutation_count();
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(
            repository
                .acknowledge(
                    ReportCommand::new(
                        binding(first.committed().summary())?,
                        ReportAction::Dismiss
                    ),
                    IncidentUtc::from_unix_seconds(120)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(repository.snapshot().reads(), 1);
        assert_eq!(fixture.snapshot().mutation_count(), before);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn report_load_failure_is_recordable_without_fabricating_payload_availability() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = retained(b"prior redacted evidence")?;
        installed(&fixture, summary.clone(), Some(summary.revision()))?;
        assert!(repository.load(summary.seed().key(), &context).is_err());
        let failed = repository.record_attempt(
            binding(&summary)?,
            ReportAttempt::Failed(ReportFailure::Load),
            &context,
        )?;
        assert_eq!(
            failed.summary().facts().reporting().last(),
            Some(ReportAttempt::Failed(ReportFailure::Load))
        );
        assert_eq!(failed.summary().facts().payload(), summary.facts().payload());
        assert_eq!(failed.payload_revision(), Some(summary.revision()));
        assert_eq!(failed.summary().facts().retention().acknowledged(), None);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn live_publication_ticket_blocks_report_metadata_before_effects() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        let _ticket = repository.begin_publish(
            summary.seed().clone(),
            ExpectedSummary::Absent,
            IncidentOperationId::new(1)?,
            &context,
        )?;
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.record_attempt(binding(&summary)?, ReportAttempt::Presented, &context),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert!(matches!(
            repository.acknowledge(
                ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
                IncidentUtc::from_unix_seconds(110)?,
                &context
            ),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        Ok(())
    })
}
