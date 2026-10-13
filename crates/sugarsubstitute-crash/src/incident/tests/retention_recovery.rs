// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover persisted payload intent under a distinct actual sealed operation.
use super::{
    publication_fixture::{admitted, setup},
    retention_fixture::*,
    *,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
fn successor(
    fixture: &DiagnosticStorageFixture,
    context: &JobContext,
) -> Result<IncidentRepository, TestError> {
    let current = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([9; 16])?,
    };
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(
        LogicalStoreOperationKey::new(current, IncidentOperationId::new(22)?),
        context,
    )?;
    storage.resolve(
        std::path::Path::new("external-repository-fixture"),
        current.namespace,
        &operation,
        context,
    )?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok(IncidentRepository::new(storage, operation, context)?)
}
fn recover(
    repository: &mut IncidentRepository,
    historical: LogicalStoreOperationKey,
    key: IncidentKey,
    context: &JobContext,
) -> Result<DiagnosticPayloadDiscardPermit, TestError> {
    let mut journal = repository.storage.read(
        DiagnosticObject::Journal(historical),
        65_536,
        &repository.operation,
        context,
    )?;
    let mut summary = repository.storage.read(
        DiagnosticObject::Summary(key),
        65_536,
        &repository.operation,
        context,
    )?;
    Ok(repository.storage.recover_payload_discard(
        &mut journal,
        &mut summary,
        &IncidentPayloadDiscardValidator,
        &repository.operation,
        context,
    )?)
}
#[test]
fn payload_original_restart_adopts_exact_prior_stage_without_restaging() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let key = prepared.original.summary().seed().key();
        let historical = repository.operation();
        let candidate = prepared.candidate.clone();
        let candidate_facts = prepared.preparation.candidate_stage();
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        let permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        repository.retire(&context)?;
        drop(permit);
        drop(stage);
        drop(prepared);
        drop(repository);
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        let mut repository = successor(&fixture, &context)?;
        assert_ne!(repository.operation().owner(), historical.owner());
        let before = fixture.snapshot().mutation_count();
        let mut permit = recover(&mut repository, historical, key, &context)?;
        assert_eq!(fixture.snapshot().mutation_count(), before);
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        assert_eq!(completion.summary().facts(), candidate_facts);
        repository.storage.finish_payload_discard(
            &mut permit,
            &completion,
            &repository.operation,
            &context,
        )?;
        assert_eq!(repository.load(key, &context)?.summary(), candidate.summary());
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn payload_candidate_restart_uses_journal_and_actual_absence_without_summary_replay() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let key = prepared.original.summary().seed().key();
        let historical = repository.operation();
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
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        let candidate_facts = completion.summary().facts();
        assert!(fixture.snapshot().active_identity(prepared.payload).is_none());
        repository.retire(&context)?;
        drop(completion);
        drop(permit);
        drop(stage);
        drop(prepared);
        drop(repository);
        let mut repository = successor(&fixture, &context)?;
        let before = fixture.snapshot().mutation_count();
        let mut permit = recover(&mut repository, historical, key, &context)?;
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        assert_eq!(completion.summary().facts(), candidate_facts);
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.storage.finish_payload_discard(
            &mut permit,
            &completion,
            &repository.operation,
            &context,
        )?;
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn payload_original_restart_rejects_missing_victim_and_preserves_journal_summary() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let key = prepared.original.summary().seed().key();
        let historical = repository.operation();
        let payload = prepared.payload;
        let original = prepared.preparation.original_file();
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        let permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        repository.retire(&context)?;
        drop(permit);
        drop(stage);
        drop(prepared);
        drop(repository);
        fixture.erase_existing(payload)?;
        let mut repository = successor(&fixture, &context)?;
        assert!(recover(&mut repository, historical, key, &context).is_err());
        assert_eq!(
            fixture.snapshot().active_identity(DiagnosticObject::Summary(key)),
            Some(original)
        );
        assert!(
            fixture.snapshot().active_identity(DiagnosticObject::Journal(historical)).is_some()
        );
        assert!(repository.snapshot().writer_held());
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn payload_restart_rejects_third_summary_identity_before_any_reclamation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let key = prepared.original.summary().seed().key();
        let historical = repository.operation();
        let payload = prepared.payload;
        let original = prepared.original.clone();
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        let permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        repository.retire(&context)?;
        drop(permit);
        drop(stage);
        drop(prepared);
        drop(repository);
        let replaced = fixture.put(DiagnosticObject::Summary(key), &encode_summary(&original)?)?;
        let before = fixture.snapshot().mutation_count();
        let mut repository = successor(&fixture, &context)?;
        assert!(recover(&mut repository, historical, key, &context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(
            fixture.snapshot().active_identity(DiagnosticObject::Summary(key)),
            Some(replaced)
        );
        assert!(fixture.snapshot().active_identity(payload).is_some());
        assert_eq!(repository.snapshot().reads(), 2);
        assert!(repository.snapshot().writer_held());
        repository.retire(&context)?;
        Ok(())
    })
}
