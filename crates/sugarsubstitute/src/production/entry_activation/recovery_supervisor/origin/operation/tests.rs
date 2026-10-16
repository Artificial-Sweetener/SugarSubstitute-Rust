// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply external filesystem observations to the real repository retirement owner.
use super::super::test_support::TestError;
use super::*;
use sugarsubstitute_contracts::{
    incident::{IncidentKey, IncidentOperationId, IncidentSubject},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticObject, fixture::DiagnosticStorageFixture,
};
/// Prepare actual native storage ownership; this fixture grants no recovery publication authority.
pub(in crate::production::entry_activation::recovery_supervisor::origin) fn prepared(
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<Publication, TestError> {
    let mut storage = fixture.storage(context)?;
    let key = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    );
    let operation = storage.begin_operation(key, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    let stage = storage.stage(
        DiagnosticObject::Summary(IncidentKey::new(IncidentSubject::Supervisor(key.owner()))),
        64,
        &operation,
        context,
    )?;
    // Dropping the public pin leaves the actual unfinished stage owned by storage.
    drop(stage);
    assert_eq!(storage.snapshot().stages(), 1);
    assert_eq!(storage.snapshot().live_handles(), 1);
    assert_eq!(fixture.snapshot().live_outputs(), 1);
    let mut publication = Publication::new();
    publication.repository = Some(IncidentRepository::new(storage, operation, context)?);
    publication.opening = Some(opening);
    publication.started = true;
    Ok(publication)
}

/// Retain a genuine canonical read pin alongside its actual repository, without issuing origin proof.
pub(in crate::production::entry_activation::recovery_supervisor::origin) fn prepared_read(
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<
    (Publication, sugarsubstitute_supervision::diagnostic_storage::DiagnosticReadLease),
    TestError,
> {
    let key = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    );
    let object = DiagnosticObject::Journal(key);
    fixture.put(object, b"preserved canonical journal fixture")?;
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    let read = storage.read(object, 4096, &operation, context)?;
    assert_eq!(storage.snapshot().reads(), 1);
    assert!(storage.snapshot().writer_held());
    let mut publication = Publication::new();
    publication.repository = Some(IncidentRepository::new(storage, operation, context)?);
    publication.opening = Some(opening);
    publication.started = true;
    Ok((publication, read))
}
/// Inspect only the real retained repository state; this supplies no origin or retirement authority.
pub(in crate::production::entry_activation::recovery_supervisor::origin) fn snapshot(
    publication: &Publication,
) -> Result<sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageSnapshot, TestError> {
    Ok(publication.repository.as_ref().ok_or("Actual retained repository missing")?.snapshot())
}
