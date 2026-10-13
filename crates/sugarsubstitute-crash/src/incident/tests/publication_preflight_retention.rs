// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual pressure, retention settlement and preserved uncertain custody.
use super::publication_fixture::{admitted, setup};
use super::publication_preflight::{demand, initial, now};
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
fn installed(
    fixture: &DiagnosticStorageFixture,
    instance: u64,
    ack: Option<IncidentUtc>,
) -> TestResult {
    let payload = b"retained exact redacted text";
    let base = super::reporting_fixture::retained(payload)?;
    let key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner: owner(2).map_err(|cause| std::io::Error::other(cause.to_string()))?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(4)?,
    }));
    let seed = IncidentSeed::new(
        key,
        base.seed().revision(),
        base.seed().build().clone(),
        base.seed().created(),
        base.seed().stage(),
    )?;
    let facts = base.facts();
    let summary = IncidentSummary::new(
        seed,
        base.revision(),
        SummaryFacts::new(
            RetentionFacts::new(facts.retention().occurred(), ack)?,
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?;
    fixture.put(DiagnosticObject::Payload(key, base.revision(), AttachmentKind::Fault), payload)?;
    fixture.put(
        DiagnosticObject::Summary(key),
        &encode_summary(&StoredIncidentSummary::new(summary, Some(base.revision()))?)?,
    )?;
    Ok(())
}
#[test]
fn thirty_two_actual_groups_settle_one_exact_intent_before_new_generation_admission() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        for instance in 10..42 {
            installed(&fixture, instance, None)?;
        }
        let token = repository.preflight_publication(
            initial()?,
            ExpectedSummary::Absent,
            demand(1)?,
            now()?,
            &context,
        )?;
        assert_eq!(
            repository.storage.inventory(&repository.operation, &context)?.payload_groups(),
            31
        );
        assert_eq!(repository.snapshot().reads(), 0);
        assert_eq!(repository.snapshot().stages(), 0);
        let oldest = IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner: owner(2).map_err(|cause| std::io::Error::other(cause.to_string()))?,
            instance: InstanceId::new(10)?,
            generation: RunGeneration::new(4)?,
        }));
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Payload(
                    oldest,
                    IncidentRevision::new(2)?,
                    AttachmentKind::Fault
                ))
                .is_none()
        );
        assert_eq!(
            repository.load(oldest, &context)?.summary().facts().payload(),
            &PayloadDisposition::Discarded(DiscardedPayload::new(
                PayloadDiscard::StoragePressure,
                28
            )?)
        );
        let _ticket = repository.begin_publish(token, IncidentOperationId::new(1)?, &context)?;
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn retirement_failure_during_age_retention_preserves_original_payload_and_custody() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        installed(&fixture, 10, Some(IncidentUtc::from_unix_seconds(110)?))?;
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(
            repository
                .enforce_retention(
                    IncidentUtc::from_unix_seconds(110 + ACKNOWLEDGED_RETENTION_SECONDS as i64)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(repository.snapshot().writer_held());
        assert!(context.memory().claim(1).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn unreferenced_payload_blocks_without_deletion_authority() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        fixture.put(
            DiagnosticObject::Payload(
                initial()?.key(),
                IncidentRevision::new(1)?,
                AttachmentKind::Fault,
            ),
            b"unknown historical payload",
        )?;
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(0)?,
                now()?,
                &context
            ),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn actual_decoded_acknowledgement_after_supplied_utc_blocks_before_publication_effects()
-> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        installed(&fixture, 10, Some(IncidentUtc::from_unix_seconds(201)?))?;
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(0)?,
                now()?,
                &context
            ),
            Err(IncidentStoreError::RecoveryConflict)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(repository.snapshot().writer_held());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn full_summary_store_fails_projection_before_seed_staging() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let base = summary().map_err(|cause| std::io::Error::other(cause.to_string()))?;
        for instance in 10..138 {
            let key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
                owner: owner(2).map_err(|cause| std::io::Error::other(cause.to_string()))?,
                instance: InstanceId::new(instance)?,
                generation: RunGeneration::new(4)?,
            }));
            let seed = IncidentSeed::new(
                key,
                base.seed().revision(),
                base.seed().build().clone(),
                base.seed().created(),
                base.seed().stage(),
            )?;
            let current = StoredIncidentSummary::new(
                IncidentSummary::new(seed, base.revision(), base.facts().clone())?,
                None,
            )?;
            fixture.put(DiagnosticObject::Summary(key), &encode_summary(&current)?)?;
        }
        let (inventory, records) = repository.publication_inventory(now()?, &context)?;
        assert_eq!(records.len(), MAX_SUMMARIES);
        assert_eq!(records.records.capacity(), MAX_SUMMARIES);
        assert_eq!(records.keys.capacity(), MAX_SUMMARIES);
        assert_eq!(repository.snapshot().reads(), 0);
        drop(records);
        drop(inventory);
        let changes = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.preflight_publication(
                initial()?,
                ExpectedSummary::Absent,
                demand(0)?,
                now()?,
                &context
            ),
            Err(IncidentStoreError::Capacity)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        assert_eq!(repository.snapshot().stages(), 0);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn actual_acknowledged_count_and_age_settle_serial_groups_without_summary_evictions() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        for instance in 10..31 {
            installed(&fixture, instance, Some(IncidentUtc::from_unix_seconds(110)?))?;
        }
        let first = repository.enforce_retention(now()?, &context)?;
        assert_eq!(first.reclaimed_payload_groups(), 1);
        assert_eq!(first.reclaimed_bytes(), 28);
        assert_eq!(first.payload_groups(), 20);
        assert_eq!(first.summaries(), 21);
        assert_eq!(first.evictions().acknowledged(), 0);
        let aged = repository.enforce_retention(
            IncidentUtc::from_unix_seconds(110 + ACKNOWLEDGED_RETENTION_SECONDS as i64)?,
            &context,
        )?;
        assert_eq!(aged.reclaimed_payload_groups(), 20);
        assert_eq!(aged.reclaimed_bytes(), 560);
        assert_eq!(aged.payload_groups(), 0);
        assert_eq!(aged.summaries(), 21);
        assert_eq!(aged.evictions().acknowledged(), 0);
        assert_eq!(repository.snapshot().reads(), 0);
        assert_eq!(repository.snapshot().stages(), 0);
        assert!(!repository.snapshot().disposition_retained());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
