// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify admission, streaming and retention around actual external failure points.
use super::*;
#[test]
fn diagnostic_partial_stage_and_failed_close_retain_the_original_claim() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .partial_stage = true;
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 64, &operation, &context)
                .err()
                .ok_or("partial stage unexpectedly succeeded")?
                .code(),
            Code::Write
        );
        assert_eq!(storage.snapshot().stages(), 1);
        assert_eq!(storage.snapshot().live_handles(), 1);
        assert!(context.memory().claim(1).is_err());
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert!(storage.retire(&operation, &context).is_err());
        assert!(context.memory().claim(1).is_err());
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        storage.retire(&operation, &context)?;
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn diagnostic_eight_retained_stages_reject_overload_before_any_new_effect() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        for _ in 0..DIAGNOSTIC_STAGE_BOUND {
            drop(storage.stage(
                DiagnosticObject::Summary(incident()?),
                64,
                &operation,
                &context,
            )?);
        }
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert_eq!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 64, &operation, &context)
                .err()
                .ok_or("stage overload admitted")?
                .code(),
            Code::Capacity
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        assert_eq!(
            storage
                .begin_operation(key(2)?, &context)
                .err()
                .ok_or("live operation bypassed")?
                .code(),
            Code::Busy
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_partial_open_remains_owned_until_actual_checked_retirement() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        filesystem.put(object, b"summary");
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .partial_open = true;
        assert_eq!(
            storage
                .read(object, 64, &operation, &context)
                .err()
                .ok_or("read unexpectedly succeeded")?
                .code(),
            Code::Read
        );
        assert_eq!(storage.snapshot().reads(), 1);
        assert_eq!(storage.snapshot().live_handles(), 1);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert_eq!(
            storage
                .retire(&operation, &context)
                .err()
                .ok_or("retirement unexpectedly succeeded")?
                .code(),
            Code::Retirement
        );
        assert_eq!(storage.snapshot().reads(), 1);
        assert_eq!(storage.snapshot().live_handles(), 1);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        storage.retire(&operation, &context)?;
        assert_eq!(storage.snapshot().live_handles(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_dropped_read_does_not_reuse_capacity_or_release_custody() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        filesystem.put(object, b"summary");
        let (mut storage, operation) = setup(&context, filesystem)?;
        for _ in 0..DIAGNOSTIC_READ_BOUND {
            drop(storage.read(object, 64, &operation, &context)?);
        }
        assert_eq!(
            storage.read(object, 64, &operation, &context).err().ok_or("capacity admitted")?.code(),
            Code::Capacity
        );
        assert_eq!(storage.snapshot().live_handles(), 4);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_actual_stream_bounds_and_flush_precede_activation() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let object = DiagnosticObject::Summary(incident()?);
        let mut stage = storage.stage(object, 8, &operation, &context)?;
        assert_eq!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .err()
                .ok_or("unflushed activation")?
                .code(),
            Code::Flush
        );
        stage.write_chunk(b"12345678", &context)?;
        assert_eq!(
            stage.write_chunk(b"9", &context).err().ok_or("oversize write")?.code(),
            Code::Capacity
        );
        assert_eq!(stage.flush(&context)?.read_facts().bytes(), 8);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.activate =
            true;
        assert_eq!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .err()
                .ok_or("uncertain activation")?
                .code(),
            Code::ActivationUncertain
        );
        drop(stage);
        assert_eq!(storage.snapshot().stages(), 1);
        assert_eq!(storage.snapshot().live_handles(), 1);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_full_logical_operation_replay_is_rejected_without_new_effects() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        storage.finish_operation(&operation, &context)?;
        assert_eq!(
            storage.begin_operation(key(1)?, &context).err().ok_or("replay admitted")?.code(),
            Code::InvalidIdentity
        );
        let foreign = LogicalStoreOperationKey::new(owner_key(3)?, IncidentOperationId::new(2)?);
        assert_eq!(
            storage
                .begin_operation(foreign, &context)
                .err()
                .ok_or("foreign owner admitted")?
                .code(),
            Code::InvalidIdentity
        );
        let next = storage.begin_operation(key(2)?, &context)?;
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&next, &context)?;
        Ok(())
    })
}
