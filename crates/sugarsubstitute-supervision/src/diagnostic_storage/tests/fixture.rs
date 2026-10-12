// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume the public CPU fixture through real operation, admission and custody.
use super::*;
use crate::diagnostic_storage::fixture::{DiagnosticFixtureFailure, DiagnosticStorageFixture};
fn fixture_owner(
    fixture: &DiagnosticStorageFixture,
    context: &JobContext,
) -> Result<(DiagnosticStorage, DiagnosticStorageOperation), TestError> {
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key(1)?, context)?;
    storage.resolve(
        Path::new("external-fixture-only"),
        operation.key().owner().namespace,
        &operation,
        context,
    )?;
    storage.acquire_writer(&operation, context)?;
    assert_eq!(
        storage.inventory(&operation, context)?.status(),
        DiagnosticInventoryStatus::Complete
    );
    Ok((storage, operation))
}
#[test]
fn diagnostic_public_fixture_partial_outputs_retain_real_claim_until_checked_retirement()
-> TestResult {
    for fault in [DiagnosticFixtureFailure::PartialCreate, DiagnosticFixtureFailure::PartialOpen] {
        admitted(move |context, _| {
            let fixture = DiagnosticStorageFixture::new()?;
            let object = DiagnosticObject::Summary(incident()?);
            fixture.put(object, b"summary")?;
            let (mut storage, operation) = fixture_owner(&fixture, &context)?;
            fixture.set_failure(fault, true);
            let code = match fault {
                DiagnosticFixtureFailure::PartialCreate => storage
                    .stage(object, 64, &operation, &context)
                    .err()
                    .ok_or("partial create succeeded")?
                    .code(),
                DiagnosticFixtureFailure::PartialOpen => storage
                    .read(object, 64, &operation, &context)
                    .err()
                    .ok_or("partial open succeeded")?
                    .code(),
                _ => return Err("unexpected fault".into()),
            };
            assert!(matches!(code, Code::Read | Code::Write));
            assert_eq!(fixture.snapshot().live_outputs(), 1);
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert!(context.memory().claim(1).is_err());
            fixture.set_failure(DiagnosticFixtureFailure::Close, true);
            assert_eq!(
                storage.retire(&operation, &context).err().ok_or("failed close succeeded")?.code(),
                Code::Retirement
            );
            assert_eq!(fixture.snapshot().live_outputs(), 1);
            assert!(context.memory().claim(1).is_err());
            fixture.set_failure(DiagnosticFixtureFailure::Close, false);
            storage.retire(&operation, &context)?;
            assert_eq!(fixture.snapshot().live_outputs(), 0);
            drop(operation);
            drop(storage);
            assert_eq!(
                context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
                DIAGNOSTIC_STORAGE_BYTES
            );
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_public_fixture_inventory_binds_actual_canonical_subject_and_preserves_failed_close()
-> TestResult {
    admitted(|context, _| {
        let fixture = DiagnosticStorageFixture::new()?;
        let object = DiagnosticObject::Summary(incident()?);
        let expected = fixture.put(object, b"summary")?;
        fixture.put(DiagnosticObject::StoreState, b"ledger")?;
        let (mut storage, operation) = fixture_owner(&fixture, &context)?;
        let inventory = storage.inventory(&operation, &context)?;
        let candidate = inventory
            .entries()
            .find(|entry| entry.class() == DiagnosticEntryClass::Summary)
            .ok_or("canonical summary missing")?;
        assert_eq!(candidate.file_identity(), expected.file_identity());
        let mut lease = storage.read_inventory(candidate, 64, &operation, &context)?;
        let wrong =
            DiagnosticObject::Summary(IncidentKey::new(IncidentSubject::Supervisor(owner_key(4)?)));
        assert_eq!(
            lease.bind_object(wrong, &context).err().ok_or("changed subject bound")?.code(),
            Code::InvalidIdentity
        );
        lease.bind_object(object, &context)?;
        assert_eq!(lease.facts(), expected);
        assert!(fixture.erase_existing(object).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(lease.retire(&context).is_err());
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        assert_eq!(storage.snapshot().reads(), 1);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        lease.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_public_fixture_post_rename_failure_observes_actual_target_and_retains_output()
-> TestResult {
    admitted(|context, _| {
        let fixture = DiagnosticStorageFixture::new()?;
        let (mut storage, operation) = fixture_owner(&fixture, &context)?;
        let object = DiagnosticObject::Summary(incident()?);
        let mut stage = storage.stage(object, 8, &operation, &context)?;
        stage.write_chunk(b"summary", &context)?;
        let expected = stage.flush(&context)?;
        assert_eq!(fixture.snapshot().mutation_count(), 2);
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert_eq!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .err()
                .ok_or("post-rename failure minted activation")?
                .code(),
            Code::ActivationUncertain
        );
        assert_eq!(fixture.snapshot().mutation_count(), 3);
        assert_eq!(fixture.snapshot().active_identity(object), Some(expected.read_facts()));
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        assert_eq!(storage.snapshot().stages(), 1);
        assert!(storage.finish_operation(&operation, &context).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(storage.retire(&operation, &context).is_err());
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        storage.retire(&operation, &context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_public_fixture_bounded_observations_preserve_unique_stage_inodes() -> TestResult {
    let fixture = DiagnosticStorageFixture::new()?;
    assert!(fixture.put(DiagnosticObject::Seed(incident()?), &vec![0; 4097]).is_err());
    let mut identities = std::collections::BTreeSet::new();
    for index in 1..=256 {
        let historical =
            LogicalStoreOperationKey::new(owner_key(3)?, IncidentOperationId::new(index)?);
        let facts = fixture.put_stage(DiagnosticObject::StoreState, historical, b"candidate")?;
        assert!(identities.insert(facts.file_identity()));
    }
    assert_eq!(fixture.snapshot().live_outputs(), 0);
    assert_eq!(fixture.snapshot().mutation_count(), 0);
    Ok(())
}

#[test]
fn diagnostic_public_fixture_reuses_only_checked_closed_external_output_slots() -> TestResult {
    admitted(|context, _| {
        let fixture = DiagnosticStorageFixture::new()?;
        let object = DiagnosticObject::Summary(incident()?);
        let expected = fixture.put(object, b"summary")?;
        let (mut storage, operation) = fixture_owner(&fixture, &context)?;
        for _ in 0..300 {
            let mut lease = storage.read(object, 64, &operation, &context)?;
            assert_eq!(lease.facts(), expected);
            assert_eq!(fixture.snapshot().live_outputs(), 1);
            lease.retire(&context)?;
            assert_eq!(fixture.snapshot().live_outputs(), 0);
        }
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
