// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact intent phases and native slots through callback and effect uncertainty.
use super::*;
impl MigrationIntent {
    pub(super) fn new(id: u64, object: DiagnosticObject) -> Self {
        Self {
            id,
            object,
            original_read: None,
            original_stage: None,
            candidate_stage: None,
            journal_stage: None,
            journal_read: None,
            journal_facts: None,
            journal_attempted: false,
            archive_stage: None,
            archive_sha256: None,
            archive_bytes: None,
            plan: None,
            case: DiagnosticSummaryMigrationCase::Original,
            validating: false,
            semantic_verified: false,
            candidate_active: None,
            archive_active: None,
        }
    }
}
pub(super) fn required(
    id: Option<u64>,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    id.ok_or_else(|| error(key, Code::Closed))
}
pub(super) fn migration(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<&mut MigrationIntent, DiagnosticStorageError> {
    state
        .migration
        .as_deref_mut()
        .filter(|intent| intent.id == id)
        .ok_or_else(|| error(key, Code::InvalidIdentity))
}
pub(super) fn pin(
    shared: &Shared,
    other: &Shared,
    key: LogicalStoreOperationKey,
    operation: &DiagnosticStorageOperation,
) -> Result<(), DiagnosticStorageError> {
    if !Arc::ptr_eq(shared, other)
        || !Arc::ptr_eq(shared, &operation.shared)
        || key != operation.key
    {
        return Err(error(operation.key, Code::InvalidIdentity));
    }
    Ok(())
}
pub(super) fn validation(
    key: LogicalStoreOperationKey,
    cause: DiagnosticRecoveryValidationError,
) -> DiagnosticStorageError {
    native(key, Code::InvalidIdentity, ClaimError::new("diagnostic_migration_validation", cause))
}
pub(super) fn revalidate(
    state: &mut State,
    sources: &[&DiagnosticRecoverySource],
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    if !state.writer || state.namespace != Some(key.owner().namespace) {
        return Err(error(key, Code::InvalidIdentity));
    }
    for source in sources {
        retained_source(state, source, key, context)?;
    }
    Ok(())
}
pub(in crate::diagnostic_storage) fn pins_read(state: &State, id: u64) -> bool {
    state
        .migration
        .as_ref()
        .is_some_and(|intent| intent.original_read == Some(id) || intent.journal_read == Some(id))
}
pub(super) fn classified(
    state: &State,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if !state.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    for entry in &state.scan_entries {
        let recognized = match entry.class {
            DiagnosticEntryClass::Stage => state.archives.iter().any(|archive| {
                archive.generation == state.scan_generation
                    && archive.original.identity == entry.identity
                    && archive.original.bytes == entry.bytes
                    && crate::protected_storage::record_file::diagnostic_object_binding(
                        archive.object,
                        key.owner().namespace,
                    )
                    .1 == entry.digest
            }),
            DiagnosticEntryClass::Journal => state.archives.iter().any(|archive| {
                archive.generation == state.scan_generation
                    && archive.archive.identity == entry.identity
                    && archive.archive.bytes == entry.bytes
                    && crate::protected_storage::record_file::diagnostic_object_binding(
                        DiagnosticObject::Journal(archive.historical),
                        key.owner().namespace,
                    )
                    .1 == entry.digest
            }),
            _ => true,
        };
        if !recognized {
            return Err(error(key, Code::InventoryIncomplete));
        }
    }
    Ok(())
}
pub(super) fn adopt_stage(
    state: &mut State,
    object: DiagnosticObject,
    plan: &DiagnosticSummaryMigrationPlan,
    role: MigrationRole,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<u64, DiagnosticStorageError> {
    check(state, key, context, false)?;
    if !state.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    let (expected, access) = match role {
        MigrationRole::Original => (plan.original_stage, MigrationStageAccess::PreservedOriginal),
        MigrationRole::Candidate => (plan.candidate_stage, MigrationStageAccess::Candidate),
        _ => return Err(error(key, Code::InvalidIdentity)),
    };
    if state.stages.iter().any(|stage| stage.recovered == Some(expected)) {
        return Err(error(key, Code::Busy));
    }
    let id = reserve_stage(state, object, role, 0, key)?;
    let index = stage_index(state, id, key)?;
    state.stages[index].recovered = Some(expected);
    state.stages[index].written = expected.bytes;
    let (native_id, actual) = state
        .port
        .recover_migration_stage(object, plan.historical, expected, access, context)
        .map_err(|cause| native(key, Code::Read, cause))?;
    state.stages[index].native = Some(native_id);
    if native_id == 0 || !expected.matches(actual) {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.stages[index].flushed = Some(actual);
    Ok(id)
}
pub(super) fn reserve_archive(
    state: &mut State,
    historical: LogicalStoreOperationKey,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if state.archives.iter().any(|archive| archive.historical == historical) {
        return Ok(());
    }
    if state.archives.len() >= DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND {
        return Err(error(key, Code::Capacity));
    }
    state.archives.try_reserve_exact(1).map_err(|cause| {
        native(key, Code::Capacity, ClaimError::new("diagnostic_archive_reservation", cause))
    })
}
pub(super) fn recognize(
    state: &mut State,
    plan: DiagnosticSummaryMigrationPlan,
    archive: DiagnosticReadFacts,
    original: DiagnosticReadFacts,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if let Some(known) = state.archives.iter_mut().find(|known| known.historical == plan.historical)
    {
        if known.object != DiagnosticObject::Summary(plan.incident)
            || known.archive != archive
            || known.original != original
        {
            return Err(error(key, Code::ChangedIdentity));
        }
        known.generation = state.scan_generation;
    } else {
        reserve_archive(state, plan.historical, key)?;
        state.archives.push(MigrationArchive {
            historical: plan.historical,
            object: DiagnosticObject::Summary(plan.incident),
            archive,
            original,
            generation: state.scan_generation,
        });
    }
    Ok(())
}
