// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual finite preflight and effects through the protected external boundary.
use super::publication_fixture::{admitted, admitted_handoff, setup};
use super::*;
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
pub(super) fn initial() -> Result<IncidentSeed, Box<dyn Error + Send + Sync>> {
    seed().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
pub(super) fn now() -> Result<IncidentUtc, IncidentError> {
    IncidentUtc::from_unix_seconds(111)
}
pub(super) fn demand(bytes: u64) -> Result<IncidentPublicationDemand, IncidentStoreError> {
    IncidentPublicationDemand::new(bytes, usize::from(bytes != 0), 1, 131_072)
}
#[test]
fn preflight_checks_literal_projected_edges_and_nonwrapping_demand() -> TestResult {
    assert!(IncidentPublicationDemand::new(0, 0, 1, 131_071).is_err());
    assert!(IncidentPublicationDemand::new(0, 1, 1, 131_072).is_err());
    assert!(IncidentPublicationDemand::new(1, 0, 1, 131_072).is_err());
    assert!(IncidentPublicationDemand::new(0, 0, 2, 131_072).is_err());
    assert!(IncidentPublicationDemand::new(u64::MAX, 1, 1, 131_072).is_err());
    let boundary = IncidentPublicationDemand::new(MAX_PAYLOAD_BYTES - 131_072, 1, 1, 131_072)?;
    assert_eq!(boundary.payload_bytes() + boundary.staging_headroom(), MAX_PAYLOAD_BYTES);
    assert_eq!(boundary.payload_groups(), 1);
    assert_eq!(boundary.summary_slots(), 1);
    assert!(IncidentPublicationDemand::new(MAX_PAYLOAD_BYTES - 131_071, 1, 1, 131_072).is_err());
    Ok(())
}
#[test]
fn cumulative_payload_overshoot_rejects_before_actual_stage_or_activation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let chunk = RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
            FailureEvidence::Unexplained,
        )])?;
        let bytes = chunk.as_bytes().len() as u64;
        let preflight = repository.preflight_publication(
            initial()?,
            ExpectedSummary::Absent,
            demand(bytes)?,
            now()?,
            &context,
        )?;
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        let mut ticket =
            repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)?;
        repository.append_text(&mut ticket, AttachmentKind::Fault, chunk, &context)?;
        let mutations = fixture.snapshot().mutation_count();
        let next = RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
            FailureEvidence::Unexplained,
        )])?;
        assert!(matches!(
            repository.append_text(&mut ticket, AttachmentKind::Log, next, &context),
            Err(IncidentStoreError::Capacity)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), mutations);
        assert_eq!(repository.snapshot().stages(), 0);
        assert!(
            repository
                .preflight_publication(
                    initial()?,
                    ExpectedSummary::Absent,
                    demand(0)?,
                    now()?,
                    &context
                )
                .is_err()
        );
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn dropped_preflight_keeps_fence_and_report_metadata_cannot_bypass_it() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let token = repository.preflight_publication(
            initial()?,
            ExpectedSummary::Absent,
            demand(0)?,
            now()?,
            &context,
        )?;
        drop(token);
        assert!(
            repository
                .preflight_publication(
                    initial()?,
                    ExpectedSummary::Absent,
                    demand(0)?,
                    now()?,
                    &context
                )
                .is_err()
        );
        assert!(repository.publish_seed(initial()?, &context).is_err());
        let binding = ReportBinding::new(
            initial()?.key(),
            IncidentRevision::new(2)?,
            ReportSessionId::new(1)?,
        );
        assert!(matches!(
            repository.record_attempt(binding, ReportAttempt::Presented, &context),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn foreign_repository_token_cannot_consume_same_numbered_current_admission() -> TestResult {
    admitted_handoff(
        |context| {
            let (_, mut prior) = setup(&context)?;
            let foreign = prior.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(0)?,
                now()?,
                &context,
            )?;
            prior.retire(&context)?;
            drop(prior);
            Ok(foreign)
        },
        |context, foreign| {
            let (fixture, mut current) = setup(&context)?;
            let valid = current.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(0)?,
                now()?,
                &context,
            )?;
            assert!(
                current.begin_publish(foreign, IncidentOperationId::new(1)?, &context).is_err()
            );
            assert_eq!(fixture.snapshot().mutation_count(), 0);
            let _ticket = current.begin_publish(valid, IncidentOperationId::new(1)?, &context)?;
            current.retire(&context)?;
            Ok(())
        },
    )
}
#[test]
fn rejected_preflight_id_is_not_reused_and_regressing_utc_rejects_before_effects() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        assert!(
            repository
                .preflight_publication(
                    initial()?,
                    ExpectedSummary::Absent,
                    demand(0)?,
                    IncidentUtc::from_unix_seconds(99)?,
                    &context
                )
                .is_err()
        );
        let token = repository.preflight_publication(
            initial()?,
            ExpectedSummary::Absent,
            demand(0)?,
            now()?,
            &context,
        )?;
        assert_eq!(repository.next_ticket, 3);
        repository.retire(&context)?;
        assert!(repository.begin_publish(token, IncidentOperationId::new(1)?, &context).is_err());
        assert!(
            repository
                .preflight_publication(
                    initial()?,
                    ExpectedSummary::Absent,
                    demand(0)?,
                    IncidentUtc::from_unix_seconds(110)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        Ok(())
    })
}

#[test]
fn retired_publish_ticket_cannot_append_to_another_repository_generation() -> TestResult {
    admitted_handoff(
        |context| {
            let (_, mut prior) = setup(&context)?;
            let token = prior.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(1024)?,
                now()?,
                &context,
            )?;
            let foreign = prior.begin_publish(token, IncidentOperationId::new(1)?, &context)?;
            prior.retire(&context)?;
            drop(prior);
            Ok(foreign)
        },
        |context, mut foreign| {
            let (fixture, mut current) = setup(&context)?;
            let token = current.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(1024)?,
                now()?,
                &context,
            )?;
            let mut valid = current.begin_publish(token, IncidentOperationId::new(1)?, &context)?;
            let before = fixture.snapshot().mutation_count();
            let chunk = || {
                RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
                    FailureEvidence::Unexplained,
                )])
            };
            assert!(matches!(
                current.append_text(&mut foreign, AttachmentKind::Fault, chunk()?, &context),
                Err(IncidentStoreError::RecoveryConflict)
            ));
            assert_eq!(fixture.snapshot().mutation_count(), before);
            current.append_text(&mut valid, AttachmentKind::Fault, chunk()?, &context)?;
            current.retire(&context)?;
            Ok(())
        },
    )
}
