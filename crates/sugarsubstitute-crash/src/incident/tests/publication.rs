// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real repository commits and custody through the protected external fixture.
use super::publication_fixture::{admitted, setup};
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
fn test_seed() -> Result<IncidentSeed, TestError> {
    super::seed().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
fn test_summary() -> Result<IncidentSummary, TestError> {
    super::summary().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
#[test]
fn repository_seed_actual_repeat_and_changed_content_preserves_original() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let seed = test_seed()?;
        let first = repository.publish_seed(seed.clone(), &context)?;
        let changes = fixture.snapshot().mutation_count();
        let repeat = repository.publish_seed(seed.clone(), &context)?;
        assert_eq!(first.content_sha256(), repeat.content_sha256());
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        let changed = IncidentSeed::new(
            seed.key(),
            seed.revision(),
            seed.build().clone(),
            seed.created(),
            IncidentStage::Running,
        )?;
        assert!(matches!(
            repository.publish_seed(changed, &context),
            Err(IncidentStoreError::ChangedDuplicate)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        assert_eq!(repository.snapshot().reads(), 1);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn repository_summary_commit_load_and_stale_cas_are_actual() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = test_summary()?;
        let mut ticket = {
            let expected = ExpectedSummary::Absent;
            repository
                .preflight_publication(
                    summary.seed().clone(),
                    expected,
                    IncidentPublicationDemand::new(
                        262_144,
                        1,
                        usize::from(matches!(expected, ExpectedSummary::Absent)),
                        131_072,
                    )?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context,
                )
                .and_then(|preflight| {
                    repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                })
        }?;
        let committed = repository.commit(&mut ticket, summary.clone(), &context)?;
        assert_eq!(committed.summary(), &summary);
        assert_eq!(
            committed.content_sha256(),
            super::super::codec_ledger::digest(&encode_summary(&StoredIncidentSummary::new(
                summary.clone(),
                None
            )?)?)
        );
        let loaded = repository.load(committed.key(), &context)?;
        assert_eq!(loaded.summary(), &summary);
        assert_eq!(loaded.evictions().pending(), 0);
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            {
                let expected = ExpectedSummary::Absent;
                repository
                    .preflight_publication(
                        summary.seed().clone(),
                        expected,
                        IncidentPublicationDemand::new(
                            262_144,
                            1,
                            usize::from(matches!(expected, ExpectedSummary::Absent)),
                            131_072,
                        )?,
                        IncidentUtc::from_unix_seconds(111)?,
                        &context,
                    )
                    .and_then(|preflight| {
                        repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                    })
            },
            Err(IncidentStoreError::StaleRevision)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        let mut next = {
            let expected = ExpectedSummary::Exact {
                revision: committed.revision(),
                content_sha256: committed.content_sha256(),
            };
            repository
                .preflight_publication(
                    summary.seed().clone(),
                    expected,
                    IncidentPublicationDemand::new(
                        262_144,
                        1,
                        usize::from(matches!(expected, ExpectedSummary::Absent)),
                        131_072,
                    )?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context,
                )
                .and_then(|preflight| {
                    repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                })
        }?;
        let updated = IncidentSummary::new(
            summary.seed().clone(),
            summary.revision().next()?,
            summary.facts().clone(),
        )?;
        assert_eq!(repository.commit(&mut next, updated, &context)?.revision().get(), 3);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn repository_failed_activation_keeps_stage_and_physical_charge() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(matches!(
            repository.publish_seed(test_seed()?, &context),
            Err(IncidentStoreError::Storage(_))
        ));
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
fn repository_ticket_drop_keeps_admission_and_foreign_operation_cannot_publish() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        assert!(
            {
                let expected = ExpectedSummary::Absent;
                repository
                    .preflight_publication(
                        test_seed()?,
                        expected,
                        IncidentPublicationDemand::new(
                            262_144,
                            1,
                            usize::from(matches!(expected, ExpectedSummary::Absent)),
                            131_072,
                        )?,
                        IncidentUtc::from_unix_seconds(111)?,
                        &context,
                    )
                    .and_then(|preflight| {
                        repository.begin_publish(preflight, IncidentOperationId::new(2)?, &context)
                    })
            }
            .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        assert!(
            repository
                .preflight_publication(
                    test_seed()?,
                    ExpectedSummary::Absent,
                    IncidentPublicationDemand::new(0, 0, 1, 131_072)?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context
                )
                .is_err()
        );
        repository.retire(&context)?;
        drop(repository);
        let (_, mut repository) = setup(&context)?;
        {
            let _ticket = {
                let expected = ExpectedSummary::Absent;
                repository
                    .preflight_publication(
                        test_seed()?,
                        expected,
                        IncidentPublicationDemand::new(
                            262_144,
                            1,
                            usize::from(matches!(expected, ExpectedSummary::Absent)),
                            131_072,
                        )?,
                        IncidentUtc::from_unix_seconds(111)?,
                        &context,
                    )
                    .and_then(|preflight| {
                        repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                    })
            }?;
        }
        assert!(
            {
                let expected = ExpectedSummary::Absent;
                repository
                    .preflight_publication(
                        test_seed()?,
                        expected,
                        IncidentPublicationDemand::new(
                            262_144,
                            1,
                            usize::from(matches!(expected, ExpectedSummary::Absent)),
                            131_072,
                        )?,
                        IncidentUtc::from_unix_seconds(111)?,
                        &context,
                    )
                    .and_then(|preflight| {
                        repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                    })
            }
            .is_err()
        );
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn repository_text_payload_requires_actual_digest_and_changed_source_blocks_load() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let original = test_summary()?;
        let mut ticket = {
            let expected = ExpectedSummary::Absent;
            repository
                .preflight_publication(
                    original.seed().clone(),
                    expected,
                    IncidentPublicationDemand::new(
                        262_144,
                        1,
                        usize::from(matches!(expected, ExpectedSummary::Absent)),
                        131_072,
                    )?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context,
                )
                .and_then(|preflight| {
                    repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)
                })
        }?;
        let chunk = RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
            FailureEvidence::Unexplained,
        )])?;
        let bytes = chunk.as_bytes().to_vec();
        let actual_digest = super::super::codec_ledger::digest(&bytes);
        let original_bytes = chunk.original_bytes();
        let omitted = chunk.omitted_bytes();
        repository.append_text(&mut ticket, AttachmentKind::Fault, chunk, &context)?;
        let summary = |hash| -> Result<IncidentSummary, IncidentStoreError> {
            let facts = original.facts();
            Ok(IncidentSummary::new(
                original.seed().clone(),
                original.revision(),
                SummaryFacts::new(
                    facts.retention(),
                    facts.exit(),
                    facts.failure(),
                    facts.capture(),
                    PayloadDisposition::Retained(AttachmentInventory::new(&[
                        AttachmentFacts::new(
                            AttachmentKind::Fault,
                            bytes.len() as u64,
                            original_bytes,
                            omitted,
                            hash,
                        )?,
                    ])?),
                    facts.reporting(),
                    facts.symbols().clone(),
                ),
            )?)
        };
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.commit(&mut ticket, summary([9; 32])?, &context),
            Err(IncidentStoreError::WrongDigest)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        let committed = repository.commit(&mut ticket, summary(actual_digest)?, &context)?;
        assert_eq!(committed.payload_revision(), Some(committed.revision()));
        assert_eq!(repository.load(committed.key(), &context)?.summary(), committed.summary());
        let facts = committed.summary().facts();
        let advanced = IncidentSummary::new(
            committed.summary().seed().clone(),
            committed.revision().next()?,
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
                facts.symbols().clone(),
            ),
        )?;
        let stored = StoredIncidentSummary::new(advanced.clone(), committed.payload_revision())?;
        fixture.put(DiagnosticObject::Summary(committed.key()), &encode_summary(&stored)?)?;
        let before = fixture.snapshot().mutation_count();
        let loaded = repository.load(committed.key(), &context)?;
        assert_eq!(loaded.summary(), &advanced);
        assert_eq!(loaded.payload_revision(), Some(committed.revision()));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        fixture.put(
            DiagnosticObject::Payload(committed.key(), committed.revision(), AttachmentKind::Fault),
            &vec![b'x'; bytes.len()],
        )?;
        assert!(matches!(
            repository.load(committed.key(), &context),
            Err(IncidentStoreError::WrongDigest)
        ));
        assert_eq!(repository.snapshot().reads(), 1);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
