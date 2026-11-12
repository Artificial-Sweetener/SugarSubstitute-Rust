// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual durable journal custody and exact completion precede reclamation.
use super::*;
use sha2::{Digest, Sha256};
fn plan(
    filesystem: &Filesystem,
    operation: LogicalStoreOperationKey,
) -> Result<DiagnosticDispositionPlan, TestError> {
    let key = incident()?;
    let revision = IncidentRevision::new(1)?;
    let victim = DiagnosticObject::Payload(key, revision, AttachmentKind::Log);
    let facts = filesystem.put(victim, b"payload");
    Ok(DiagnosticDispositionPlan::new(
        victim,
        key,
        revision,
        facts,
        operation,
        DispositionIntent::PayloadDiscard,
        Sha256::digest(b"validated journal").into(),
        DiagnosticObject::Summary(key),
        Sha256::digest(b"completion").into(),
    )?)
}
fn permit(
    storage: &mut DiagnosticStorage,
    plan: DiagnosticDispositionPlan,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<DurableDeletionPermit, TestError> {
    let mut stage = storage.stage_disposition(plan, operation, context)?;
    stage.write_chunk(b"validated journal", context)?;
    stage.flush(context)?;
    let permit = storage.activate_disposition(
        &mut stage,
        DiagnosticPredecessor::Absent,
        operation,
        context,
    )?;
    storage.inventory(operation, context)?;
    Ok(permit)
}
#[test]
fn diagnostic_wrong_journal_bytes_never_mint_a_durable_deletion_permit() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = plan(&filesystem, operation.key())?;
        let mut stage = storage.stage_disposition(plan, &operation, &context)?;
        stage.write_chunk(b"different journal", &context)?;
        assert_eq!(
            stage.flush(&context).err().ok_or("wrong digest admitted")?.code(),
            Code::ChangedIdentity
        );
        assert_eq!(
            storage
                .activate_disposition(
                    &mut stage,
                    DiagnosticPredecessor::Absent,
                    &operation,
                    &context
                )
                .err()
                .ok_or("wrong journal minted permit")?
                .code(),
            Code::ChangedIdentity
        );
        assert!(!storage.snapshot().disposition_retained());
        assert_eq!(storage.snapshot().stages(), 1);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_failed_final_close_never_settles_victim_or_retires_journal() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = plan(&filesystem, operation.key())?;
        let mut permit = permit(&mut storage, plan, &operation, &context)?;
        let mut lease = storage.prepare_delete(&mut permit, &operation, &context)?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert_eq!(
            storage
                .delete(&mut lease, &mut permit, &operation, &context)
                .err()
                .ok_or("failed close reported deletion")?
                .code(),
            Code::DeleteUncertain
        );
        assert!(
            !storage
                .shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .deletion
                .as_ref()
                .ok_or("missing intent")?
                .victim_settled
        );
        assert_eq!(storage.snapshot().live_handles(), 2);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        let deleted = storage.delete(&mut lease, &mut permit, &operation, &context)?;
        storage.inventory(&operation, &context)?;
        assert_eq!(deleted.object(), plan.victim);
        assert!(plan.facts.matches(deleted.facts()));
        assert_eq!(storage.snapshot().live_handles(), 1);
        assert!(storage.snapshot().disposition_retained());
        let mut stage = storage.stage(plan.completion, 32, &operation, &context)?;
        stage.write_chunk(b"completion", &context)?;
        stage.flush(&context)?;
        let completion =
            storage.activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)?;
        storage.inventory(&operation, &context)?;
        assert_eq!(
            storage.retire_disposition(&mut permit, &completion, &operation, &context)?.object(),
            DiagnosticObject::Journal(plan.operation)
        );
        assert!(!storage.snapshot().disposition_retained());
        assert_eq!(storage.snapshot().live_handles(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_absence_failure_keeps_intent_until_same_exact_delete_settles() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = plan(&filesystem, operation.key())?;
        let mut permit = permit(&mut storage, plan, &operation, &context)?;
        let mut lease = storage.prepare_delete(&mut permit, &operation, &context)?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.absence =
            true;
        assert_eq!(
            storage
                .delete(&mut lease, &mut permit, &operation, &context)
                .err()
                .ok_or("absence failure settled")?
                .code(),
            Code::DeleteUncertain
        );
        assert!(
            !storage
                .shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .deletion
                .as_ref()
                .ok_or("missing intent")?
                .victim_settled
        );
        assert_eq!(storage.snapshot().live_handles(), 1);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.absence =
            false;
        storage.delete(&mut lease, &mut permit, &operation, &context)?;
        assert!(
            storage
                .shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .deletion
                .as_ref()
                .ok_or("missing intent")?
                .victim_settled
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
