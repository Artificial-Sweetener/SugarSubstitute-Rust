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
pub(super) fn migration<'a>(
    state: &'a mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<&'a mut MigrationIntent, DiagnosticStorageError> {
    state
        .data_mut()?
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
    if !Shared::ptr_eq(shared, other)
        || !Shared::ptr_eq(shared, &operation.shared)
        || key != operation.key
    {
        return Err(error(operation.key, Code::InvalidIdentity));
    }
    Ok(())
}
pub(super) fn revalidate(
    state: &mut StateAccess<'_>,
    sources: &[&DiagnosticRecoverySource],
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    if !state.data_mut()?.writer || state.data_mut()?.namespace != Some(key.owner().namespace) {
        return Err(error(key, Code::InvalidIdentity));
    }
    for source in sources {
        retained_source(state, source, key, context)?;
    }
    Ok(())
}
pub(in crate::diagnostic_storage) fn pins_read(
    state: &mut StateAccess<'_>,
    id: u64,
) -> Result<bool, DiagnosticStorageError> {
    Ok(state
        .data_mut()?
        .migration
        .as_ref()
        .is_some_and(|intent| intent.original_read == Some(id) || intent.journal_read == Some(id)))
}

pub(super) fn classified(
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    let data = state.data_mut()?;
    if !data.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    for entry in &data.scan_entries {
        let recognized = match entry.class {
            DiagnosticEntryClass::Stage => data.archives.iter().any(|archive| {
                archive.generation == data.scan_generation
                    && archive.original.identity == entry.identity
                    && archive.original.bytes == entry.bytes
                    && crate::durable_storage::protected::record_file::diagnostic_object_binding(
                        archive.object,
                        key.owner().namespace,
                    )
                    .1 == entry.digest
            }),
            DiagnosticEntryClass::Journal => {
                data.archives.iter().any(|archive| {
                    archive.generation == data.scan_generation
                    && archive.archive.identity == entry.identity
                    && archive.archive.bytes == entry.bytes
                    && crate::durable_storage::protected::record_file::diagnostic_object_binding(
                        DiagnosticObject::Journal(archive.historical),
                        key.owner().namespace,
                    )
                    .1 == entry.digest
                }) || capture_read::recognized_origin(data, entry)
            }
            _ => true,
        };
        if !recognized {
            return Err(error(key, Code::InventoryIncomplete));
        }
    }
    Ok(())
}
pub(super) fn adopt_stage(
    state: &mut StateAccess<'_>,
    object: DiagnosticObject,
    plan: &DiagnosticSummaryMigrationPlan,
    role: MigrationRole,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<u64, DiagnosticStorageError> {
    check(state, key, context, false)?;
    if !state.data_mut()?.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    let (expected, access) = match role {
        MigrationRole::Original => (plan.original_stage, SummaryStageAccess::PreservedOriginal),
        MigrationRole::Candidate => (plan.candidate_stage, SummaryStageAccess::Candidate),
        _ => return Err(error(key, Code::InvalidIdentity)),
    };
    if state.data_mut()?.stages.iter().any(|stage| stage.recovered == Some(expected)) {
        return Err(error(key, Code::Busy));
    }
    let id = reserve_stage(state, object, stage::StageRole::Migration(role), 0, key)?;
    let index = stage_index(state, id, key)?;
    state.data_mut()?.stages[index].recovered = Some(expected);
    state.data_mut()?.stages[index].written = expected.bytes;
    let (native_id, actual) = state
        .call_native(|port| {
            port.recover_summary_stage(object, plan.historical, expected, access, context)
        })?
        .map_err(|cause| native(key, Code::Read, cause))?;
    state.data_mut()?.stages[index].native = Some(native_id);
    if native_id == 0 || !expected.matches(actual) {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.data_mut()?.stages[index].flushed = Some(actual);
    Ok(id)
}
pub(super) fn reserve_archive(
    state: &mut StateAccess<'_>,
    historical: LogicalStoreOperationKey,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if state.data_mut()?.archives.iter().any(|archive| archive.historical == historical) {
        return Ok(());
    }
    if state.data_mut()?.archives.len() >= DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND {
        return Err(error(key, Code::Capacity));
    }
    let reservation = allocation::reservation(&state.data_mut()?.allocation, Some(key))?;
    state.data_mut()?.archives.try_reserve_exact(1).map_err(|cause| reservation.failed(cause))
}
pub(super) fn recognize(
    state: &mut StateAccess<'_>,
    plan: DiagnosticSummaryMigrationPlan,
    archive: DiagnosticReadFacts,
    original: DiagnosticReadFacts,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    let generation = state.data_mut()?.scan_generation;
    if let Some(known) =
        state.data_mut()?.archives.iter_mut().find(|known| known.historical == plan.historical)
    {
        if known.object != DiagnosticObject::Summary(plan.incident)
            || known.archive != archive
            || known.original != original
        {
            return Err(error(key, Code::ChangedIdentity));
        }
        known.generation = generation;
    } else {
        reserve_archive(state, plan.historical, key)?;
        state.data_mut()?.archives.push(MigrationArchive {
            historical: plan.historical,
            object: DiagnosticObject::Summary(plan.incident),
            archive,
            original,
            generation,
        });
    }
    Ok(())
}
