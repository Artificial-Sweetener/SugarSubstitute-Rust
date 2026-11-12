// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real execution/source loans and fake external disk, without native namespace or process proof fabrication.
use super::*;
use crate::completion_wake::Wake;
use crate::production::diagnostic_helper::tests::{TestClock, TestError, key, observe, seed, spec};
use std::sync::Arc;
use sugarsubstitute_contracts::lifecycle::*;
use sugarsubstitute_crash::incident::{StoredIncidentSummary, encode_summary};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::fixture::*;
pub(super) fn summary() -> Result<IncidentSummary, TestError> {
    Ok(IncidentSummary::new(
        seed()?,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 259 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Unavailable,
            ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
            SymbolOutcome::NotRequested,
        ),
    )?)
}
/// External fixture opening invokes the actual storage algorithms; no production token is minted.
pub(super) fn prepared(
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<RecoveryOperation, TestError> {
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key()?, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    let mut repository = IncidentRepository::new(storage, operation, context)?;
    repository.acquire_writer(context)?;
    if let DiagnosticStoreOpening::Created(mut genesis) = opening {
        repository.initialize(&mut genesis, context)?;
    }
    let mut recovery = RecoveryOperation::new();
    recovery.original = Some(context.memory().clone());
    recovery.repository = Some(repository);
    Ok(recovery)
}
#[test]
fn actual_pending_snapshot_and_original_credit_survive_terminal_until_checked_disposal()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        spec(JobId::new(1)?),
        |context| -> Result<RecoveryOperation, TestError> {
            let fixture = DiagnosticStorageFixture::new()?;
            let original = summary()?;
            let mut recovery = prepared(&context, &fixture)?;
            fixture.put(
                DiagnosticObject::Summary(original.seed().key()),
                &encode_summary(&StoredIncidentSummary::new(original.clone(), None)?)?,
            )?;
            recovery.pending = Some(
                recovery
                    .repository
                    .as_mut()
                    .ok_or("Repository absent")?
                    .pending_incidents(IncidentUtc::from_unix_seconds(110)?, &context)?,
            );
            assert!(recovery.pending.as_ref().ok_or("Snapshot absent")?.summaries().is_err());
            assert!(recovery.retire(&context));
            let mut summaries = recovery.pending.as_ref().ok_or("Snapshot absent")?.summaries()?;
            assert_eq!(summaries.next().ok_or("Pending absent")?.summary(), &original);
            assert!(summaries.next().is_none());
            drop(summaries);
            Ok(recovery)
        },
    )?;
    let done = observe(&client, &wake, &mut job)?;
    let recovery = match done.outcome {
        JobOutcome::Success(recovery) => recovery,
        JobOutcome::Failure(cause) => return Err(cause),
        JobOutcome::Panicked(cause) => {
            return Err(format!("Actual recovery worker panicked: {}", cause.as_str()).into());
        }
        other => return Err(format!("Actual recovery worker stopped: {:?}", other.kind()).into()),
    };
    drop(done.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    assert!(recovery.retired && !recovery.failed());
    drop(recovery);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_corrupt_pending_read_preserves_first_error_and_checked_close_failure()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        spec(JobId::new(1)?),
        |context| -> Result<(), TestError> {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut recovery = prepared(&context, &fixture)?;
            let object = DiagnosticObject::Summary(seed()?.key());
            let before = fixture.put(object, b"original corrupt bytes")?;
            let failure = recovery
                .repository
                .as_mut()
                .ok_or("Repository absent")?
                .pending_incidents(IncidentUtc::from_unix_seconds(110)?, &context)
                .err()
                .ok_or("Corrupt source admitted")?;
            recovery.first = Some(failure);
            fixture.set_failure(DiagnosticFixtureFailure::Close, true);
            assert!(!recovery.retire(&context));
            assert!(recovery.first.is_some() && recovery.retirement.is_some());
            assert_eq!(fixture.snapshot().active_identity(object), Some(before));
            fixture.set_failure(DiagnosticFixtureFailure::Close, false);
            assert!(recovery.retire(&context));
            assert!(recovery.failed());
            Ok(())
        },
    )?;
    let done = observe(&client, &wake, &mut job)?;
    match done.outcome {
        JobOutcome::Success(()) => {}
        JobOutcome::Failure(cause) => return Err(cause),
        JobOutcome::Panicked(cause) => {
            return Err(format!("Actual corrupt-read worker panicked: {}", cause.as_str()).into());
        }
        other => {
            return Err(format!("Actual corrupt-read worker stopped: {:?}", other.kind()).into());
        }
    }
    drop(done.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[expect(
    clippy::panic,
    reason = "Exercise original recovery custody across an intentional external provider unwind"
)]
#[test]
fn actual_supervised_unwind_restores_original_source_for_independent_checked_cleanup()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut metadata = spec(JobId::new(1)?);
    metadata.reservation_bytes = RECOVERY_SOURCE_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, metadata, |context| source::Source::new(&context))?;
    let completed = observe(&client, &wake, &mut constructor)?;
    let source = match completed.outcome {
        JobOutcome::Success(source) => source,
        _ => return Err("Actual source constructor failed".into()),
    };
    drop(completed.memory);
    drop(constructor);
    let original = source.alias();
    let mut worker = client.submit_blocking(
        &scope,
        spec(JobId::new(2)?),
        move |context| -> Result<bool, TestError> {
            let mut loan = original.loan()?;
            let operation = loan.operation()?;
            let fixture = DiagnosticStorageFixture::new()?;
            let mut stored = prepared(&context, &fixture)?;
            let summary = summary()?;
            fixture.put(
                DiagnosticObject::Summary(summary.seed().key()),
                &encode_summary(&StoredIncidentSummary::new(summary, None)?)?,
            )?;
            stored.pending = Some(
                stored
                    .repository
                    .as_mut()
                    .ok_or("Repository absent")?
                    .pending_incidents(IncidentUtc::from_unix_seconds(110)?, &context)?,
            );
            *operation = stored;
            panic!("fixed recovery external provider unwind")
        },
    )?;
    let completed = observe(&client, &wake, &mut worker)?;
    let panic = match completed.outcome {
        JobOutcome::Panicked(cause) => cause,
        JobOutcome::Failure(cause) => return Err(cause),
        other => {
            return Err(
                format!("Actual supervised provider unwind absent: {:?}", other.kind()).into()
            );
        }
    };
    assert_eq!(
        source.inspect(|operation| operation.pending.is_some() && !operation.retired),
        Some(true)
    );
    let panicked_worker_memory = completed.memory;
    drop(worker);
    let cleanup_source = source.alias();
    let mut cleanup_spec = spec(JobId::new(3)?);
    cleanup_spec.reservation_bytes = 4096;
    let cleanup_bytes = cleanup_spec.reservation_bytes;
    let mut cleanup = client.submit_blocking(&scope, cleanup_spec, move |context| {
        let mut loan = cleanup_source.loan()?;
        Ok::<_, RecoveryIssue>(loan.operation()?.retire(&context))
    })?;
    let completed = observe(&client, &wake, &mut cleanup)?;
    assert!(matches!(completed.outcome, JobOutcome::Success(true)));
    drop(completed);
    drop(cleanup);
    drop(owner);
    // Storage retains the independently admitted cleanup context through actual Source disposal.
    assert_eq!(
        scope.snapshot().reserved_bytes,
        RECOVERY_SOURCE_BYTES + DIAGNOSTIC_STORAGE_BYTES + cleanup_bytes
    );
    assert_eq!(panic.as_str(), "fixed recovery external provider unwind");
    source.release()?;
    // The original panicked worker completion still owns its entire accepted reservation.
    assert_eq!(scope.snapshot().reserved_bytes, RECOVERY_SOURCE_BYTES + DIAGNOSTIC_STORAGE_BYTES);
    drop(source);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(panic);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(panicked_worker_memory);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
