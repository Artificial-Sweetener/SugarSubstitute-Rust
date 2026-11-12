// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise strict semantic recovery through actual retained journal and ledger reads.
use super::publication_fixture::{admitted, setup};
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
fn file(actual: DiagnosticReadFacts) -> Result<IncidentFileFacts, IncidentStoreError> {
    IncidentFileFacts::new(actual.file_identity(), actual.sha256(), actual.bytes())
}
fn data(
    context: &JobContext,
) -> Result<
    (
        DiagnosticStorageFixture,
        IncidentRepository,
        LogicalStoreOperationKey,
        DiagnosticReadFacts,
        DiagnosticReadFacts,
    ),
    TestError,
> {
    let (fixture, repository) = setup(context)?;
    let namespace = repository.operation().owner().namespace;
    let original = SummaryEvictions::new(namespace, 0, 0, 0, None)?;
    let incident = IncidentKey::new(IncidentSubject::Supervisor(repository.operation().owner()));
    let candidate = original.record(SummaryEviction::new(
        incident,
        IncidentUtc::from_unix_seconds(101)?,
        SummaryEvictionKind::Pending,
    ))?;
    let original_bytes = encode_store_state(&IncidentStoreState::Ledger(original))?;
    let candidate_bytes = encode_store_state(&IncidentStoreState::Ledger(candidate))?;
    let original_facts = fixture.put(DiagnosticObject::StoreState, &original_bytes)?;
    let victim = fixture.put(DiagnosticObject::Summary(incident), b"exact preserved victim")?;
    let historical = LogicalStoreOperationKey::new(
        CoordinatorKey { namespace, epoch: CoordinatorEpoch::new([9; 16])? },
        IncidentOperationId::new(1)?,
    );
    let candidate_facts =
        fixture.put_stage(DiagnosticObject::StoreState, historical, &candidate_bytes)?;
    let journal = SummaryEvictionJournal::new(
        historical,
        IncidentRevision::new(2)?,
        file(victim)?,
        file(original_facts)?,
        file(candidate_facts)?,
        original,
        candidate,
    )?;
    fixture.put(
        DiagnosticObject::Journal(historical),
        &encode_store_state(&IncidentStoreState::SummaryEviction(Box::new(journal)))?,
    )?;
    Ok((fixture, repository, historical, original_facts, candidate_facts))
}
fn reads(
    repository: &mut IncidentRepository,
    historical: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(DiagnosticReadLease, DiagnosticReadLease), TestError> {
    repository.storage.inventory(&repository.operation, context)?;
    Ok((
        repository.storage.read(
            DiagnosticObject::Journal(historical),
            65536,
            &repository.operation,
            context,
        )?,
        repository.storage.read(
            DiagnosticObject::StoreState,
            65536,
            &repository.operation,
            context,
        )?,
    ))
}
#[test]
fn repository_recovery_original_present_and_absent_use_exact_retained_candidate() -> TestResult {
    for present in [true, false] {
        admitted(move |context| {
            let (fixture, mut repository, historical, original, candidate) = data(&context)?;
            let incident =
                IncidentKey::new(IncidentSubject::Supervisor(repository.operation().owner()));
            if !present {
                fixture.erase_existing(DiagnosticObject::Summary(incident))?;
            }
            let (mut journal, mut ledger) = reads(&mut repository, historical, &context)?;
            let mut permit = repository.storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &IncidentRecoveryValidator,
                &repository.operation,
                &context,
            )?;
            assert_eq!(repository.snapshot().reads(), 0);
            if present {
                let mut victim = repository.storage.prepare_delete(
                    &mut permit,
                    &repository.operation,
                    &context,
                )?;
                repository.storage.delete(
                    &mut victim,
                    &mut permit,
                    &repository.operation,
                    &context,
                )?;
            }
            repository.storage.inventory(&repository.operation, &context)?;
            let mut stage = repository.storage.recover_completion_stage(
                &mut permit,
                &repository.operation,
                &context,
            )?;
            assert_eq!(stage.flush(&context)?.read_facts(), candidate);
            let completion = repository.storage.activate(
                &mut stage,
                super::super::repository::predecessor(original),
                &repository.operation,
                &context,
            )?;
            assert_eq!(completion.facts(), candidate);
            repository.storage.inventory(&repository.operation, &context)?;
            repository.storage.retire_disposition(
                &mut permit,
                &completion,
                &repository.operation,
                &context,
            )?;
            assert_eq!(
                fixture.snapshot().active_identity(DiagnosticObject::StoreState),
                Some(candidate)
            );
            repository.retire(&context)?;
            assert_eq!(fixture.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn repository_recovery_changed_ledger_preserves_journal_and_actual_read_custody() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository, historical, _, _) = data(&context)?;
        let namespace = repository.operation().owner().namespace;
        let incident =
            IncidentKey::new(IncidentSubject::Supervisor(repository.operation().owner()));
        let third = SummaryEvictions::new(
            namespace,
            5,
            0,
            0,
            Some(SummaryEviction::new(
                incident,
                IncidentUtc::from_unix_seconds(101)?,
                SummaryEvictionKind::Pending,
            )),
        )?;
        fixture.put(
            DiagnosticObject::StoreState,
            &encode_store_state(&IncidentStoreState::Ledger(third))?,
        )?;
        let (mut journal, mut ledger) = reads(&mut repository, historical, &context)?;
        let before = fixture.snapshot().mutation_count();
        assert!(
            repository
                .storage
                .recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &IncidentRecoveryValidator,
                    &repository.operation,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(repository.snapshot().reads(), 2);
        assert!(fixture.erase_existing(DiagnosticObject::Journal(historical)).is_err());
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn repository_recovery_changed_journal_name_and_schema_never_mint_authority() -> TestResult {
    for malformed in [false, true] {
        admitted(move |context| {
            let (fixture, mut repository, historical, _, _) = data(&context)?;
            let wrong = LogicalStoreOperationKey::new(historical.owner(), historical.id().next()?);
            let mut original = repository.storage.read(
                DiagnosticObject::Journal(historical),
                65536,
                &repository.operation,
                &context,
            )?;
            let mut bytes = vec![0; original.facts().bytes() as usize];
            original.read_chunk(0, &mut bytes, &context)?;
            original.retire(&context)?;
            if malformed {
                bytes = br#"{"schema_version":99,"content":{}}"#.to_vec();
            }
            fixture.put(DiagnosticObject::Journal(wrong), &bytes)?;
            repository.storage.inventory(&repository.operation, &context)?;
            let mut journal = repository.storage.read(
                DiagnosticObject::Journal(wrong),
                65536,
                &repository.operation,
                &context,
            )?;
            let mut ledger = repository.storage.read(
                DiagnosticObject::StoreState,
                65536,
                &repository.operation,
                &context,
            )?;
            let before = fixture.snapshot().mutation_count();
            let cause = repository
                .storage
                .recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &IncidentRecoveryValidator,
                    &repository.operation,
                    &context,
                )
                .err()
                .ok_or("foreign journal accepted")?;
            assert!(cause.source().is_some());
            assert_eq!(fixture.snapshot().mutation_count(), before);
            assert_eq!(repository.snapshot().reads(), 2);
            repository.retire(&context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn repository_recovery_candidate_absent_is_reconciled_once_and_present_blocks() -> TestResult {
    for present in [false, true] {
        admitted(move |context| {
            let (fixture, mut repository, historical, original, candidate) = data(&context)?;
            let incident =
                IncidentKey::new(IncidentSubject::Supervisor(repository.operation().owner()));
            fixture.erase_existing(DiagnosticObject::Summary(incident))?;
            let (mut journal, mut ledger) = reads(&mut repository, historical, &context)?;
            let mut permit = repository.storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &IncidentRecoveryValidator,
                &repository.operation,
                &context,
            )?;
            let mut stage = repository.storage.recover_completion_stage(
                &mut permit,
                &repository.operation,
                &context,
            )?;
            let active = repository.storage.activate(
                &mut stage,
                super::super::repository::predecessor(original),
                &repository.operation,
                &context,
            )?;
            assert_eq!(active.facts(), candidate);
            repository.retire(&context)?;
            drop((active, stage, permit, journal, ledger, repository));
            if present {
                fixture.put(DiagnosticObject::Summary(incident), b"returned victim")?;
            }
            let mut storage = fixture.storage(&context)?;
            let operation = storage.begin_operation(
                LogicalStoreOperationKey::new(historical.owner(), IncidentOperationId::new(2)?),
                &context,
            )?;
            storage.resolve(
                std::path::Path::new("external-repository-fixture"),
                historical.owner().namespace,
                &operation,
                &context,
            )?;
            storage.acquire_writer(&operation, &context)?;
            storage.inventory(&operation, &context)?;
            let mut journal =
                storage.read(DiagnosticObject::Journal(historical), 65536, &operation, &context)?;
            let mut ledger =
                storage.read(DiagnosticObject::StoreState, 65536, &operation, &context)?;
            let before = fixture.snapshot().mutation_count();
            let result = storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &IncidentRecoveryValidator,
                &operation,
                &context,
            );
            if present {
                assert_eq!(
                    result.err().ok_or("Candidate with present victim accepted")?.code(),
                    DiagnosticStorageFailureCode::ChangedIdentity
                );
                assert_eq!(fixture.snapshot().mutation_count(), before);
                assert_eq!(fixture.snapshot().live_outputs(), 2);
            } else {
                let mut permit = result?;
                assert!(
                    storage.recover_completion_stage(&mut permit, &operation, &context).is_err()
                );
                let active = storage.reconcile_activation(
                    DiagnosticObject::StoreState,
                    super::super::repository::predecessor(candidate),
                    &operation,
                    &context,
                )?;
                storage.retire_disposition(&mut permit, &active, &operation, &context)?;
                assert_eq!(fixture.snapshot().mutation_count(), before + 1);
                assert_eq!(
                    fixture.snapshot().active_identity(DiagnosticObject::StoreState),
                    Some(candidate)
                );
            }
            storage.retire(&operation, &context)?;
            assert_eq!(fixture.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn repository_recovery_failed_ledger_close_keeps_intent_and_original_claim() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository, historical, _, _) = data(&context)?;
        let incident =
            IncidentKey::new(IncidentSubject::Supervisor(repository.operation().owner()));
        fixture.erase_existing(DiagnosticObject::Summary(incident))?;
        let (mut journal, mut ledger) = reads(&mut repository, historical, &context)?;
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        let before = fixture.snapshot().mutation_count();
        assert_eq!(
            repository
                .storage
                .recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &IncidentRecoveryValidator,
                    &repository.operation,
                    &context
                )
                .err()
                .ok_or("failed ledger close accepted")?
                .code(),
            DiagnosticStorageFailureCode::Retirement
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(fixture.snapshot().live_outputs(), 2);
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(context.memory().claim(1).is_err());
        assert!(repository.retire(&context).is_err());
        assert_eq!(fixture.snapshot().live_outputs(), 2);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
