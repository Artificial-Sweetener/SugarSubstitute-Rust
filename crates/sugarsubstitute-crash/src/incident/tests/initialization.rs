// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise canonical genesis publication through actual storage and finite Core jobs.
use super::{publication_fixture::admitted, *};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

fn opening(
    fixture: &DiagnosticStorageFixture,
    context: &JobContext,
) -> Result<(IncidentRepository, DiagnosticStoreOpening), TestError> {
    let key = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    );
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok((IncidentRepository::new(storage, operation, context)?, opening))
}
fn created(opening: DiagnosticStoreOpening) -> Result<DiagnosticStoreGenesis, TestError> {
    match opening {
        DiagnosticStoreOpening::Created(genesis) => Ok(genesis),
        DiagnosticStoreOpening::Existing => Err("fresh external namespace was existing".into()),
    }
}
fn empty() -> Result<IncidentStoreState, TestError> {
    Ok(IncidentStoreState::Ledger(SummaryEvictions::new(NamespaceId::new([1; 32]), 0, 0, 0, None)?))
}
#[test]
fn repository_initialization_publishes_canonical_ledger_before_actual_seed() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        assert!(repository.validate_store(&context).is_err());
        let actual = repository.initialize(&mut genesis, &context)?;
        let canonical = encode_store_state(&empty()?)?;
        assert_eq!(actual.object(), DiagnosticObject::StoreState);
        assert_eq!(actual.facts().sha256(), codec_ledger::digest(&canonical));
        assert_eq!(actual.facts().bytes(), canonical.len() as u64);
        assert_eq!(
            repository.validate_store(&context)?,
            match empty()? {
                IncidentStoreState::Ledger(value) => value,
                _ => return Err("empty constructor changed variant".into()),
            }
        );
        let commit =
            repository.publish_seed(seed().map_err(|error| error.to_string())?, &context)?;
        assert!(fixture.snapshot().active_identity(DiagnosticObject::Seed(commit.key())).is_some());
        drop(commit);
        drop(actual);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn existing_missing_ledger_blocks_seed_without_new_mutation() -> TestResult {
    super::publication_fixture::admitted_handoff(
        |context| {
            let fixture = DiagnosticStorageFixture::new()?;
            let (mut repository, observed) = opening(&fixture, &context)?;
            let genesis = created(observed)?;
            repository.retire(&context)?;
            drop(genesis);
            drop(repository);
            Ok(fixture)
        },
        |context, fixture| {
            let (mut repository, observed) = opening(&fixture, &context)?;
            assert!(matches!(observed, DiagnosticStoreOpening::Existing));
            let before = fixture.snapshot().mutation_count();
            assert!(repository.validate_store(&context).is_err());
            assert!(
                repository
                    .publish_seed(seed().map_err(|error| error.to_string())?, &context)
                    .is_err()
            );
            assert_eq!(fixture.snapshot().mutation_count(), before);
            assert!(fixture.snapshot().active_identity(DiagnosticObject::StoreState).is_none());
            repository.retire(&context)?;
            Ok(())
        },
    )
}
#[test]
fn genesis_uncertain_activation_keeps_exact_inode_until_checked_reconciliation() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        let facts = fixture
            .snapshot()
            .active_identity(DiagnosticObject::StoreState)
            .ok_or("rename did not occur")?;
        let mutations = fixture.snapshot().mutation_count();
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(
            repository.publish_seed(seed().map_err(|error| error.to_string())?, &context).is_err()
        );
        assert!(repository.initialize(&mut genesis, &context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), mutations);
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(repository.reconcile_initialization(&context).is_err());
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(context.memory().claim(1).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        let actual = repository.reconcile_initialization(&context)?;
        assert_eq!(actual.facts(), facts);
        assert_eq!(fixture.snapshot().mutation_count(), mutations);
        assert_eq!(repository.snapshot().stages(), 0);
        drop(actual);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn genesis_failed_readback_keeps_activation_without_publishing_semantic_success() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        fixture.set_failure(DiagnosticFixtureFailure::ReadCorruption, true);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        assert!(repository.initialization.is_some());
        assert_eq!(repository.snapshot().reads(), 1);
        let before = fixture.snapshot().mutation_count();
        assert!(
            repository.publish_seed(seed().map_err(|error| error.to_string())?, &context).is_err()
        );
        fixture.set_failure(DiagnosticFixtureFailure::ReadCorruption, false);
        let actual = repository.reconcile_initialization(&context)?;
        assert_eq!(repository.snapshot().reads(), 0);
        assert_eq!(fixture.snapshot().mutation_count(), before);
        drop(actual);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn genesis_partial_creation_cannot_restage_or_release_original_charge() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        let before = fixture.snapshot().mutation_count();
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        assert!(repository.reconcile_initialization(&context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(context.memory().claim(1).is_err());
        repository.retire(&context)?;
        drop(genesis);
        drop(repository);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn retained_external_ordinary_stage_blocks_initializer_before_ledger_effect() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        let staged = repository.storage.stage(
            DiagnosticObject::Seed(seed().map_err(|cause| cause.to_string())?.key()),
            1024,
            &repository.operation,
            &context,
        )?;
        let before = fixture.snapshot().mutation_count();
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        assert!(repository.initialization.is_none());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(fixture.snapshot().active_identity(DiagnosticObject::StoreState).is_none());
        assert!(context.memory().claim(1).is_err());
        repository.retire(&context)?;
        drop(staged);
        Ok(())
    })
}
#[test]
fn retained_external_partial_stage_blocks_initializer_before_ledger_effect() -> TestResult {
    admitted(|context| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut repository, observed) = opening(&fixture, &context)?;
        let mut genesis = created(observed)?;
        fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
        assert!(
            repository
                .storage
                .stage(
                    DiagnosticObject::Seed(seed().map_err(|cause| cause.to_string())?.key()),
                    1024,
                    &repository.operation,
                    &context
                )
                .is_err()
        );
        let before = fixture.snapshot().mutation_count();
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(repository.initialize(&mut genesis, &context).is_err());
        assert!(repository.initialization.is_none());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert!(fixture.snapshot().active_identity(DiagnosticObject::StoreState).is_none());
        assert!(context.memory().claim(1).is_err());
        repository.retire(&context)?;
        Ok(())
    })
}
