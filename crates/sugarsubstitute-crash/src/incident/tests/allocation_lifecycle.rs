// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual original-claim lifetime independently of native retirement.
use super::{
    publication_fixture::{admitted, setup},
    reporting_fixture, *,
};
use sugarsubstitute_supervision::diagnostic_storage::{fixture::DiagnosticFixtureFailure, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
fn seed() -> Result<IncidentSeed, TestError> {
    super::seed().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}

#[test]
fn escaped_seed_keeps_original_charge_after_repository_retirement() -> TestResult {
    admitted(|context| {
        let (_, mut repository) = setup(&context)?;
        let committed = repository.publish_seed(seed()?, &context)?;
        repository.retire(&context)?;
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        assert_eq!(committed.key(), seed()?.key());
        drop(committed);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn concurrent_report_results_have_independent_custody_after_native_close() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = reporting_fixture::original()?;
        reporting_fixture::installed(&fixture, summary.clone(), None)?;
        let presented = repository.record_attempt(
            reporting_fixture::binding(&summary)?,
            ReportAttempt::Presented,
            &context,
        )?;
        let acknowledged = repository.acknowledge(
            ReportCommand::new(
                reporting_fixture::binding(presented.summary())?,
                ReportAction::Dismiss,
            ),
            IncidentUtc::from_unix_seconds(110)?,
            &context,
        )?;
        let loaded = repository.load(summary.seed().key(), &context)?;
        assert_eq!(loaded.summary(), acknowledged.committed().summary());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        drop(repository);
        drop(presented);
        assert!(context.memory().claim(1).is_err());
        drop(acknowledged);
        assert!(context.memory().claim(1).is_err());
        drop(loaded);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn actual_codec_error_keeps_phase_and_original_json_position_after_owner_drop() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        fixture.put(DiagnosticObject::Summary(seed()?.key()), b"{\n\"schema_version\":2,")?;
        let error =
            repository.load(seed()?.key(), &context).err().ok_or("Malformed record accepted")?;
        let parser = error
            .source()
            .and_then(|source| source.downcast_ref::<serde_json::Error>())
            .ok_or("Original parser cause absent")?;
        assert!(parser.is_eof());
        assert_eq!(parser.line(), 2);
        assert!(parser.column() > 0);
        assert_eq!(error.code(), IncidentStoreFailureCode::Codec);
        repository.retire(&context)?;
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn dropped_preflight_keeps_private_admission_until_checked_retirement() -> TestResult {
    admitted(|context| {
        let (_, mut repository) = setup(&context)?;
        let token = repository.preflight_publication(
            seed()?,
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(0, 0, 1, 131_072)?,
            IncidentUtc::from_unix_seconds(111)?,
            &context,
        )?;
        drop(token);
        assert!(repository.preflight.is_some());
        assert!(
            repository
                .preflight_publication(
                    seed()?,
                    ExpectedSummary::Absent,
                    IncidentPublicationDemand::new(0, 0, 1, 131_072)?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context
                )
                .is_err()
        );
        repository.retire(&context)?;
        drop(repository);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn exclusive_collection_rejection_precedes_publication_and_returns_on_actual_drop() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let custody = repository.storage.recovery_collection(&repository.operation, &context)?;
        let collection = super::super::allocation::SummaryCollection::new(custody)?;
        assert_eq!(collection.keys.capacity(), MAX_SUMMARIES);
        assert_eq!(collection.records.capacity(), MAX_SUMMARIES);
        let before = fixture.snapshot().mutation_count();
        assert!(
            repository
                .preflight_publication(
                    seed()?,
                    ExpectedSummary::Absent,
                    IncidentPublicationDemand::new(0, 0, 1, 131_072)?,
                    IncidentUtc::from_unix_seconds(111)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(repository.preflight.is_none());
        drop(collection);
        let token = repository.preflight_publication(
            seed()?,
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(0, 0, 1, 131_072)?,
            IncidentUtc::from_unix_seconds(111)?,
            &context,
        )?;
        drop(token);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn failed_close_retains_repository_and_claim_until_real_boundary_retry() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        fixture.put(DiagnosticObject::Summary(seed()?.key()), b"{}")?;
        let error =
            repository.load(seed()?.key(), &context).err().ok_or("Invalid Summary accepted")?;
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(repository.retire(&context).is_err());
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(context.memory().claim(1).is_err());
        assert!(fixture.snapshot().live_outputs() > 0);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

fn reserve_failure() -> Result<std::collections::TryReserveError, TestError> {
    Vec::<u64>::new()
        .try_reserve_exact(usize::MAX)
        .err()
        .ok_or_else(|| "Capacity overflow unexpectedly reserved".into())
}
fn assert_reserve_cause(error: &IncidentStoreError) -> TestResult {
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<std::collections::TryReserveError>())
        .ok_or("Original allocation cause absent")?;
    assert_eq!(source.to_string(), reserve_failure()?.to_string());
    assert_eq!(error.code(), IncidentStoreFailureCode::Capacity);
    assert_eq!(error.to_string(), IncidentStoreError::Capacity.to_string());
    assert_eq!(format!("{error:?}"), format!("{:?}", IncidentStoreError::Capacity));
    Ok(())
}
#[test]
fn real_collection_reserve_failure_preserves_cause_and_charge_after_owner_drop() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let custody = repository.storage.recovery_collection(&repository.operation, &context)?;
        let mut collection = super::super::allocation::SummaryCollection::new(custody)?;
        assert_eq!(collection.keys.capacity(), MAX_SUMMARIES);
        assert_eq!(collection.records.capacity(), MAX_SUMMARIES);
        let source = collection
            .records
            .try_reserve_exact(usize::MAX)
            .err()
            .ok_or("Capacity overflow unexpectedly reserved")?;
        let error = collection.allocation_failed(source);
        assert_reserve_cause(&error)?;
        assert!(repository.storage.recovery_collection(&repository.operation, &context).is_err());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        assert_reserve_cause(&error)?;
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn real_pending_reserve_failure_preserves_cause_and_charge_after_owner_drop() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let custody = repository.storage.recovery_pending(&repository.operation, &context)?;
        let error = IncidentStoreError::Allocation(
            super::super::allocation::LoanedAllocationFailure::pending(reserve_failure()?, custody),
        );
        assert_reserve_cause(&error)?;
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        assert_reserve_cause(&error)?;
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
