// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise genuine admitted storage and external physical capture effects.
use super::*;
#[path = "control_tests.rs"]
mod control;
use crate::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture, ProtectedCaptureSourceFixture,
};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobOutcome},
    incident::{AttachmentKind, IncidentKey, IncidentRevision, IncidentSubject},
};
type TestError = Box<dyn std::error::Error + Send + Sync>;
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Debug)]
struct SourceFailure;
impl std::fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("external source failure")
    }
}
impl std::error::Error for SourceFailure {}
struct Source<'a> {
    generation: &'a ProtectedCaptureReadLease,
    shared: Shared,
    calls: usize,
    fail: bool,
}
// SAFETY: this deterministic external ABI boundary reports only the selected physical
// outcome; genuine storage/generation/funding/fencing remains the production owner.
#[allow(unsafe_code)]
unsafe impl DiagnosticCaptureNativeSource for Source<'_> {
    type Error = SourceFailure;
    fn settle(&mut self, fence: &mut DiagnosticCaptureFence<'_>) -> Result<(), Self::Error> {
        self.calls += 1;
        assert!(self.shared.inner.try_lock().is_ok(), "physical source invoked under storage lock");
        assert!(fence.matches_generation(self.generation));
        assert!(fence.check_current().is_ok());
        if self.fail { Err(SourceFailure) } else { Ok(()) }
    }
}
fn actual(
    work: impl FnOnce(
        &JobContext,
        &mut DiagnosticStorage,
        &DiagnosticStorageOperation,
        &DiagnosticStorageFixture,
        &mut ProtectedCaptureReadLease,
    ) -> Result<(), TestError>
    + Send
    + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let key = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
            epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
        },
        sugarsubstitute_contracts::incident::IncidentOperationId::new(1)?,
    );
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = ProtectedCaptureSourceFixture::required_reservation_bytes();
    let mut source_job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        ProtectedCaptureSourceFixture::create(
            &context,
            key.owner(),
            std::env::temp_dir().join(format!(
                "ss-capture-intent-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
        )
    })?;
    let done = rig.consume_job(&mut source_job)?;
    let mut source = match done.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("original source stopped".into()),
    };
    drop(done.memory);
    drop(source_job);
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(key, &context)?;
        storage.resolve(Path::new("external-disk"), key.owner().namespace, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        drop(storage.inventory(&operation, &context)?);
        let mut output = None;
        storage.prepare_capture_read(source.root_mut(), &mut output, &operation, &context)?;
        let generation = output.as_mut().ok_or("generation absent")?;
        let (cpp, rust) = storage.reserve_finished_owner(generation, &operation, &context)?;
        work(&context, &mut storage, &operation, &fixture, generation)?;
        assert!(!storage.snapshot().disposition_retained());
        drop(cpp);
        generation.retire(&context)?;
        drop(output);
        storage.retire(&operation, &context)?;
        source.retire(&context)?;
        drop(operation);
        drop(storage);
        drop(source);
        assert!(context.memory().claim(1).is_err());
        drop(rust);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok::<_, TestError>(())
    })?;
    let done = rig.consume_job(&mut job)?;
    match done.outcome {
        JobOutcome::Success(()) => (),
        JobOutcome::Failure(cause) => return Err(cause),
        other => return Err(format!("capture intent stopped: {other:?}").into()),
    }
    drop(done.memory);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
fn reads(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    fixture: &DiagnosticStorageFixture,
    context: &JobContext,
) -> Result<(DiagnosticReadLease, DiagnosticReadLease, DiagnosticReadLease), TestError> {
    let key = IncidentKey::new(IncidentSubject::Supervisor(operation.key().owner()));
    let journal = DiagnosticObject::Journal(operation.key());
    let summary = DiagnosticObject::Summary(key);
    let payload =
        DiagnosticObject::Payload(key, IncidentRevision::new(1)?, AttachmentKind::Minidump);
    fixture.put(journal, b"journal")?;
    fixture.put(summary, b"summary")?;
    fixture.put(payload, b"dump")?;
    Ok((
        storage.read(journal, 65536, operation, context)?,
        storage.read(summary, 65536, operation, context)?,
        storage.read(payload, 65536, operation, context)?,
    ))
}
#[test]
fn capture_intent_fences_actual_three_pins_and_settles_before_checked_journal_absence()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let (mut journal, mut summary, mut payload) = reads(storage, operation, fixture, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        assert_eq!(storage.snapshot().reads(), 4);
        assert!(storage.snapshot().disposition_retained());
        for read in [&mut journal, &mut summary, &mut payload] {
            assert_eq!(read.retire(context).err().ok_or("pinned read retired")?.code(), Code::Busy);
        }
        assert!(
            storage
                .stage(DiagnosticObject::Journal(operation.key()), 16, operation, context)
                .is_err()
        );
        assert!(
            storage
                .read(DiagnosticObject::Journal(operation.key()), 65536, operation, context)
                .is_err()
        );
        let mut source =
            Source { generation, shared: storage.shared.clone(), calls: 0, fail: false };
        let result = storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(result.object(), DiagnosticObject::Journal(operation.key()));
        assert_eq!(source.calls, 1);
        assert!(fixture.snapshot().active_identity(result.object()).is_none());
        assert!(!storage.snapshot().disposition_retained());
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
#[test]
fn capture_intent_close_and_absence_failures_keep_pins_without_repeating_native_settlement()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let (mut journal, mut summary, mut payload) = reads(storage, operation, fixture, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        let mut source =
            Source { generation, shared: storage.shared.clone(), calls: 0, fail: false };
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(storage.settle_capture(&mut intent, &mut source, operation, context).is_err());
        assert_eq!(source.calls, 1);
        assert!(storage.snapshot().disposition_retained());
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        fixture.set_failure(DiagnosticFixtureFailure::AbsenceObservation, true);
        assert!(storage.settle_capture(&mut intent, &mut source, operation, context).is_err());
        assert_eq!(source.calls, 1);
        assert!(storage.snapshot().disposition_retained());
        fixture.set_failure(DiagnosticFixtureFailure::AbsenceObservation, false);
        storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(source.calls, 1);
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
#[test]
fn capture_intent_retains_original_native_failure_and_allows_only_same_owner_reconciliation()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let (mut journal, mut summary, mut payload) = reads(storage, operation, fixture, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        let mut source =
            Source { generation, shared: storage.shared.clone(), calls: 0, fail: true };
        let before = fixture.snapshot().mutation_count();
        let cause = storage
            .settle_capture(&mut intent, &mut source, operation, context)
            .err()
            .ok_or("native failure lost")?;
        assert!(matches!(cause.source_error(), Some(SourceFailure)));
        assert!(cause.post_storage_error().is_none());
        assert!(!cause.post_panicked());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(storage.snapshot().disposition_retained());
        assert_eq!(
            summary.retire(context).err().ok_or("uncertain source lost pin")?.code(),
            Code::Busy
        );
        source.fail = false;
        storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(source.calls, 2);
        drop(cause);
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
#[test]
fn capture_intent_error_destination_capacity_failure_has_no_native_or_journal_effects()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let (mut journal, mut summary, mut payload) = reads(storage, operation, fixture, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        let mut source =
            Source { generation, shared: storage.shared.clone(), calls: 0, fail: false };
        let mut retained = Vec::new();
        while let Ok(slot) =
            super::types::CaptureErrorSlot::<SourceFailure>::new(&storage.shared, operation.key())
        {
            retained.try_reserve(1)?;
            retained.push(slot);
        }
        let before = fixture.snapshot().mutation_count();
        let cause = storage
            .settle_capture(&mut intent, &mut source, operation, context)
            .err()
            .ok_or("unfunded native effect admitted")?;
        assert_eq!(cause.post_storage_error().ok_or("capacity cause lost")?.code(), Code::Capacity);
        assert_eq!(source.calls, 0);
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(storage.snapshot().disposition_retained());
        drop(cause);
        drop(retained);
        storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(source.calls, 1);
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
#[test]
fn capture_intent_historical_epoch_keeps_actual_current_operation_and_same_namespace_read_custody()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let historical = sugarsubstitute_contracts::incident::LogicalStoreOperationKey::new(
            CoordinatorKey {
                namespace: operation.key().owner().namespace,
                epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([1; 16])?,
            },
            sugarsubstitute_contracts::incident::IncidentOperationId::new(1)?,
        );
        let key = IncidentKey::new(IncidentSubject::Supervisor(historical.owner()));
        let j = DiagnosticObject::Journal(historical);
        let s = DiagnosticObject::Summary(key);
        let p = DiagnosticObject::Payload(key, IncidentRevision::new(1)?, AttachmentKind::Minidump);
        fixture.put(j, b"original journal")?;
        fixture.put(s, b"original summary")?;
        fixture.put(p, b"dump")?;
        let mut journal = storage.read(j, 65536, operation, context)?;
        let mut summary = storage.read(s, 65536, operation, context)?;
        let mut payload = storage.read(p, 65536, operation, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        let mut source =
            Source { generation, shared: storage.shared.clone(), calls: 0, fail: false };
        let deletion = storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(deletion.object(), j);
        assert_ne!(historical.owner(), operation.key().owner());
        assert_eq!(source.calls, 1);
        assert!(fixture.snapshot().active_identity(j).is_none());
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
