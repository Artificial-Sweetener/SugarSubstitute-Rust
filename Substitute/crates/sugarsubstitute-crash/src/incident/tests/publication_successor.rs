// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve retained evidence until actual disposition settles before a successor publication.
use super::publication_fixture::{admitted, setup};
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
fn install(
    fixture: &DiagnosticStorageFixture,
) -> Result<StoredIncidentSummary, Box<dyn Error + Send + Sync>> {
    let bytes = b"exact original retained evidence";
    let base = super::reporting_fixture::retained(bytes)?;
    let facts = base.facts();
    let summary = IncidentSummary::new(
        base.seed().clone(),
        base.revision(),
        SummaryFacts::new(
            RetentionFacts::new(
                facts.retention().occurred(),
                Some(IncidentUtc::from_unix_seconds(110)?),
            )?,
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?;
    let stored = StoredIncidentSummary::new(summary, Some(base.revision()))?;
    fixture.put(
        DiagnosticObject::Payload(base.seed().key(), base.revision(), AttachmentKind::Fault),
        bytes,
    )?;
    fixture.put(DiagnosticObject::Summary(base.seed().key()), &encode_summary(&stored)?)?;
    Ok(stored)
}
fn demand() -> Result<IncidentPublicationDemand, IncidentStoreError> {
    IncidentPublicationDemand::new(1024, 1, 0, 131_072)
}
#[test]
fn exact_retained_predecessor_rejects_before_ticket_or_effects_and_preserves_original() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let original = install(&fixture)?;
        let key = original.summary().seed().key();
        let (_, before) = repository.find_summary(key, &context)?.ok_or("Summary missing")?;
        let payload = DiagnosticObject::Payload(
            key,
            original.payload_revision().ok_or("payload revision missing")?,
            AttachmentKind::Fault,
        );
        let identity = fixture.snapshot().active_identity(payload);
        let changes = fixture.snapshot().mutation_count();
        let result = repository.preflight_publication(
            original.summary().seed().clone(),
            ExpectedSummary::Exact {
                revision: original.summary().revision(),
                content_sha256: before.sha256(),
            },
            demand()?,
            IncidentUtc::from_unix_seconds(111)?,
            &context,
        );
        let error = result.err().ok_or("retained evidence admitted replacement")?;
        assert_eq!(error.code(), IncidentStoreFailureCode::RetainedPayloadRequiresDisposition);
        assert!(error.source().is_none());
        assert!(error.to_string().contains("settled protected disposition"));
        assert!(repository.preflight.is_none());
        assert_eq!(repository.snapshot().stages(), 0);
        assert_eq!(repository.snapshot().reads(), 0);
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        assert_eq!(fixture.snapshot().active_identity(payload), identity);
        let (current, after) = repository.find_summary(key, &context)?.ok_or("Summary missing")?;
        assert_eq!(encode_summary(&current)?, encode_summary(&original)?);
        assert_eq!(before, after);
        repository.verify_payloads(&current, &context)?;
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn age_eligible_actual_discard_settles_journal_before_successor_generation_publication()
-> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let original = install(&fixture)?;
        let key = original.summary().seed().key();
        let old_payload = DiagnosticObject::Payload(
            key,
            original.payload_revision().ok_or("payload revision missing")?,
            AttachmentKind::Fault,
        );
        let utc = IncidentUtc::from_unix_seconds(110 + ACKNOWLEDGED_RETENTION_SECONDS as i64)?;
        let retired = repository.enforce_retention(utc, &context)?;
        assert_eq!(retired.reclaimed_payload_groups(), 1);
        assert_eq!(retired.reclaimed_bytes(), 32);
        assert!(!repository.snapshot().disposition_retained());
        assert_eq!(repository.snapshot().reads(), 0);
        assert_eq!(repository.snapshot().stages(), 0);
        assert!(fixture.snapshot().active_identity(old_payload).is_none());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Journal(repository.operation()))
                .is_none()
        );
        let (discarded, actual) =
            repository.find_summary(key, &context)?.ok_or("discarded Summary missing")?;
        assert!(matches!(discarded.summary().facts().payload(), PayloadDisposition::Discarded(_)));
        assert!(discarded.payload_revision().is_none());
        let changes = fixture.snapshot().mutation_count();
        let token = repository.preflight_publication(
            original.summary().seed().clone(),
            ExpectedSummary::Exact {
                revision: discarded.summary().revision(),
                content_sha256: actual.sha256(),
            },
            demand()?,
            utc,
            &context,
        )?;
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        let mut ticket = repository.begin_publish(token, IncidentOperationId::new(1)?, &context)?;
        let chunk = RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
            FailureEvidence::Unexplained,
        )])?;
        let attachment = AttachmentFacts::new(
            AttachmentKind::Fault,
            chunk.as_bytes().len() as u64,
            chunk.original_bytes(),
            chunk.omitted_bytes(),
            super::super::codec_ledger::digest(chunk.as_bytes()),
        )?;
        repository.append_text(&mut ticket, AttachmentKind::Fault, chunk, &context)?;
        let facts = discarded.summary().facts();
        let next = IncidentSummary::new(
            original.summary().seed().clone(),
            discarded.summary().revision().next()?,
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                PayloadDisposition::Retained(AttachmentInventory::new(&[attachment])?),
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?;
        let commit = repository.commit(&mut ticket, next, &context)?;
        assert_eq!(commit.payload_revision(), Some(commit.revision()));
        assert!(commit.revision() > original.summary().revision());
        assert_eq!(repository.load(key, &context)?.payload_revision(), Some(commit.revision()));
        assert!(fixture.snapshot().active_identity(old_payload).is_none());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Payload(
                    key,
                    commit.revision(),
                    AttachmentKind::Fault
                ))
                .is_some()
        );
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn uncertain_discard_close_rejects_successor_without_new_effects_until_actual_settlement()
-> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared =
            super::retention_fixture::prepare(&fixture, &mut repository, &context, false)?;
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        let mut permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(
            repository
                .storage
                .complete_payload_discard(&mut permit, &repository.operation, &context)
                .is_err()
        );
        let before = fixture.snapshot().mutation_count();
        let summary = prepared.original.summary();
        let summary_object = DiagnosticObject::Summary(summary.seed().key());
        let summary_identity = fixture.snapshot().active_identity(summary_object);
        let payload_identity = fixture.snapshot().active_identity(prepared.payload);
        assert!(
            repository
                .preflight_publication(
                    summary.seed().clone(),
                    ExpectedSummary::Exact {
                        revision: summary.revision(),
                        content_sha256: super::super::codec_ledger::digest(&encode_summary(
                            &prepared.original
                        )?),
                    },
                    demand()?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(fixture.snapshot().active_identity(summary_object), summary_identity);
        assert_eq!(fixture.snapshot().active_identity(prepared.payload), payload_identity);
        assert!(repository.preflight.is_none());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Journal(repository.operation()))
                .is_some()
        );
        assert!(repository.snapshot().writer_held());
        assert_eq!(repository.snapshot().reads(), 1);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        repository.storage.finish_payload_discard(
            &mut permit,
            &completion,
            &repository.operation,
            &context,
        )?;
        assert!(fixture.snapshot().active_identity(prepared.payload).is_none());
        assert!(!repository.snapshot().disposition_retained());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
