// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unproven native source before effects using separate actual original jobs.
use super::*;
#[test]
fn actual_capture_read_rejects_uncreated_source_preserving_both_original_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let mut source_spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    source_spec.reservation_bytes = 1024 * 1024;
    let mut source_job = rig.client.submit_blocking(&rig.scope, source_spec, |context| {
        crate::ProtectedStorageRoot::new(
            context
                .memory()
                .claim(1024 * 1024)
                .map_err(|cause| ClaimError::new("source_actual_claim", cause))?,
        )
    })?;
    let source_done = rig.consume_job(&mut source_job)?;
    let mut source = match source_done.outcome {
        JobOutcome::Success(root) => root,
        _ => return Err("actual source construction failed".into()),
    };
    drop(source_done.memory);
    drop(source_job);
    let mut current_spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    current_spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut current_job = rig.client.submit_blocking(&rig.scope, current_spec, move |context| {
        let fixture = super::super::fixture::DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(key(1).map_err(|_| error_key())?, &context)?;
        let mut output = None;
        assert!(
            storage.prepare_capture_read(&mut source, &mut output, &operation, &context).is_err()
        );
        assert!(output.is_none());
        assert_eq!(storage.snapshot().reads(), 0);
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        assert!(context.memory().claim(1).is_err());
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        source
            .retire(&context)
            .map_err(|cause| DiagnosticStorageError::native(None, Code::Retirement, cause))?;
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).map_err(|_| error_key())?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let done = rig.consume_job(&mut current_job)?;
    assert!(
        matches!(done.outcome, JobOutcome::Success(())),
        "actual source result: {:?}",
        done.outcome
    );
    drop(done);
    drop(current_job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
fn error_key() -> DiagnosticStorageError {
    DiagnosticStorageError::rejected(None, Code::Admission)
}
